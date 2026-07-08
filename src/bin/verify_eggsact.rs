use std::env;
use std::fs;
use std::process::Command;
use std::time::Instant;

enum StepStatus {
    Pass(f64),
    Fail(f64),
    Skip,
}

impl StepStatus {
    fn duration_secs(&self) -> f64 {
        match self {
            StepStatus::Pass(d) | StepStatus::Fail(d) => *d,
            StepStatus::Skip => 0.0,
        }
    }
}

fn run_cmd(cmd: &str, args: &[&str]) -> StepStatus {
    let start = Instant::now();
    let status = Command::new(cmd)
        .args(args)
        .env("EGGCALC_NO_CONFIG", "1")
        .status();
    let elapsed = start.elapsed().as_secs_f64();
    match status {
        Ok(s) if s.success() => StepStatus::Pass(elapsed),
        Ok(_) => StepStatus::Fail(elapsed),
        Err(e) => {
            eprintln!("error: failed to run {cmd}: {e}");
            StepStatus::Fail(elapsed)
        }
    }
}

fn short_head() -> String {
    let out = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    }
}

fn parity_available() -> bool {
    fs::metadata("../eggcalc").is_ok()
}

fn format_duration(secs: f64) -> String {
    if secs < 1.0 {
        format!("{:.0}ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        format!("{:.0}m {:.0}s", secs / 60.0, secs % 60.0)
    }
}

fn status_badge(s: &StepStatus) -> &str {
    match s {
        StepStatus::Pass(_) => "`PASS`",
        StepStatus::Fail(_) => "`FAIL`",
        StepStatus::Skip => "`SKIP`",
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let report_format = args
        .windows(2)
        .find(|w| w[0] == "--report")
        .map(|w| w[1].as_str())
        .unwrap_or("markdown");

    if report_format != "markdown" {
        eprintln!("warning: --report {report_format} not supported; falling back to markdown");
    }

    let commit = short_head();
    let has_parity = parity_available();

    // Run steps
    let fmt = run_cmd("cargo", &["fmt", "--all", "--", "--check"]);
    let clippy = run_cmd(
        "cargo",
        &[
            "clippy",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    );
    let test_lib = run_cmd("cargo", &["test", "--all-features", "--lib"]);
    let test_bins = run_cmd("cargo", &["test", "--all-features", "--bins"]);
    let test_integration = run_cmd(
        "cargo",
        &[
            "test",
            "--all-features",
            "--tests",
            "--",
            "--skip",
            "parity",
        ],
    );
    let test_doc = run_cmd("cargo", &["test", "--doc"]);
    let docs = run_cmd("cargo", &["run", "--bin", "generate-docs", "--", "--check"]);
    let package = run_cmd("cargo", &["package", "--verbose"]);
    let parity = if has_parity {
        run_cmd(
            "cargo",
            &["test", "--test", "lib", "parity", "--all-features"],
        )
    } else {
        StepStatus::Skip
    };

    // Determine overall result
    let all_steps: Vec<(&str, &StepStatus)> = vec![
        ("cargo fmt", &fmt),
        ("cargo clippy", &clippy),
        ("cargo test --lib", &test_lib),
        ("cargo test --bins", &test_bins),
        ("cargo test --tests (skip parity)", &test_integration),
        ("cargo test --doc", &test_doc),
        ("generate-docs --check", &docs),
        ("cargo package", &package),
        ("parity tests", &parity),
    ];
    let failed = all_steps
        .iter()
        .any(|(_, s)| matches!(s, StepStatus::Fail(_)));

    // Emit report
    println!("# Eggsact Verification Report");
    println!();
    println!("**commit:** `{commit}`");
    println!("**generated-docs freshness:** {}", status_badge(&docs));
    println!(
        "**parity availability:** {}",
        if has_parity {
            "available"
        } else {
            "unavailable (`../eggcalc` not found) — parity tests skipped"
        }
    );
    println!();
    println!("## Results");
    println!();
    println!("| Step | Status | Duration |");
    println!("|------|--------|----------|");
    for (name, status) in &all_steps {
        let dur = if matches!(status, StepStatus::Skip) {
            "-".to_string()
        } else {
            format_duration(status.duration_secs())
        };
        println!("| {} | {} | {} |", name, status_badge(status), dur);
    }
    println!();

    if !failed {
        println!("**Overall: PASS**");
    } else {
        println!("**Overall: FAIL**");
    }

    let exit_code: i32 = if failed { 1 } else { 0 };
    std::process::exit(exit_code);
}
