use crate::mcp::registry::ToolCost;
use crate::mcp::response::ToolResponse;
use std::cell::RefCell;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

thread_local! {
    static CURRENT_CANCEL_FLAG: RefCell<Option<Arc<AtomicBool>>> = const { RefCell::new(None) };
}

/// Set the current thread's cancellation flag for the duration of `f`.
///
/// Nested calls properly restore the previous flag when `f` returns.
pub fn with_cancel_flag<F, R>(flag: Option<Arc<AtomicBool>>, f: F) -> R
where
    F: FnOnce() -> R,
{
    CURRENT_CANCEL_FLAG.with(|cell| {
        let prev = std::mem::replace(&mut *cell.borrow_mut(), flag);
        let result = f();
        *cell.borrow_mut() = prev;
        result
    })
}

/// Retrieve the current thread's cancellation flag, if any.
pub fn current_cancel_flag() -> Option<Arc<AtomicBool>> {
    CURRENT_CANCEL_FLAG.with(|cell| cell.borrow().clone())
}

/// Create a `BudgetContext` suitable for a tool handler, automatically
/// picking up the thread-local cancellation flag if one is set.
///
/// This is the recommended way for handler functions (which have signature
/// `fn(&Value) -> ToolResponse` and cannot receive context directly) to
/// create a `BudgetContext` with cancellation support.
pub fn for_handler(budget: ToolBudget) -> BudgetContext {
    let mut ctx = BudgetContext::new(budget);
    if let Some(flag) = current_cancel_flag() {
        ctx = ctx.with_cancellation(flag);
    }
    ctx
}

/// Budget tier classification for MCP tools.
///
/// `BudgetTier` is the lightweight label that maps a tool's cost class to an
/// enforceable `ToolBudget`. The tiers are intentionally coarse so that tool
/// authors can declare cost once (via `ToolCost`) and the runtime can resolve
/// concrete limits at dispatch time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BudgetTier {
    Cheap,
    Moderate,
    Heavy,
}

/// Enforceable resource budget for an MCP tool invocation.
///
/// Unlike `ToolCost` which is a *descriptive* metadata annotation on a
/// `ToolSpec`, `ToolBudget` carries the *enforceable* numeric limits that the
/// runtime applies during tool execution. This separation keeps tool
/// declarations ergonomic while giving the runtime precise control.
///
/// Three canonical budget tiers are provided as `const` values:
/// - `CHEAP` – fast, lightweight tools (unit conversion, text compare, …)
/// - `MODERATE` – tools that may do heavier text/regex work
/// - `HEAVY` – composite tools that spawn sub-tools (edit_preflight, …)
///
/// Use the builder methods (`with_max_elapsed_ms`, `with_max_findings`) to
/// derive per-tool overrides from these base budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolBudget {
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    pub max_text_chars: usize,
    pub max_list_items: usize,
    pub max_regex_pattern_chars: usize,
    pub max_regex_samples: usize,
    pub max_elapsed_ms: u64,
    pub max_spawned_workers: usize,
    pub max_findings: usize,
}

impl ToolBudget {
    /// Budget for cheap / low-cost tools.
    pub const CHEAP: Self = Self {
        max_input_bytes: 1_000_000,
        max_output_bytes: 1_000_000,
        max_text_chars: 100_000,
        max_list_items: 10_000,
        max_regex_pattern_chars: 1_000,
        max_regex_samples: 100,
        max_elapsed_ms: 10_000,
        max_spawned_workers: 16,
        max_findings: 100,
    };

    /// Budget for moderate-cost tools (30 s elapsed budget).
    pub const MODERATE: Self = Self {
        max_input_bytes: Self::CHEAP.max_input_bytes,
        max_output_bytes: Self::CHEAP.max_output_bytes,
        max_text_chars: Self::CHEAP.max_text_chars,
        max_list_items: Self::CHEAP.max_list_items,
        max_regex_pattern_chars: Self::CHEAP.max_regex_pattern_chars,
        max_regex_samples: Self::CHEAP.max_regex_samples,
        max_elapsed_ms: 30_000,
        max_spawned_workers: Self::CHEAP.max_spawned_workers,
        max_findings: Self::CHEAP.max_findings,
    };

