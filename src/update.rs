//! Binary-first self-update support and deterministic release-contract helpers.

use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const REPOSITORY: &str = "eggstack/eggsact";
pub const CRATES_API: &str = "https://crates.io/api/v1/crates/eggsact";
pub const USER_AGENT: &str = "eggsact-self-update";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StableVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl StableVersion {
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl std::fmt::Display for StableVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseTarget {
    pub rust_target: &'static str,
    pub asset_name: &'static str,
    pub windows: bool,
}

pub const RELEASE_TARGETS: &[ReleaseTarget] = &[
    ReleaseTarget {
        rust_target: "x86_64-unknown-linux-gnu",
        asset_name: "eggsact-x86_64-unknown-linux-gnu",
        windows: false,
    },
    ReleaseTarget {
        rust_target: "aarch64-unknown-linux-gnu",
        asset_name: "eggsact-aarch64-unknown-linux-gnu",
        windows: false,
    },
    ReleaseTarget {
        rust_target: "x86_64-apple-darwin",
        asset_name: "eggsact-x86_64-apple-darwin",
        windows: false,
    },
    ReleaseTarget {
        rust_target: "aarch64-apple-darwin",
        asset_name: "eggsact-aarch64-apple-darwin",
        windows: false,
    },
    ReleaseTarget {
        rust_target: "x86_64-pc-windows-msvc",
        asset_name: "eggsact-x86_64-pc-windows-msvc.exe",
        windows: true,
    },
];

/// The ARMv7 mapping is recognized by installers but is not published until it
/// has an executable/QEMU qualification result.
pub const ARMV7_TARGET: &str = "armv7-unknown-linux-gnueabihf";

pub fn target_for_host(os: &str, arch: &str) -> Option<&'static ReleaseTarget> {
    let name = match (os, arch) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("linux", "arm") => ARMV7_TARGET,
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => return None,
    };
    RELEASE_TARGETS
        .iter()
        .find(|target| target.rust_target == name)
}

#[allow(dead_code)] // Shared contract helper exercised by mapping tests/installers.
pub fn target_name_for_installer(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64" | "amd64") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64" | "arm64") => Some("aarch64-unknown-linux-gnu"),
        ("linux", "armv7l") => Some(ARMV7_TARGET),
        ("darwin", "x86_64") => Some("x86_64-apple-darwin"),
        ("darwin", "arm64") => Some("aarch64-apple-darwin"),
        _ => None,
    }
}

pub fn asset_name(target: &str) -> String {
    if target == "x86_64-pc-windows-msvc" {
        format!("eggsact-{target}.exe")
    } else {
        format!("eggsact-{target}")
    }
}

pub fn release_asset_url(version: &StableVersion, target: &str) -> String {
    format!(
        "https://github.com/{REPOSITORY}/releases/download/v{version}/{}",
        asset_name(target)
    )
}

#[allow(dead_code)] // Documents and tests the installer URL contract.
pub fn latest_asset_url(target: &str) -> String {
    format!(
        "https://github.com/{REPOSITORY}/releases/latest/download/{}",
        asset_name(target)
    )
}

pub fn checksum_url(binary_url: &str) -> String {
    format!("{binary_url}.sha256")
}

pub fn parse_stable_version(raw: &str) -> Result<StableVersion, String> {
    let mut parts = raw.trim().split('.');
    let values = [parts.next(), parts.next(), parts.next()];
    if parts.next().is_some() || values.iter().any(|part| part.is_none()) {
        return Err(format!("invalid stable version '{raw}' (expected X.Y.Z)"));
    }
    let mut parsed = [0_u64; 3];
    for (slot, part) in parsed.iter_mut().zip(values.into_iter().flatten()) {
        if part.is_empty() || (part.len() > 1 && part.starts_with('0')) {
            return Err(format!("invalid stable version '{raw}' (expected X.Y.Z)"));
        }
        *slot = part
            .parse()
            .map_err(|_| format!("invalid stable version '{raw}' (expected X.Y.Z)"))?;
    }
    Ok(StableVersion::new(parsed[0], parsed[1], parsed[2]))
}

pub fn parse_candidate_version(output: &str) -> Result<StableVersion, String> {
    let trimmed = output.trim();
    let version = trimmed
        .strip_prefix("eggsact ")
        .ok_or_else(|| format!("candidate reported unexpected version output '{trimmed}'"))?;
    if trimmed != format!("eggsact {version}") {
        return Err(format!(
            "candidate reported unexpected version output '{trimmed}'"
        ));
    }
    parse_stable_version(version)
}

