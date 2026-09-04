use super::format_rfc3339;
use crate::mcp::budget::{self, BudgetContext, ToolBudget};
use time::{Date, OffsetDateTime, Time, UtcOffset};

#[derive(Clone, Debug)]
pub struct CronField {
    pub values: Vec<u32>,
    pub min: u32,
    pub max: u32,
    pub star_syntax: bool,
}

impl CronField {
    fn allows(&self, value: u32) -> bool {
        value >= self.min && value <= self.max && self.values.binary_search(&value).is_ok()
    }

    fn normalized(&self) -> String {
        self.values
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Clone, Debug)]
pub struct CronSchedule {
    pub minute: CronField,
    pub hour: CronField,
    pub day_of_month: CronField,
    pub month: CronField,
    pub day_of_week: CronField,
}

fn parse_number(token: &str, names: &[(&str, u32)], min: u32, max: u32) -> Result<u32, String> {
    if let Some((_, value)) = names
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(token))
    {
        return Ok(*value);
    }
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("invalid cron value: {token}"));
    }
    let value = token
        .parse::<u32>()
        .map_err(|_| format!("cron value is too large: {token}"))?;
    if value < min || value > max {
        return Err(format!("cron value {value} is outside {min}..{max}"));
    }
    Ok(value)
}

fn parse_field(
    input: &str,
    min: u32,
    max: u32,
    names: &[(&str, u32)],
    sunday_alias: bool,
) -> Result<CronField, String> {
    if input.is_empty() {
        return Err("cron fields cannot be empty".to_string());
    }
    let mut values = vec![false; (max - min + 1) as usize];
    for item in input.split(',') {
        if item.is_empty() {
            return Err("cron lists cannot contain empty items".to_string());
        }
        let (range_part, step) = match item.split_once('/') {
            Some((range, step)) if !step.is_empty() && !step.contains('/') => {
                let step = step
                    .parse::<u32>()
                    .map_err(|_| format!("invalid cron step: {step}"))?;
                if step == 0 {
                    return Err("cron step must be positive".to_string());
                }
                (range, step)
            }
            Some(_) => return Err(format!("invalid cron step expression: {item}")),
            None => (item, 1),
        };
        let (start, end) = if range_part == "*" {
            (min, max)
        } else if let Some((start, end)) = range_part.split_once('-') {
            if end.contains('-') || start.is_empty() || end.is_empty() {
                return Err(format!("invalid cron range: {range_part}"));
            }
            let start = parse_number(start, names, min, max)?;
            let end = parse_number(end, names, min, max)?;
            if start > end {
                return Err(format!("cron ranges do not wrap: {range_part}"));
            }
            (start, end)
        } else if item.contains('/') {
            let start = parse_number(range_part, names, min, max)?;
            (start, max)
        } else {
            let value = parse_number(range_part, names, min, max)?;
            (value, value)
        };
        let mut value = start;
        while value <= end {
            let normalized = if sunday_alias && value == 7 { 0 } else { value };
            values[(normalized - min) as usize] = true;
            match value.checked_add(step) {
                Some(next) => value = next,
                None => break,
            }
        }
    }
    let values = values
        .into_iter()
        .enumerate()
        .filter_map(|(index, enabled)| enabled.then_some(index as u32 + min))
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err("cron field must allow at least one value".to_string());
    }
    Ok(CronField {
        values,
        min,
        max,
        star_syntax: input.starts_with('*'),
    })
}

const MONTHS: [(&str, u32); 12] = [
    ("JAN", 1),
    ("FEB", 2),
    ("MAR", 3),
    ("APR", 4),
    ("MAY", 5),
    ("JUN", 6),
    ("JUL", 7),
    ("AUG", 8),
    ("SEP", 9),
    ("OCT", 10),
    ("NOV", 11),
    ("DEC", 12),
];
const WEEKDAYS: [(&str, u32); 7] = [
    ("SUN", 0),
    ("MON", 1),
    ("TUE", 2),
    ("WED", 3),
    ("THU", 4),
    ("FRI", 5),
    ("SAT", 6),
];

pub fn parse(expression: &str) -> Result<CronSchedule, String> {
    if expression.trim_start().starts_with('@') {
        return Err("cron nicknames such as @daily are not supported".to_string());
    }
    let fields = expression.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err("cron expression must contain exactly five fields".to_string());
    }
    if expression.contains("CRON_TZ") || expression.starts_with("TZ=") {
        return Err("cron timezone prefixes are not supported".to_string());
    }
    Ok(CronSchedule {
        minute: parse_field(fields[0], 0, 59, &[], false)?,
        hour: parse_field(fields[1], 0, 23, &[], false)?,
        day_of_month: parse_field(fields[2], 1, 31, &[], false)?,
        month: parse_field(fields[3], 1, 12, &MONTHS, false)?,
        day_of_week: parse_field(fields[4], 0, 7, &WEEKDAYS, true)?,
    })
}