    /// Budget for heavy / composite tools (30 s elapsed, 2 MB output).
    pub const HEAVY: Self = Self {
        max_input_bytes: Self::MODERATE.max_input_bytes,
        max_output_bytes: 2_000_000,
        max_text_chars: Self::MODERATE.max_text_chars,
        max_list_items: Self::MODERATE.max_list_items,
        max_regex_pattern_chars: Self::MODERATE.max_regex_pattern_chars,
        max_regex_samples: Self::MODERATE.max_regex_samples,
        max_elapsed_ms: Self::MODERATE.max_elapsed_ms,
        max_spawned_workers: Self::MODERATE.max_spawned_workers,
        max_findings: Self::MODERATE.max_findings,
    };

    /// Returns the budget tier that best matches this budget instance.
    ///
    /// Identity is checked against the canonical `CHEAP` / `MODERATE` /
    /// `HEAVY` consts. If the budget has been customised beyond recognition
    /// of any tier, `Moderate` is returned as a safe fallback.
    pub fn tier(&self) -> BudgetTier {
        if *self == Self::CHEAP {
            BudgetTier::Cheap
        } else if *self == Self::MODERATE {
            BudgetTier::Moderate
        } else if *self == Self::HEAVY {
            BudgetTier::Heavy
        } else {
            BudgetTier::Moderate
        }
    }

    /// Derive a new budget with a custom `max_elapsed_ms` override.
    pub fn with_max_elapsed_ms(self, ms: u64) -> Self {
        Self {
            max_elapsed_ms: ms,
            ..self
        }
    }

    /// Derive a new budget with a custom `max_findings` override.
    pub fn with_max_findings(self, n: usize) -> Self {
        Self {
            max_findings: n,
            ..self
        }
    }

    /// Derive a new budget with a custom `max_output_bytes` override.
    /// Used by tests and by tools that need a tighter per-call output cap.
    pub fn with_max_output_bytes(self, n: usize) -> Self {
        Self {
            max_output_bytes: n,
            ..self
        }
    }

    /// Derive a new budget with a custom `max_input_bytes` override.
    pub fn with_max_input_bytes(self, n: usize) -> Self {
        Self {
            max_input_bytes: n,
            ..self
        }
    }
}

/// Map a tool's declared `ToolCost` to an enforceable `ToolBudget`.
///
/// Composite / heavy tools that spawn sub-tools receive explicit overrides
/// regardless of their declared cost, because their actual resource usage
/// can exceed what the cost label alone would suggest.
pub fn budget_for_tool(tool_name: &str, cost: ToolCost) -> ToolBudget {
    // Heavy composite tools always get HEAVY budgets.
    match tool_name {
        "edit_preflight" | "command_preflight" | "config_preflight" => ToolBudget::HEAVY,
        "text_security_inspect" | "patch_apply_check" => ToolBudget::HEAVY,
        _ => budget_for_tier(cost.into()),
    }
}

/// Resolve a `BudgetTier` into its canonical `ToolBudget`.
pub fn budget_for_tier(tier: BudgetTier) -> ToolBudget {
    match tier {
        BudgetTier::Cheap => ToolBudget::CHEAP,
        BudgetTier::Moderate => ToolBudget::MODERATE,
        BudgetTier::Heavy => ToolBudget::HEAVY,
    }
}

/// Implicit conversion from `ToolCost` to `BudgetTier`.
impl From<ToolCost> for BudgetTier {
    fn from(cost: ToolCost) -> Self {
        match cost {
            ToolCost::Cheap => BudgetTier::Cheap,
            ToolCost::Moderate => BudgetTier::Moderate,
            ToolCost::Heavy => BudgetTier::Heavy,
        }
    }
}

/// Runtime context that carries an active budget alongside a deadline and
/// an optional cancellation flag.
///
/// Create a `BudgetContext` at the start of a tool invocation and pass it
/// through the execution path. Long-running loops should periodically
/// check `should_stop()` to honour both the time budget and external
/// cancellation.
pub struct BudgetContext {
    pub deadline: Option<Instant>,
    pub budget: ToolBudget,
    pub cancelled: Option<Arc<AtomicBool>>,
}