pub fn parse_checksum(raw: &str) -> Result<[u8; 32], String> {
    let token = raw.split_whitespace().next().unwrap_or_default();
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("checksum sidecar does not begin with a 64-hex SHA-256 digest".into());
    }
    let mut digest = [0_u8; 32];
    for (index, slot) in digest.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&token[index * 2..index * 2 + 2], 16)
            .map_err(|_| "checksum sidecar contains invalid hexadecimal".to_string())?;
    }
    Ok(digest)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    Success,
    NotFound,
    HardFailure,
}

pub fn classify_http_status(status: u16) -> DownloadStatus {
    match status {
        200..=299 => DownloadStatus::Success,
        404 => DownloadStatus::NotFound,
        _ => DownloadStatus::HardFailure,
    }
}

fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
    let base = env::temp_dir();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock error: {error}"))?
        .as_nanos();
    for attempt in 0..32_u32 {
        let path = base.join(format!("{prefix}-{}-{now}-{attempt}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                        .map_err(|error| format!("cannot secure temporary directory: {error}"))?;
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("cannot create temporary directory: {error}")),
        }
    }
    Err("could not create a unique temporary directory".into())
}

fn download(url: &str, destination: &Path) -> Result<DownloadStatus, String> {
    let output = Command::new("curl")
        .args([
            "--proto",
            "=https",
            "--tlsv1.2",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--connect-timeout",
            "10",
            "--max-time",
            "120",
            "--user-agent",
            USER_AGENT,
            "--output",
        ])
        .arg(destination)
        .args(["--write-out", "%{http_code}", url])
        .output()
        .map_err(|error| format!("cannot run curl: {error}"))?;
    let status = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u16>()
        .unwrap_or(0);
    if output.status.success() {
        return Ok(classify_http_status(status));
    }
    Ok(if status == 404 {
        DownloadStatus::NotFound
    } else {
        DownloadStatus::HardFailure
    })
}

fn download_required(url: &str, destination: &Path, what: &str) -> Result<(), String> {
    match download(url, destination)? {
        DownloadStatus::Success => Ok(()),
        DownloadStatus::NotFound => Err(format!("{what} was not found at {url}")),
        DownloadStatus::HardFailure => Err(format!("failed to download {what} from {url}")),
    }
}

fn run_bounded(mut command: Command, timeout: Duration) -> Result<std::process::Output, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot execute candidate: {error}"))?;
    wait_bounded(&mut child, timeout)?;
    child
        .wait_with_output()
        .map_err(|error| format!("cannot collect candidate output: {error}"))
}