pub fn normalized_expression(schedule: &CronSchedule) -> String {
    [
        schedule.minute.normalized(),
        schedule.hour.normalized(),
        schedule.day_of_month.normalized(),
        schedule.month.normalized(),
        schedule.day_of_week.normalized(),
    ]
    .join(" ")
}

pub fn parsed_values(schedule: &CronSchedule) -> serde_json::Value {
    serde_json::json!({
        "minute": schedule.minute.values,
        "hour": schedule.hour.values,
        "day_of_month": schedule.day_of_month.values,
        "month": schedule.month.values,
        "day_of_week": schedule.day_of_week.values,
    })
}

fn day_matches(schedule: &CronSchedule, date: Date) -> bool {
    if !schedule.month.allows(date.month() as u32) {
        return false;
    }
    let dom = schedule.day_of_month.allows(u32::from(date.day()));
    let dow = schedule
        .day_of_week
        .allows(u32::from(date.weekday().number_days_from_sunday()));
    // Vixie/Cronie semantics: if either DOM or DOW starts with `*`
    // (including `*/n` steps), both parsed predicates must match.
    // Otherwise either field may match.
    if schedule.day_of_month.star_syntax || schedule.day_of_week.star_syntax {
        dom && dow
    } else {
        dom || dow
    }
}

pub fn search_next(
    schedule: &CronSchedule,
    after: OffsetDateTime,
    count: usize,
) -> Result<Vec<String>, Box<crate::mcp::response::ToolResponse>> {
    let budget = budget::for_handler(ToolBudget::MODERATE);
    let offset: UtcOffset = after.offset();
    let mut date = after.date();
    let mut result = Vec::new();
    const MAX_DAYS: usize = 146_097;
    for _ in 0..=MAX_DAYS {
        if budget.should_stop() {
            return Err(Box::new(
                budget.check_should_stop("cron_inspect").unwrap_err(),
            ));
        }
        if day_matches(schedule, date) {
            for hour in &schedule.hour.values {
                for minute in &schedule.minute.values {
                    let local = date
                        .with_time(
                            Time::from_hms(*hour as u8, *minute as u8, 0)
                                .expect("cron range is validated"),
                        )
                        .assume_offset(offset);
                    if local > after {
                        result.push(format_rfc3339(local).unwrap_or_else(|_| local.to_string()));
                        if result.len() == count {
                            return Ok(result);
                        }
                    }
                }
            }
        }
        date = match date.next_day() {
            Some(next) => next,
            None => {
                return Err(Box::new(
                    crate::mcp::response::ToolResponse::error_with_code(
                        "invalid_arguments",
                        crate::mcp::machine_codes::INVALID_ARGUMENTS,
                        "cron search exceeded the supported calendar range",
                        None,
                        Some("cron_inspect"),
                    ),
                ))
            }
        };
    }
    Ok(result)
}