impl BudgetContext {
    /// Create a fresh context whose deadline is `now + budget.max_elapsed_ms`.
    pub fn new(budget: ToolBudget) -> Self {
        let deadline = Instant::now() + Duration::from_millis(budget.max_elapsed_ms);
        Self {
            deadline: Some(deadline),
            budget,
            cancelled: None,
        }
    }

    /// Attach an external cancellation flag.
    pub fn with_cancellation(self, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            cancelled: Some(cancelled),
            ..self
        }
    }

    /// Returns `true` when the deadline has passed.
    pub fn is_expired(&self) -> bool {
        match self.deadline {
            Some(d) => Instant::now() >= d,
            None => false,
        }
    }

    /// Returns `true` when the cancellation flag has been set.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
            .as_ref()
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// Returns `true` when the context should stop (expired or cancelled).
    pub fn should_stop(&self) -> bool {
        self.is_expired() || self.is_cancelled()
    }

    /// Returns `Err(TOOL_RESPONSE)` if the context has been cancelled.
    ///
    /// Use this at the top of long-running handlers or loops to bail out
    /// early when the caller has signalled cancellation.
    #[allow(clippy::result_large_err)]
    pub fn check_not_cancelled(&self, tool_name: &str) -> Result<(), ToolResponse> {
        if self.is_cancelled() {
            Err(ToolResponse::error_with_code(
                "cancelled",
                crate::mcp::machine_codes::CANCELLED,
                &format!("Tool '{}' was cancelled", tool_name),
                None,
                Some(tool_name),
            ))
        } else {
            Ok(())
        }
    }

    /// Returns `Err(TOOL_RESPONSE)` if the deadline has been exceeded.
    ///
    /// Use this inside loops that may run long to produce a clean timeout
    /// error instead of relying on the outer `tokio::time::timeout`.
    #[allow(clippy::result_large_err)]
    pub fn check_deadline(&self, tool_name: &str) -> Result<(), ToolResponse> {
        if self.is_expired() {
            Err(ToolResponse::error_with_code(
                "timeout",
                crate::mcp::machine_codes::TIMEOUT,
                &format!("Tool '{}' exceeded its time budget", tool_name),
                Some(vec!["Try a simpler input or shorter text".to_string()]),
                Some(tool_name),
            ))
        } else {
            Ok(())
        }
    }

    /// Returns `Err(TOOL_RESPONSE)` if the text exceeds `max_text_chars`.
    #[allow(clippy::result_large_err)]
    pub fn check_text_len(
        &self,
        field_name: &str,
        text: &str,
        tool_name: &str,
    ) -> Result<(), ToolResponse> {
        if text.len() > self.budget.max_text_chars {
            Err(ToolResponse::error_with_code(
                "input_too_large",
                crate::mcp::machine_codes::INPUT_TOO_LARGE,
                &format!(
                    "Field '{}' length {} exceeds limit {}",
                    field_name,
                    text.len(),
                    self.budget.max_text_chars
                ),
                None,
                Some(tool_name),
            ))
        } else {
            Ok(())
        }
    }

    /// Returns `Err(TOOL_RESPONSE)` if the list length exceeds `max_list_items`.
    #[allow(clippy::result_large_err)]
    pub fn check_list_len(
        &self,
        field_name: &str,
        len: usize,
        tool_name: &str,
    ) -> Result<(), ToolResponse> {
        if len > self.budget.max_list_items {
            Err(ToolResponse::error_with_code(
                "input_too_large",
                crate::mcp::machine_codes::INPUT_TOO_LARGE,
                &format!(
                    "Field '{}' length {} exceeds limit {}",
                    field_name, len, self.budget.max_list_items
                ),
                None,
                Some(tool_name),
            ))
        } else {
            Ok(())
        }
    }

    /// Returns `Err(TOOL_RESPONSE)` if the context should stop.
    ///
    /// Checks cancellation first, then deadline. Use this in handler
    /// cooldown checks where `should_stop()` is true but the reason
    /// could be either cancellation or expiry.
    #[allow(clippy::result_large_err)]
    pub fn check_should_stop(&self, tool_name: &str) -> Result<(), ToolResponse> {
        if self.is_cancelled() {
            return self.check_not_cancelled(tool_name);
        }
        if self.is_expired() {
            return self.check_deadline(tool_name);
        }
        Ok(())
    }

    /// Returns the remaining time before deadline, or `None` if no deadline.
    pub fn remaining_time_ms(&self) -> Option<u64> {
        self.deadline.map(|d| {
            let now = Instant::now();
            if now >= d {
                0
            } else {
                d.duration_since(now).as_millis() as u64
            }
        })
    }
}