fn wait_bounded(child: &mut Child, timeout: Duration) -> Result<(), String> {
    let started = SystemTime::now();
    loop {
        if child
            .try_wait()
            .map_err(|error| format!("cannot poll candidate: {error}"))?
            .is_some()
        {
            return Ok(());
        }
        if started.elapsed().unwrap_or(timeout) >= timeout {
            let _ = child.kill();
            return Err("candidate execution exceeded the 10-second timeout".into());
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn sha256_file(path: &Path) -> Result<[u8; 32], String> {
    let mut file = File::open(path).map_err(|error| format!("cannot read candidate: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot hash candidate: {error}"))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().into())
}

fn crates_latest_version() -> Result<StableVersion, String> {
    let staging = unique_temp_dir("eggsact-update-metadata")?;
    let path = staging.join("crate.json");
    let result = (|| {
        download_required(CRATES_API, &path, "crates.io metadata")?;
        let mut text = String::new();
        File::open(&path)
            .map_err(|error| format!("cannot open crates.io metadata: {error}"))?
            .read_to_string(&mut text)
            .map_err(|error| format!("cannot read crates.io metadata: {error}"))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("invalid crates.io metadata: {error}"))?;
        let version = json
            .get("crate")
            .and_then(|value| value.get("max_stable_version"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "crates.io metadata has no max_stable_version".to_string())?;
        parse_stable_version(version)
    })();
    let _ = fs::remove_dir_all(staging);
    result
}

fn cargo_candidate(staging: &Path, version: StableVersion) -> Result<PathBuf, String> {
    let root = staging.join("cargo-root");
    let mut command = Command::new("cargo");
    command
        .args(["install", "eggsact", "--locked", "--root"])
        .arg(&root)
        .args(["--version", &format!("={version}")]);
    let status = command
        .status()
        .map_err(|error| format!("cannot run cargo install: {error}"))?;
    if !status.success() {
        return Err(format!("cargo install exited with {status}"));
    }
    let path = root.join("bin").join(if cfg!(windows) {
        "eggsact.exe"
    } else {
        "eggsact"
    });
    if !path.is_file() {
        return Err("cargo install produced no eggsact executable".into());
    }
    Ok(path)
}

fn prepare_candidate(
    staging: &Path,
    target: &ReleaseTarget,
    latest: StableVersion,
) -> Result<PathBuf, String> {
    let binary = staging.join(target.asset_name);
    let checksum = staging.join("candidate.sha256");
    let binary_url = release_asset_url(&latest, target.rust_target);
    match download(&binary_url, &binary)? {
        DownloadStatus::NotFound => return cargo_candidate(staging, latest),
        DownloadStatus::HardFailure => {
            return Err(format!(
                "failed to download release asset from {binary_url}"
            ))
        }
        DownloadStatus::Success => {}
    }
    download_required(&checksum_url(&binary_url), &checksum, "release checksum")?;
    let mut checksum_text = String::new();
    File::open(&checksum)
        .map_err(|error| format!("cannot open checksum: {error}"))?
        .read_to_string(&mut checksum_text)
        .map_err(|error| format!("cannot read checksum: {error}"))?;
    let expected = parse_checksum(&checksum_text)?;
    let actual = sha256_file(&binary)?;
    if expected != actual {
        return Err("release checksum does not match the downloaded executable".into());
    }
    Ok(binary)
}

#[cfg(windows)]
fn replace_current(candidate: &Path, current: &Path) -> Result<ReplacementOutcome, String> {
    let pid = std::process::id();
    let adjacent = current.with_extension(format!("eggsact-update-{pid}.exe"));
    fs::copy(candidate, &adjacent).map_err(|error| permission_error(current, error))?;
    let status = current.with_extension(format!("eggsact-update-{pid}.status"));
    let script = windows_replacement_script(pid, &adjacent, current, &status);
    Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot schedule Windows executable replacement: {error}"))?;
    Ok(ReplacementOutcome::Staged {
        status_path: status,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // The staged variant is constructed only on Windows.
enum ReplacementOutcome {
    Complete,
    Staged { status_path: PathBuf },
}

#[cfg(unix)]
fn replace_current(candidate: &Path, current: &Path) -> Result<ReplacementOutcome, String> {
    let adjacent = current.with_extension(format!("eggsact-update-{}", std::process::id()));
    fs::copy(candidate, &adjacent).map_err(|error| permission_error(current, error))?;
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&adjacent, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("cannot make staged update executable: {error}"))?;
    }
    fs::rename(&adjacent, current).map_err(|error| permission_error(current, error))?;
    Ok(ReplacementOutcome::Complete)
}

#[allow(dead_code)] // Used by the Windows-only replacement path.
fn powershell_quote(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

#[allow(dead_code)] // Used by the Windows-only replacement path and cross-platform tests.
fn windows_replacement_script(pid: u32, source: &Path, target: &Path, status: &Path) -> String {
    let source = powershell_quote(source);
    let target = powershell_quote(target);
    let status = powershell_quote(status);
    format!(
        "$p={pid}; $source='{source}'; $target='{target}'; $status='{status}'; $status_tmp=\"$status.tmp\"; function Write-UpdateStatus([string]$value) {{ Set-Content -LiteralPath $status_tmp -Value $value -NoNewline; Move-Item -LiteralPath $status_tmp -Destination $status -Force }}; try {{ while (Get-Process -Id $p -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 100 }}; $last_error='replacement did not complete'; for ($attempt=0; $attempt -lt 50; $attempt++) {{ try {{ Move-Item -LiteralPath $source -Destination $target -Force -ErrorAction Stop; Remove-Item -LiteralPath $status -Force -ErrorAction SilentlyContinue; exit 0 }} catch {{ $last_error=$_.Exception.Message; Start-Sleep -Milliseconds 100 }} }}; Write-UpdateStatus(\"failed: $last_error\"); exit 1 }} catch {{ try {{ Write-UpdateStatus(\"failed: $($_.Exception.Message)\") }} catch {{ }}; exit 1 }}"
    )
}

fn permission_error(path: &Path, error: io::Error) -> String {
    if cfg!(windows) {
        format!("cannot replace {}: {error}. Close active MCP clients and retry from an Administrator PowerShell.", path.display())
    } else {
        format!(
            "cannot replace {}: {error}. Retry with: {}",
            path.display(),
            retry_command(path)
        )
    }
}

#[allow(dead_code)] // Also serves callers that preflight a permission retry.
pub fn retry_command(path: &Path) -> String {
    if cfg!(windows) {
        "Run PowerShell as Administrator and retry `eggsact update`.".into()
    } else {
        format!("sudo {} update", path.display())
    }
}

pub fn run() -> Result<(), String> {
    let current =
        env::current_exe().map_err(|error| format!("cannot locate current executable: {error}"))?;
    let current_version = parse_stable_version(env!("CARGO_PKG_VERSION"))?;
    let latest = crates_latest_version()?;
    if latest <= current_version {
        println!("eggsact {current_version} is already current (latest stable: {latest})");
        return Ok(());
    }
    let staging = unique_temp_dir("eggsact-update")?;
    let result = (|| {
        let candidate = if let Some(target) = target_for_host(env::consts::OS, env::consts::ARCH) {
            prepare_candidate(&staging, target, latest)?
        } else {
            cargo_candidate(&staging, latest)?
        };
        let mut version_command = Command::new(&candidate);
        version_command.arg("--version");
        let output = run_bounded(version_command, Duration::from_secs(10))?;
        if !output.status.success() {
            return Err("candidate --version failed".into());
        }
        let reported = parse_candidate_version(&String::from_utf8_lossy(&output.stdout))?;
        if reported != latest {
            return Err(format!("candidate reported {reported}, expected {latest}"));
        }
        let replacement = replace_current(&candidate, &current)?;
        let _ = fs::remove_dir_all(&staging);
        Ok(replacement)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    match result? {
        ReplacementOutcome::Complete => {
            println!("updated eggsact from {current_version} to {latest}");
            println!("new MCP launches use the new version; existing stdio sessions may continue using the prior image until their client reconnects.");
        }
        ReplacementOutcome::Staged { status_path } => {
            println!("update staged from {current_version} to {latest}; replacement will complete after this process exits.");
            println!("If replacement fails, read {} and close active MCP clients before retrying from an Administrator PowerShell.", status_path.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_mapping_is_explicit() {
        assert_eq!(
            target_for_host("linux", "x86_64").unwrap().rust_target,
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            target_for_host("linux", "aarch64").unwrap().rust_target,
            "aarch64-unknown-linux-gnu"
        );
        assert!(target_for_host("linux", "arm").is_none());
        assert_eq!(
            target_name_for_installer("linux", "armv7l"),
            Some(ARMV7_TARGET)
        );
        assert!(target_for_host("freebsd", "x86_64").is_none());
    }

    #[test]
    fn asset_urls_are_exact_and_versionless_in_names() {
        let version = StableVersion::new(1, 2, 4);
        assert_eq!(
            asset_name("x86_64-pc-windows-msvc"),
            "eggsact-x86_64-pc-windows-msvc.exe"
        );
        assert_eq!(release_asset_url(&version, "x86_64-unknown-linux-gnu"), "https://github.com/eggstack/eggsact/releases/download/v1.2.4/eggsact-x86_64-unknown-linux-gnu");
        assert_eq!(latest_asset_url("aarch64-apple-darwin"), "https://github.com/eggstack/eggsact/releases/latest/download/eggsact-aarch64-apple-darwin");
    }

    #[test]
    fn versions_and_candidate_output_are_strict() {
        assert_eq!(
            parse_stable_version("1.2.3").unwrap(),
            StableVersion::new(1, 2, 3)
        );
        assert!(parse_stable_version("1.2.3-beta").is_err());
        assert_eq!(
            parse_candidate_version("eggsact 1.2.3\n").unwrap(),
            StableVersion::new(1, 2, 3)
        );
        assert!(parse_candidate_version("other 1.2.3\n").is_err());
    }

    #[test]
    fn checksum_and_http_classification_are_bounded() {
        assert_eq!(classify_http_status(404), DownloadStatus::NotFound);
        assert_eq!(classify_http_status(503), DownloadStatus::HardFailure);
        assert_eq!(classify_http_status(200), DownloadStatus::Success);
        assert!(parse_checksum("not-a-checksum").is_err());
        assert!(parse_checksum(&format!("{}  file", "a".repeat(64))).is_ok());
    }

    #[test]
    fn windows_replacement_script_waits_and_records_failure_without_killing() {
        let script = windows_replacement_script(
            42,
            Path::new(r"C:\Program Files\Eggsact\candidate.exe"),
            Path::new(r"C:\Program Files\Eggsact\eggsact.exe"),
            Path::new(r"C:\Program Files\Eggsact\eggsact.status"),
        );
        assert!(script.contains("Get-Process -Id $p"));
        assert!(script.contains("Move-Item -LiteralPath $source"));
        assert!(script.contains("Write-UpdateStatus(\"failed:"));
        assert!(!script.contains("Stop-Process"));
        assert!(!script.contains("taskkill"));
    }
}