pub fn satisfiable(schedule: &CronSchedule, after: OffsetDateTime) -> bool {
    let budget_ctx: BudgetContext = budget::for_handler(ToolBudget::CHEAP);
    let mut date = after.date();
    for _ in 0..=146_097 {
        if budget_ctx.should_stop() {
            return false;
        }
        if day_matches(schedule, date) {
            return true;
        }
        let Some(next) = date.next_day() else {
            return false;
        };
        date = next;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::{Date, Month};

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).unwrap()
    }

    #[test]
    fn dom_and_dow_use_star_syntax_flags() {
        let monday_jun1 = date(2026, Month::June, 1);
        let tuesday_jun2 = date(2026, Month::June, 2);
        let monday_jun8 = date(2026, Month::June, 8);
        let wed_jul1 = date(2026, Month::July, 1);

        // 1. `0 0 * * MON` -> Mondays only (DOM star => AND).
        let dom_star = parse("0 0 * * MON").unwrap();
        assert!(dom_star.day_of_month.star_syntax);
        assert!(!dom_star.day_of_week.star_syntax);
        assert!(day_matches(&dom_star, monday_jun1));
        assert!(!day_matches(&dom_star, tuesday_jun2));
        assert!(day_matches(&dom_star, monday_jun8));

        // 2. `0 0 1 * *` -> first of month only (DOW star => AND).
        let dow_star = parse("0 0 1 * *").unwrap();
        assert!(!dow_star.day_of_month.star_syntax);
        assert!(dow_star.day_of_week.star_syntax);
        assert!(day_matches(&dow_star, monday_jun1));
        assert!(!day_matches(&dow_star, tuesday_jun2));
        assert!(!day_matches(&dow_star, monday_jun8));
        assert!(day_matches(&dow_star, wed_jul1));

        // 3. `0 0 1 * MON` -> first of month OR Monday (neither star => OR).
        let both_restricted = parse("0 0 1 * MON").unwrap();
        assert!(!both_restricted.day_of_month.star_syntax);
        assert!(!both_restricted.day_of_week.star_syntax);
        assert!(day_matches(&both_restricted, monday_jun1));
        assert!(day_matches(&both_restricted, monday_jun8));
        assert!(day_matches(&both_restricted, wed_jul1));
        assert!(!day_matches(&both_restricted, tuesday_jun2));

        // 4. `0 0 1-31 * MON` -> every valid day (explicit full range is not
        // star syntax, so OR with a full DOM set matches everything).
        let full_dom_range = parse("0 0 1-31 * MON").unwrap();
        assert!(!full_dom_range.day_of_month.star_syntax);
        assert!(!full_dom_range.day_of_week.star_syntax);
        assert!(day_matches(&full_dom_range, monday_jun1));
        assert!(day_matches(&full_dom_range, tuesday_jun2));
        assert!(day_matches(&full_dom_range, monday_jun8));

        let full_dow_range = parse("0 0 1 * 0-7").unwrap();
        assert!(!full_dow_range.day_of_week.star_syntax);
        assert!(day_matches(&full_dow_range, tuesday_jun2));
        assert!(day_matches(&full_dow_range, monday_jun8));
    }

    #[test]
    fn star_step_fields_carry_star_syntax_and_use_and() {
        let monday_jun1 = date(2026, Month::June, 1);
        let tuesday_jun2 = date(2026, Month::June, 2);
        let wed_jun3 = date(2026, Month::June, 3);
        let monday_jun8 = date(2026, Month::June, 8);
        let monday_jun15 = date(2026, Month::June, 15);
        let wed_jul1 = date(2026, Month::July, 1);
        let sun_feb1 = date(2026, Month::February, 1);
        let sat_aug1 = date(2026, Month::August, 1);
        let tue_sep1 = date(2026, Month::September, 1);

        // 5. `0 0 */1 * MON` -> Mondays only. `*/1` covers every DOM value
        // but still carries Vixie star syntax, so AND applies.
        let dom_star_step_all = parse("0 0 */1 * MON").unwrap();
        assert!(dom_star_step_all.day_of_month.star_syntax);
        assert!(!dom_star_step_all.day_of_week.star_syntax);
        assert!(day_matches(&dom_star_step_all, monday_jun1));
        assert!(day_matches(&dom_star_step_all, monday_jun8));
        assert!(!day_matches(&dom_star_step_all, tuesday_jun2));
        assert!(!day_matches(&dom_star_step_all, wed_jun3));

        // 6. `0 0 */2 * MON` -> only Mondays whose day-of-month is odd.
        // `*/2` from 1 matches 1,3,5,...,31.
        let dom_star_step = parse("0 0 */2 * MON").unwrap();
        assert!(dom_star_step.day_of_month.star_syntax);
        assert!(day_matches(&dom_star_step, monday_jun1));
        assert!(day_matches(&dom_star_step, monday_jun15));
        assert!(!day_matches(&dom_star_step, monday_jun8));
        assert!(!day_matches(&dom_star_step, wed_jun3));
        assert!(!day_matches(&dom_star_step, tuesday_jun2));

        // 7. `0 0 1 * */1` -> first of month only. DOW `*/1` covers every
        // weekday value but still selects AND semantics.
        let dow_star_step_all = parse("0 0 1 * */1").unwrap();
        assert!(!dow_star_step_all.day_of_month.star_syntax);
        assert!(dow_star_step_all.day_of_week.star_syntax);
        assert!(day_matches(&dow_star_step_all, monday_jun1));
        assert!(day_matches(&dow_star_step_all, wed_jul1));
        assert!(!day_matches(&dow_star_step_all, monday_jun8));
        assert!(!day_matches(&dow_star_step_all, tuesday_jun2));

        // 8. DOW star-step narrower than `*/1`: `0 0 1 * */2` matches day 1
        // AND Sun/Tue/Thu/Sat. `*/2` from 0 matches 0,2,4,6.
        let dow_star_step = parse("0 0 1 * */2").unwrap();
        assert!(dow_star_step.day_of_week.star_syntax);
        assert!(day_matches(&dow_star_step, sun_feb1));
        assert!(day_matches(&dow_star_step, sat_aug1));
        assert!(day_matches(&dow_star_step, tue_sep1));
        // DOM matches but DOW does not.
        assert!(!day_matches(&dow_star_step, monday_jun1));
        assert!(!day_matches(&dow_star_step, wed_jul1));
        // DOW matches but DOM does not.
        assert!(!day_matches(&dow_star_step, tuesday_jun2));

        // DOM star with DOW star-step also uses AND: `0 0 * * */2`.
        let both_star_step = parse("0 0 * * */2").unwrap();
        assert!(both_star_step.day_of_month.star_syntax);
        assert!(both_star_step.day_of_week.star_syntax);
        assert!(day_matches(&both_star_step, tuesday_jun2));
        assert!(day_matches(&both_star_step, sun_feb1));
        assert!(!day_matches(&both_star_step, monday_jun1));

        // Sunday 0/7 normalization still holds under AND semantics.
        for dow in ["0", "7", "SUN"] {
            let sunday = parse(&format!("0 0 * * {dow}")).unwrap();
            assert!(day_matches(&sunday, sun_feb1));
            assert!(!day_matches(&sunday, monday_jun1));
        }
    }
}