/// Budget allocation for a sub-tool within a composite tool invocation.
///
/// Composite tools (edit_preflight, config_preflight, etc.) call multiple
/// sub-tools internally. `SubBudget` provides a fraction of the parent's
/// budget to each sub-tool, preventing any single sub-tool from consuming
/// the entire composite budget.
///
/// The primary constraint is a shared deadline derived from the parent's
/// `BudgetContext`. Input/output caps are proportional fractions.
#[derive(Clone, Copy, Debug)]
pub struct SubBudget {
    pub deadline: Option<Instant>,
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    pub max_text_chars: usize,
    pub max_list_items: usize,
    pub max_findings: usize,
}

impl SubBudget {
    pub fn should_stop(&self) -> bool {
        match self.deadline {
            Some(d) => Instant::now() >= d,
            None => false,
        }
    }
}

/// Manages sub-tool budget allocation for a composite tool.
///
/// Create one per composite tool invocation. Call `allocate()` before
/// each sub-tool invocation to get a `SubBudget` for that sub-tool.
/// The allocator tracks how many sub-tools have been called and
/// ensures the shared deadline is respected.
pub struct CompositeBudgetAllocator {
    deadline: Option<Instant>,
    parent_budget: ToolBudget,
    sub_tool_count: usize,
    total_sub_tools: usize,
}

impl CompositeBudgetAllocator {
    pub fn new(parent: &BudgetContext, total_sub_tools: usize) -> Self {
        Self {
            deadline: parent.deadline,
            parent_budget: parent.budget,
            sub_tool_count: 0,
            total_sub_tools,
        }
    }

    pub fn allocate(&mut self) -> SubBudget {
        let divisor = self.total_sub_tools.max(1);
        self.sub_tool_count += 1;
        SubBudget {
            deadline: self.deadline,
            max_input_bytes: (self.parent_budget.max_input_bytes / divisor).max(1),
            max_output_bytes: (self.parent_budget.max_output_bytes / divisor).max(1),
            max_text_chars: (self.parent_budget.max_text_chars / divisor).max(1),
            max_list_items: (self.parent_budget.max_list_items / divisor).max(1),
            max_findings: (self.parent_budget.max_findings / divisor).max(1),
        }
    }

    pub fn remaining(&self) -> usize {
        self.total_sub_tools.saturating_sub(self.sub_tool_count)
    }
}

/// Create a `BudgetContext` suitable for a composite tool's sub-tool.
///
/// Shares the parent's deadline and cancellation flag but applies
/// reduced input/output limits proportional to the sub-tool's share.
pub fn sub_budget_context(parent: &BudgetContext, sub: &SubBudget) -> BudgetContext {
    BudgetContext {
        deadline: sub.deadline,
        budget: ToolBudget {
            max_input_bytes: sub.max_input_bytes,
            max_output_bytes: sub.max_output_bytes,
            max_text_chars: sub.max_text_chars,
            max_list_items: sub.max_list_items,
            max_regex_pattern_chars: parent.budget.max_regex_pattern_chars,
            max_regex_samples: parent.budget.max_regex_samples,
            max_elapsed_ms: 0,
            max_spawned_workers: parent.budget.max_spawned_workers,
            max_findings: sub.max_findings,
        },
        cancelled: parent.cancelled.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_for_tier_returns_correct_budgets() {
        assert_eq!(budget_for_tier(BudgetTier::Cheap), ToolBudget::CHEAP);
        assert_eq!(budget_for_tier(BudgetTier::Moderate), ToolBudget::MODERATE);
        assert_eq!(budget_for_tier(BudgetTier::Heavy), ToolBudget::HEAVY);
    }

    #[test]
    fn budget_for_tier_tier_roundtrip() {
        assert_eq!(ToolBudget::CHEAP.tier(), BudgetTier::Cheap);
        assert_eq!(ToolBudget::MODERATE.tier(), BudgetTier::Moderate);
        assert_eq!(ToolBudget::HEAVY.tier(), BudgetTier::Heavy);
    }

    #[test]
    fn budget_for_tool_heavy_composite_overrides() {
        let edit = budget_for_tool("edit_preflight", ToolCost::Cheap);
        assert_eq!(edit, ToolBudget::HEAVY);

        let cmd = budget_for_tool("command_preflight", ToolCost::Cheap);
        assert_eq!(cmd, ToolBudget::HEAVY);

        let cfg = budget_for_tool("config_preflight", ToolCost::Cheap);
        assert_eq!(cfg, ToolBudget::HEAVY);

        let sec = budget_for_tool("text_security_inspect", ToolCost::Cheap);
        assert_eq!(sec, ToolBudget::HEAVY);

        let patch = budget_for_tool("patch_apply_check", ToolCost::Cheap);
        assert_eq!(patch, ToolBudget::HEAVY);
    }

    #[test]
    fn budget_for_tool_regular_tool_uses_cost() {
        let b = budget_for_tool("math_eval", ToolCost::Cheap);
        assert_eq!(b, ToolBudget::CHEAP);

        let b = budget_for_tool("text_diff_explain", ToolCost::Moderate);
        assert_eq!(b, ToolBudget::MODERATE);

        let b = budget_for_tool("json_extract", ToolCost::Heavy);
        assert_eq!(b, ToolBudget::HEAVY);
    }

    #[test]
    fn with_max_elapsed_ms_overrides() {
        let b = ToolBudget::CHEAP.with_max_elapsed_ms(5_000);
        assert_eq!(b.max_elapsed_ms, 5_000);
        assert_eq!(b.max_output_bytes, ToolBudget::CHEAP.max_output_bytes);
    }

    #[test]
    fn with_max_findings_overrides() {
        let b = ToolBudget::MODERATE.with_max_findings(50);
        assert_eq!(b.max_findings, 50);
        assert_eq!(b.max_elapsed_ms, ToolBudget::MODERATE.max_elapsed_ms);
    }

    #[test]
    fn budget_context_fresh_is_not_expired() {
        let ctx = BudgetContext::new(ToolBudget::CHEAP);
        assert!(!ctx.is_expired());
        assert!(!ctx.should_stop());
    }

    #[test]
    fn budget_context_not_cancelled_when_none() {
        let ctx = BudgetContext::new(ToolBudget::CHEAP);
        assert!(!ctx.is_cancelled());
    }

    #[test]
    fn budget_context_cancelled_flag() {
        let flag = Arc::new(AtomicBool::new(true));
        let ctx = BudgetContext::new(ToolBudget::CHEAP).with_cancellation(flag);
        assert!(ctx.is_cancelled());
        assert!(ctx.should_stop());
    }

    #[test]
    fn budget_context_not_cancelled_when_flag_false() {
        let flag = Arc::new(AtomicBool::new(false));
        let ctx = BudgetContext::new(ToolBudget::CHEAP).with_cancellation(flag);
        assert!(!ctx.is_cancelled());
        assert!(!ctx.should_stop());
    }

    #[test]
    fn custom_budget_tier_fallback() {
        let custom = ToolBudget {
            max_input_bytes: 999,
            ..ToolBudget::CHEAP
        };
        assert_eq!(custom.tier(), BudgetTier::Moderate);
    }

    #[test]
    fn composite_allocator_even_split() {
        let parent = BudgetContext::new(ToolBudget::HEAVY);
        let mut alloc = CompositeBudgetAllocator::new(&parent, 3);

        let sub1 = alloc.allocate();
        let sub2 = alloc.allocate();
        let sub3 = alloc.allocate();

        let expected_output = ToolBudget::HEAVY.max_output_bytes / 3;
        assert_eq!(sub1.max_output_bytes, expected_output);
        assert_eq!(sub2.max_output_bytes, expected_output);
        assert_eq!(sub3.max_output_bytes, expected_output);

        let expected_text = ToolBudget::HEAVY.max_text_chars / 3;
        assert_eq!(sub1.max_text_chars, expected_text);
        assert_eq!(sub2.max_text_chars, expected_text);
        assert_eq!(sub3.max_text_chars, expected_text);
    }

    #[test]
    fn composite_allocator_deadline_propagated() {
        let parent = BudgetContext::new(ToolBudget::HEAVY);
        let mut alloc = CompositeBudgetAllocator::new(&parent, 2);

        let sub = alloc.allocate();
        assert_eq!(sub.deadline, parent.deadline);
    }

    #[test]
    fn composite_allocator_remaining_decrements() {
        let parent = BudgetContext::new(ToolBudget::HEAVY);
        let mut alloc = CompositeBudgetAllocator::new(&parent, 3);

        assert_eq!(alloc.remaining(), 3);
        alloc.allocate();
        assert_eq!(alloc.remaining(), 2);
        alloc.allocate();
        assert_eq!(alloc.remaining(), 1);
        alloc.allocate();
        assert_eq!(alloc.remaining(), 0);
    }

    #[test]
    fn sub_budget_should_stop_expired() {
        let sub = SubBudget {
            deadline: Some(Instant::now() - Duration::from_millis(100)),
            max_input_bytes: 1000,
            max_output_bytes: 1000,
            max_text_chars: 1000,
            max_list_items: 100,
            max_findings: 10,
        };
        assert!(sub.should_stop());
    }

    #[test]
    fn sub_budget_context_inherits_cancellation() {
        let flag = Arc::new(AtomicBool::new(true));
        let parent = BudgetContext::new(ToolBudget::HEAVY).with_cancellation(flag);
        let mut alloc = CompositeBudgetAllocator::new(&parent, 2);
        let sub = alloc.allocate();

        let ctx = sub_budget_context(&parent, &sub);
        assert!(ctx.is_cancelled());
        assert!(ctx.should_stop());
    }

    #[test]
    fn check_not_cancelled_ok_when_not_cancelled() {
        let ctx = BudgetContext::new(ToolBudget::CHEAP);
        assert!(ctx.check_not_cancelled("test").is_ok());
    }

    #[test]
    fn check_not_cancelled_err_when_cancelled() {
        let flag = Arc::new(AtomicBool::new(true));
        let ctx = BudgetContext::new(ToolBudget::CHEAP).with_cancellation(flag);
        let err = ctx.check_not_cancelled("my_tool").unwrap_err();
        assert!(err.error.as_deref().unwrap_or("").contains("cancelled"));
    }

    #[test]
    fn check_deadline_ok_when_not_expired() {
        let ctx = BudgetContext::new(ToolBudget::CHEAP);
        assert!(ctx.check_deadline("test").is_ok());
    }

    #[test]
    fn check_deadline_err_when_expired() {
        let ctx = BudgetContext {
            deadline: Some(Instant::now() - Duration::from_millis(100)),
            budget: ToolBudget::CHEAP,
            cancelled: None,
        };
        let err = ctx.check_deadline("my_tool").unwrap_err();
        assert!(err.error.as_deref().unwrap_or("").contains("time budget"));
    }

    #[test]
    fn check_text_len_ok_when_within_limit() {
        let ctx = BudgetContext::new(ToolBudget::CHEAP);
        assert!(ctx.check_text_len("text", "hello", "test").is_ok());
    }

    #[test]
    fn check_text_len_err_when_exceeds_limit() {
        let ctx = BudgetContext::new(ToolBudget::CHEAP);
        let long_text = "x".repeat(ToolBudget::CHEAP.max_text_chars + 1);
        let err = ctx
            .check_text_len("text", &long_text, "my_tool")
            .unwrap_err();
        assert!(err.error.as_deref().unwrap_or("").contains("exceeds limit"));
    }

    #[test]
    fn check_list_len_ok_when_within_limit() {
        let ctx = BudgetContext::new(ToolBudget::CHEAP);
        assert!(ctx.check_list_len("items", 5, "test").is_ok());
    }

    #[test]
    fn check_list_len_err_when_exceeds_limit() {
        let ctx = BudgetContext::new(ToolBudget::CHEAP);
        let err = ctx
            .check_list_len("items", ToolBudget::CHEAP.max_list_items + 1, "my_tool")
            .unwrap_err();
        assert!(err.error.as_deref().unwrap_or("").contains("exceeds limit"));
    }

    #[test]
    fn remaining_time_ms_some_when_deadline_set() {
        let ctx = BudgetContext::new(ToolBudget::CHEAP);
        let remaining = ctx.remaining_time_ms();
        assert!(remaining.is_some());
        assert!(remaining.unwrap() > 0);
        assert!(remaining.unwrap() <= ToolBudget::CHEAP.max_elapsed_ms);
    }

    #[test]
    fn remaining_time_ms_none_when_no_deadline() {
        let ctx = BudgetContext {
            deadline: None,
            budget: ToolBudget::CHEAP,
            cancelled: None,
        };
        assert_eq!(ctx.remaining_time_ms(), None);
    }

    #[test]
    fn remaining_time_ms_zero_when_expired() {
        let ctx = BudgetContext {
            deadline: Some(Instant::now() - Duration::from_millis(100)),
            budget: ToolBudget::CHEAP,
            cancelled: None,
        };
        assert_eq!(ctx.remaining_time_ms(), Some(0));
    }

    // ── Thread-local cancellation flag tests ────────────────────────────

    #[test]
    fn with_cancel_flag_sets_and_restores() {
        assert!(current_cancel_flag().is_none());
        let flag = Arc::new(AtomicBool::new(true));
        let captured = with_cancel_flag(Some(flag.clone()), || current_cancel_flag().unwrap());
        assert!(Arc::ptr_eq(&captured, &flag));
        // Previous flag restored
        assert!(current_cancel_flag().is_none());
    }

    #[test]
    fn with_cancel_flag_nested_calls_restore_previous() {
        let outer = Arc::new(AtomicBool::new(false));
        let inner = Arc::new(AtomicBool::new(true));
        with_cancel_flag(Some(outer.clone()), || {
            assert!(current_cancel_flag().is_some());
            with_cancel_flag(Some(inner.clone()), || {
                assert!(Arc::ptr_eq(&current_cancel_flag().unwrap(), &inner));
            });
            // Inner scope finished — outer flag restored
            assert!(Arc::ptr_eq(&current_cancel_flag().unwrap(), &outer));
        });
        assert!(current_cancel_flag().is_none());
    }

    #[test]
    fn with_cancel_flag_none_restores_none() {
        let flag = Arc::new(AtomicBool::new(false));
        with_cancel_flag(Some(flag.clone()), || {
            assert!(current_cancel_flag().is_some());
        });
        assert!(current_cancel_flag().is_none());
    }

    #[test]
    fn for_handler_picks_up_cancel_flag() {
        let flag = Arc::new(AtomicBool::new(true));
        let ctx = with_cancel_flag(Some(flag.clone()), || for_handler(ToolBudget::CHEAP));
        assert!(ctx.is_cancelled());
        assert!(ctx.should_stop());
    }

    #[test]
    fn for_handler_without_flag_has_no_cancellation() {
        let ctx = for_handler(ToolBudget::MODERATE);
        assert!(!ctx.is_cancelled());
        assert!(!ctx.should_stop());
        assert_eq!(ctx.budget, ToolBudget::MODERATE);
    }

    #[test]
    fn for_handler_budget_is_set() {
        let ctx = for_handler(ToolBudget::HEAVY);
        assert_eq!(ctx.budget, ToolBudget::HEAVY);
        assert!(ctx.deadline.is_some());
    }
}
