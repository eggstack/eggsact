//! Binary-first self-update support and deterministic release-contract helpers.
//!
//! Network transport is owned in-process by `eggfetch-core` 0.1.6 with the
//! minimal feature set `http1,tls-rustls,tls-native-roots,proxy`. The updater
//! keeps all release/update policy (version selection, asset naming, Cargo
//! fallback, SHA-256 verification, candidate validation, replacement).
//!
//! Transport policy (single configuration point in `build_update_client_*`):
//! - user agent `eggsact-self-update`;
//! - HTTP/1 only (`HttpVersionPolicy::Http1Only`);
//! - redirects followed with strict HTTPS -> HTTP downgrade rejection
//!   (`RedirectPolicy::strict`, `DowngradePolicy::Deny` before second-hop I/O);
//! - native roots with packaged WebPKI fallback (eggfetch default) and full
//!   certificate/hostname verification;
//! - explicit opt-in environment proxy routing (`ProxyEnvironment::from_env`);
//!   invalid proxy configuration fails closed, never silently direct;
//! - 10-second connect timeout + 120-second total wall-clock timeout
//!   (distinct phases via `Timeout::builder`, never `from_secs(120)` alone);
//! - no retry policy; no compression/cookies/multipart/JSON features;
//! - release binaries stream to disk chunk-by-chunk, never buffered as one
//!   `Vec<u8>` by updater code.

use eggfetch_core::{Client, HttpVersionPolicy, ProxyEnvironment, RedirectPolicy, Timeout};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;

pub const REPOSITORY: &str = "eggstack/eggsact";
pub const CRATES_API: &str = "https://crates.io/api/v1/crates/eggsact";
pub const USER_AGENT: &str = "eggsact-self-update";

/// Distinct connect deadline preserved from the historical `curl`
/// `--connect-timeout 10` contract.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Total wall-clock cap preserved from the historical `curl --max-time 120`
/// contract. Enforced alongside (not instead of) [`CONNECT_TIMEOUT`].
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(120);
/// Bounded redirect following for GitHub asset chains (normally 1-2 hops).
pub const MAX_REDIRECTS: usize = 10;
/// Conservative bound for the small crates.io metadata document. The normal
/// payload is a few kilobytes; 1 MiB is comfortably above it while preventing
/// unbounded buffering from a malformed server/proxy response.
pub const METADATA_MAX_BYTES: usize = 1_048_576;
/// Conservative bound for the tiny SHA-256 sidecar (normally ~100 bytes:
/// 64 hex digits plus whitespace/filename).
pub const CHECKSUM_MAX_BYTES: usize = 65_536;

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

// ── Updater transport adapter (private to self-update) ────────────────

/// Distinct 10s connect + 120s total updater deadlines. Never collapse to a
/// single `Timeout::from_secs(120)`.
fn update_timeout() -> Timeout {
    Timeout::builder()
        .connect(CONNECT_TIMEOUT)
        .total(TOTAL_TIMEOUT)
        .build()
}

/// Follow redirects but reject HTTPS -> HTTP downgrades before second-hop I/O.
fn update_redirect_policy() -> RedirectPolicy {
    RedirectPolicy::strict(MAX_REDIRECTS)
}

fn redact_url_for_error(url: &str) -> String {
    eggfetch_core::redact_url_string(url)
}

/// Map an eggfetch transport error into the updater's concise,
/// credential-safe application message.
fn map_transport_error(error: eggfetch_core::Error, url: &str, what: &str) -> String {
    use eggfetch_core::Error as E;
    let redacted = redact_url_for_error(url);
    let detail = error.to_string();
    match error {
        E::Timeout { phase, elapsed } => format!(
            "update request timed out ({phase} after {elapsed:?}) while fetching {what} from {redacted}"
        ),
        E::TransportIoTimeout { .. } => {
            format!("update request timed out while fetching {what} from {redacted}: {detail}")
        }
        E::Tls(_)
        | E::TlsConfig(_)
        | E::CaBundle(_)
        | E::ClientCert(_)
        | E::PrivateKey(_)
        | E::CertificateVerification(_)
        | E::HostnameVerification(_) => {
            format!("TLS/certificate failure while fetching {what} from {redacted}: {detail}")
        }
        E::InvalidProxyUrl(_)
        | E::ProxyConnect(_)
        | E::ProxyAuthRequired
        | E::ProxyConnectRejected { .. }
        | E::MalformedProxyResponse(_) => {
            format!("proxy configuration/routing failure while fetching {what}: {detail}")
        }
        E::Connect(_) | E::Hyper(_) | E::HyperClient(_) | E::Pool(_) | E::CustomTransport(_) => {
            format!("cannot resolve/connect to update endpoint {redacted} while fetching {what}: {detail}")
        }
        E::InvalidRedirectLocation(_)
        | E::TooManyRedirects { .. }
        | E::BodyNotReplayableForRedirect => {
            format!("redirect failure while fetching {what} from {redacted}: {detail}")
        }
        E::InvalidUrl(_) | E::RequestBuild(_) | E::InvalidResolvedTarget(_) => {
            format!("invalid update request for {what} from {redacted}: {detail}")
        }
        E::Body(_)
        | E::DecodedBodyTooLarge
        | E::Decompression(_)
        | E::DecompressionRatioExceeded
        | E::UnsupportedContentEncoding(_)
        | E::Protocol(_)
        | E::Io(_) => {
            format!("failed while reading release asset ({what}) from {redacted}: {detail}")
        }
        _ => format!("failed to download {what} from {redacted}: {detail}"),
    }
}

/// Single configuration point for TLS/redirect/timeout/proxy behavior.
fn build_update_client_with_timeout_and_env(
    timeout: Timeout,
    env: &ProxyEnvironment,
) -> Result<Client, String> {
    let builder = Client::builder()
        .user_agent(USER_AGENT)
        .http_version_policy(HttpVersionPolicy::Http1Only)
        .redirect_policy(update_redirect_policy())
        .timeout(timeout)
        .automatic_decompression(false);
    let builder = builder
        .proxy_environment(env)
        .map_err(|error| format!("proxy configuration/routing failure: {error}"))?;
    Ok(builder.build())
}

fn build_update_client_with_env(env: &ProxyEnvironment) -> Result<Client, String> {
    build_update_client_with_timeout_and_env(update_timeout(), env)
}

fn build_update_client() -> Result<Client, String> {
    build_update_client_with_env(&ProxyEnvironment::from_env())
}

/// Fetch a small text document (crates.io metadata, checksum sidecar) with an
/// explicit byte bound. Small responses may be buffered; the bound prevents
/// unbounded memory use from a malformed server/proxy response.
async fn get_small_text(
    client: &Client,
    url: &str,
    max_bytes: usize,
    what: &str,
) -> Result<String, String> {
    let redacted = redact_url_for_error(url);
    let mut response = client
        .get(url)
        .map_err(|error| map_transport_error(error, url, what))?
        .send()
        .await
        .map_err(|error| map_transport_error(error, url, what))?;
    let status = response.status().as_u16();
    if !matches!(classify_http_status(status), DownloadStatus::Success) {
        return Err(format!(
            "HTTP {status} from {redacted} while fetching {what}"
        ));
    }
    if let Some(declared) = response.content_length() {
        if declared > max_bytes as u64 {
            return Err(format!(
                "{what} from {redacted} exceeds {max_bytes}-byte bound (declared {declared} bytes)"
            ));
        }
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| map_transport_error(error, url, what))?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "{what} from {redacted} exceeds {max_bytes}-byte bound (received {} bytes)",
            bytes.len()
        ));
    }
    String::from_utf8(bytes.to_vec())
        .map_err(|error| format!("invalid UTF-8 in {what} from {redacted}: {error}"))
}

/// Stream a release binary to disk chunk-by-chunk. Never buffers the
/// executable body into one `Vec<u8>`/`Bytes` allocation in updater code.
/// On any body/read/write failure the partial destination is removed so
/// `prepare_candidate` cannot accidentally consume it.
async fn download_to(
    client: &Client,
    url: &str,
    destination: &Path,
) -> Result<DownloadStatus, String> {
    let mut response = client
        .get(url)
        .map_err(|error| map_transport_error(error, url, "release asset"))?
        .send()
        .await
        .map_err(|error| map_transport_error(error, url, "release asset"))?;
    let status = response.status().as_u16();
    match classify_http_status(status) {
        DownloadStatus::NotFound => return Ok(DownloadStatus::NotFound),
        DownloadStatus::HardFailure => {
            let redacted = redact_url_for_error(url);
            return Err(format!(
                "HTTP {status} from {redacted} while fetching release asset"
            ));
        }
        DownloadStatus::Success => {}
    }
    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|error| {
            format!(
                "failed while writing staged release asset {}: {error}",
                destination.display()
            )
        })?;
    let mut stream = response
        .bytes_stream()
        .map_err(|error| map_transport_error(error, url, "release asset"))?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| {
            let _ = std::fs::remove_file(destination);
            map_transport_error(error, url, "release asset")
        })?;
        if chunk.is_empty() {
            continue;
        }
        file.write_all(&chunk).await.map_err(|error| {
            let _ = std::fs::remove_file(destination);
            format!(
                "failed while writing staged release asset {}: {error}",
                destination.display()
            )
        })?;
    }
    file.flush().await.map_err(|error| {
        let _ = std::fs::remove_file(destination);
        format!(
            "failed while writing staged release asset {}: {error}",
            destination.display()
        )
    })?;
    drop(file);
    Ok(DownloadStatus::Success)
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

async fn crates_latest_version_async(client: &Client) -> Result<StableVersion, String> {
    let text = get_small_text(client, CRATES_API, METADATA_MAX_BYTES, "crates.io metadata").await?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("invalid crates.io metadata: {error}"))?;
    let version = json
        .get("crate")
        .and_then(|value| value.get("max_stable_version"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "crates.io metadata has no max_stable_version".to_string())?;
    parse_stable_version(version)
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

async fn prepare_candidate_async(
    client: &Client,
    staging: &Path,
    target: &ReleaseTarget,
    latest: StableVersion,
) -> Result<PathBuf, String> {
    let binary = staging.join(target.asset_name);
    let binary_url = release_asset_url(&latest, target.rust_target);
    match download_to(client, &binary_url, &binary).await? {
        DownloadStatus::NotFound => return cargo_candidate(staging, latest),
        DownloadStatus::HardFailure => {
            return Err(format!(
                "failed to download release asset from {}",
                redact_url_for_error(&binary_url)
            ))
        }
        DownloadStatus::Success => {}
    }
    // A missing or failed checksum sidecar is fatal: do not Cargo-fallback
    // after the binary itself was found.
    let checksum_text = get_small_text(
        client,
        &checksum_url(&binary_url),
        CHECKSUM_MAX_BYTES,
        "release checksum",
    )
    .await?;
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
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .thread_name("eggsact-update")
        .build()
        .map_err(|error| format!("cannot start update runtime: {error}"))?;
    runtime.block_on(run_async())
}

async fn run_async() -> Result<(), String> {
    let current =
        env::current_exe().map_err(|error| format!("cannot locate current executable: {error}"))?;
    let current_version = parse_stable_version(env!("CARGO_PKG_VERSION"))?;
    let client = build_update_client()?;
    let latest = crates_latest_version_async(&client).await?;
    if latest <= current_version {
        println!("eggsact {current_version} is already current (latest stable: {latest})");
        return Ok(());
    }
    let staging = unique_temp_dir("eggsact-update")?;
    let result = async {
        let candidate = if let Some(target) = target_for_host(env::consts::OS, env::consts::ARCH) {
            prepare_candidate_async(&client, &staging, target, latest).await?
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
    }
    .await;
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
    use std::net::TcpListener;
    use std::sync::Arc;

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

    #[test]
    fn updater_uses_distinct_connect_and_total_timeouts() {
        let timeout = update_timeout();
        assert_eq!(timeout.connect, Some(CONNECT_TIMEOUT));
        assert_eq!(timeout.total, Some(TOTAL_TIMEOUT));
        assert_eq!(timeout.connect, Some(Duration::from_secs(10)));
        assert_eq!(timeout.total, Some(Duration::from_secs(120)));
    }

    #[test]
    fn updater_selects_strict_redirect_policy() {
        let policy = update_redirect_policy();
        assert!(policy.follow);
        assert_eq!(policy.max_redirects, MAX_REDIRECTS);
        assert_eq!(
            policy.downgrade,
            eggfetch_core::RedirectDowngradePolicy::Deny
        );
        // Relative HTTPS-equivalent hops stay allowed under strict mode;
        // only an explicit https -> http downgrade is rejected.
        let from: url::Url = "https://example.com/a".parse().unwrap();
        let same: url::Url = "https://example.com/b".parse().unwrap();
        let down: url::Url = "http://example.com/b".parse().unwrap();
        assert!(policy.check_downgrade(&from, &same).is_ok());
        assert!(policy.check_downgrade(&from, &down).is_err());
    }

    #[test]
    fn proxy_environment_selection_is_explicit_and_testable() {
        use eggfetch_core::ProxyEnvironment;
        let env = ProxyEnvironment::from_map([("HTTPS_PROXY", "http://proxy.example:8080")]);
        let url: url::Url = "https://example.com/a".parse().unwrap();
        assert!(env.resolve(&url).unwrap().is_some());
        let bypass = ProxyEnvironment::from_map([
            ("HTTPS_PROXY", "http://proxy.example:8080"),
            ("NO_PROXY", "example.com"),
        ]);
        assert!(bypass.resolve(&url).unwrap().is_none());
        // Production constructor opts into the same explicit snapshot path.
        let client = build_update_client_with_env(&ProxyEnvironment::new());
        assert!(client.is_ok());
        let invalid = ProxyEnvironment::from_map([("HTTPS_PROXY", "http://user:bogus@[::1")]);
        assert!(build_update_client_with_env(&invalid).is_err());
    }

    #[test]
    fn updater_transport_never_spawns_curl() {
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/update.rs"));
        assert!(
            !source.contains("Command::new(\"curl\")"),
            "self-update transport must not spawn curl"
        );
        assert!(
            !source.contains("\"curl\""),
            "self-update transport must not reference a curl executable"
        );
    }

    #[test]
    fn updater_error_mapping_stays_concise_and_redacted() {
        let secret_url = "https://user:s3cret@example.com/releases/asset";
        let timeout = eggfetch_core::Error::Timeout {
            phase: eggfetch_core::TimeoutPhase::Connect,
            elapsed: Duration::from_secs(10),
        };
        let message = map_transport_error(timeout, secret_url, "release asset");
        assert!(message.contains("update request timed out"));
        assert!(!message.contains("s3cret"));

        let tls = eggfetch_core::Error::CertificateVerification("bad chain".into());
        let message = map_transport_error(tls, secret_url, "release asset");
        assert!(message.contains("TLS/certificate failure"));
        assert!(!message.contains("s3cret"));

        let proxy = eggfetch_core::Error::InvalidProxyUrl("proxy <redacted>".into());
        let message = map_transport_error(proxy, secret_url, "release asset");
        assert!(message.contains("proxy configuration/routing failure"));

        let connect = eggfetch_core::Error::Connect("refused".into());
        let message = map_transport_error(connect, secret_url, "release asset");
        assert!(message.contains("cannot resolve/connect to update endpoint"));
    }

    // ── Minimal local HTTP harness (no extra production dependencies) ──

    /// Serve exactly one response per connection: `handler` receives the raw
    /// request head and returns `(status, headers, body_chunks, abort_after_headers)`.
    /// When `abort_after_headers` is true the connection is closed mid-body.
    fn serve<F>(handler: F) -> (String, tokio::task::JoinHandle<()>)
    where
        F: Fn(String) -> (u16, Vec<(String, String)>, Vec<Vec<u8>>, bool) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test server");
        let addr = listener.local_addr().expect("local addr");
        listener
            .set_nonblocking(true)
            .expect("test listener nonblocking");
        let handler = Arc::new(handler);
        let handle = tokio::spawn(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let handler = Arc::clone(&handler);
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut head = Vec::new();
                    let mut byte = [0_u8; 1];
                    // Read until end of headers.
                    loop {
                        match socket.read(&mut byte).await {
                            Ok(0) => break,
                            Ok(_) => {
                                head.push(byte[0]);
                                if head.len() > 16_384 {
                                    break;
                                }
                                if head.ends_with(b"\r\n\r\n") {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    let request = String::from_utf8_lossy(&head).into_owned();
                    let (status, headers, chunks, abort) = handler(request);
                    let reason = match status {
                        200 => "OK",
                        204 => "No Content",
                        301 => "Moved Permanently",
                        302 => "Found",
                        404 => "Not Found",
                        500 => "Internal Server Error",
                        503 => "Service Unavailable",
                        _ => "Response",
                    };
                    let mut response =
                        format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\n");
                    for (name, value) in headers {
                        response.push_str(&format!("{name}: {value}\r\n"));
                    }
                    response.push_str("\r\n");
                    if socket.write_all(response.as_bytes()).await.is_err() {
                        return;
                    }
                    if abort {
                        return;
                    }
                    for chunk in chunks {
                        if socket.write_all(&chunk).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        (format!("http://{addr}/"), handle)
    }

    fn test_client() -> Client {
        build_update_client_with_env(&eggfetch_core::ProxyEnvironment::new())
            .expect("test client builds without proxy environment")
    }

    fn temp_destination(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "eggsact-update-test-{}-{}-{name}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        path
    }

    #[tokio::test]
    async fn download_status_classification_round_trips_local_server() {
        // 200 with exact body.
        let (base, handle) = serve(|_| {
            (
                200,
                vec![("Content-Length".into(), "11".into())],
                vec![b"hello world".to_vec()],
                false,
            )
        });
        let client = test_client();
        let dest = temp_destination("ok");
        let status = download_to(&client, &base, &dest)
            .await
            .expect("200 downloads");
        assert_eq!(status, DownloadStatus::Success);
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello world");
        let _ = std::fs::remove_file(&dest);
        handle.abort();

        // 204 empty success.
        let (base, handle) = serve(|_| (204, vec![], vec![], false));
        let dest = temp_destination("empty");
        let status = download_to(&client, &base, &dest)
            .await
            .expect("204 downloads");
        assert_eq!(status, DownloadStatus::Success);
        assert_eq!(std::fs::read(&dest).unwrap(), b"");
        let _ = std::fs::remove_file(&dest);
        handle.abort();

        // 404 maps to NotFound (Cargo fallback signal), no file created.
        let (base, handle) = serve(|_| {
            (
                404,
                vec![("Content-Length".into(), "9".into())],
                vec![b"not found".to_vec()],
                false,
            )
        });
        let dest = temp_destination("missing");
        let _ = std::fs::remove_file(&dest);
        let status = download_to(&client, &base, &dest).await.expect("404 maps");
        assert_eq!(status, DownloadStatus::NotFound);
        assert!(!dest.exists());
        handle.abort();

        // 500/503 are hard failures with an HTTP status message, no fallback.
        for code in [500_u16, 503] {
            let (base, handle) = serve(move |_| {
                (
                    code,
                    vec![("Content-Length".into(), "5".into())],
                    vec![b"error".to_vec()],
                    false,
                )
            });
            let dest = temp_destination("hard");
            let error = download_to(&client, &base, &dest)
                .await
                .expect_err("hard failure");
            assert!(error.contains(&format!("HTTP {code}")), "got: {error}");
            assert!(!dest.exists());
            handle.abort();
        }

        // Body abort after headers is a hard failure with no consumable file.
        let (base, handle) = serve(|_| {
            (
                200,
                vec![("Content-Length".into(), "100".into())],
                vec![b"partial".to_vec()],
                true,
            )
        });
        let dest = temp_destination("abort");
        let error = download_to(&client, &base, &dest)
            .await
            .expect_err("abort fails");
        assert!(
            error.contains("failed while reading")
                || error.contains("cannot resolve/connect")
                || error.contains("failed to download"),
            "got: {error}"
        );
        assert!(!dest.exists(), "partial file must be removed");
        handle.abort();
    }

    #[tokio::test]
    async fn redirect_chain_completes_and_loop_is_bounded() {
        // Final target.
        let (final_base, final_handle) = serve(|_| {
            (
                200,
                vec![("Content-Length".into(), "11".into())],
                vec![b"redirect-ok".to_vec()],
                false,
            )
        });
        let final_url = final_base.clone();
        // One-hop redirect.
        let (redir_base, redir_handle) = serve(move |_| {
            (
                302,
                vec![("Location".into(), final_url.clone())],
                vec![],
                false,
            )
        });
        let client = test_client();
        let dest = temp_destination("redir-ok");
        let status = download_to(&client, &redir_base, &dest)
            .await
            .expect("redirect follows");
        assert_eq!(status, DownloadStatus::Success);
        assert_eq!(std::fs::read(&dest).unwrap(), b"redirect-ok");
        let _ = std::fs::remove_file(&dest);
        redir_handle.abort();
        final_handle.abort();

        // Self-loop is bounded by MAX_REDIRECTS.
        let (loop_base, loop_handle) = serve(|req| {
            let host = req
                .lines()
                .find_map(|line| {
                    let lower = line.to_ascii_lowercase();
                    lower
                        .strip_prefix("host:")
                        .map(|rest| rest.trim().to_owned())
                })
                .unwrap_or_else(|| "127.0.0.1".to_owned());
            (
                302,
                vec![("Location".into(), format!("http://{host}/"))],
                vec![],
                false,
            )
        });
        let dest = temp_destination("redir-loop");
        let error = download_to(&client, &loop_base, &dest)
            .await
            .expect_err("loop bounded");
        assert!(
            error.contains("redirect") || error.contains("too many"),
            "got: {error}"
        );
        assert!(!dest.exists());
        loop_handle.abort();
    }

    #[tokio::test]
    async fn streaming_writes_chunks_in_order_and_surfaces_write_failures() {
        let chunks: Vec<Vec<u8>> = (0..8).map(|i| vec![i; 8192]).collect();
        let expected: Vec<u8> = chunks.concat();
        let expected_len = expected.len();
        let (base, handle) = serve(move |_| {
            (
                200,
                vec![("Content-Length".into(), expected_len.to_string())],
                chunks.clone(),
                false,
            )
        });
        let client = test_client();
        let dest = temp_destination("multi-chunk");
        let status = download_to(&client, &base, &dest)
            .await
            .expect("multi-chunk");
        assert_eq!(status, DownloadStatus::Success);
        assert_eq!(std::fs::read(&dest).unwrap(), {
            let mut v = Vec::new();
            for i in 0..8_u8 {
                v.extend(std::iter::repeat_n(i, 8192));
            }
            v
        });
        let _ = std::fs::remove_file(&dest);
        handle.abort();

        // Destination write failure surfaces without panic.
        let (base, handle) = serve(|_| {
            (
                200,
                vec![("Content-Length".into(), "2".into())],
                vec![b"ok".to_vec()],
                false,
            )
        });
        let dir = temp_destination("a-directory");
        std::fs::create_dir_all(&dir).unwrap();
        let error = download_to(&client, &base, &dir)
            .await
            .expect_err("write fails");
        assert!(
            error.contains("failed while writing staged release asset"),
            "got: {error}"
        );
        // A directory destination must remain a directory, not a consumable file.
        assert!(dir.is_dir());
        let _ = std::fs::remove_dir_all(&dir);
        handle.abort();
    }

    #[tokio::test]
    async fn small_text_respects_configured_body_bound() {
        let big = vec![b'x'; METADATA_MAX_BYTES + 1024];
        let len = big.len();
        let (base, handle) = serve(move |_| {
            (
                200,
                vec![("Content-Length".into(), len.to_string())],
                vec![big.clone()],
                false,
            )
        });
        let client = test_client();
        let error = get_small_text(&client, &base, METADATA_MAX_BYTES, "crates.io metadata")
            .await
            .expect_err("bound enforced");
        assert!(error.contains("exceeds"), "got: {error}");
        handle.abort();

        let (base, handle) = serve(|_| {
            (
                200,
                vec![("Content-Length".into(), "11".into())],
                vec![b"hello world".to_vec()],
                false,
            )
        });
        let text = get_small_text(&client, &base, METADATA_MAX_BYTES, "crates.io metadata")
            .await
            .expect("small body passes");
        assert_eq!(text, "hello world");
        handle.abort();
    }

    #[tokio::test]
    async fn short_injected_timeouts_still_map_to_timeout_errors() {
        use std::time::Duration;
        let (base, handle) = serve(|_| {
            // Deliberately slow headers: the short total deadline fires first.
            std::thread::sleep(Duration::from_millis(300));
            (
                200,
                vec![("Content-Length".into(), "2".into())],
                vec![b"ok".to_vec()],
                false,
            )
        });
        let timeout = Timeout::builder()
            .connect(Duration::from_secs(5))
            .total(Duration::from_millis(50))
            .build();
        let client = build_update_client_with_timeout_and_env(
            timeout,
            &eggfetch_core::ProxyEnvironment::new(),
        )
        .expect("short-timeout client builds");
        let dest = temp_destination("timeout");
        let error = download_to(&client, &base, &dest)
            .await
            .expect_err("times out");
        assert!(error.contains("update request timed out"), "got: {error}");
        assert!(!dest.exists());
        handle.abort();
    }

    #[test]
    fn release_binary_path_streams_without_whole_body_buffer() {
        // Structural guard: the binary path must use incremental
        // `bytes_stream`, while only the small metadata/checksum path may use
        // buffered `bytes()`. This keeps multi-megabyte executables off the heap.
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/update.rs"));
        let download_section = source
            .split("async fn download_to")
            .nth(1)
            .expect("download_to exists")
            .split("async fn ")
            .next()
            .unwrap_or_default();
        assert!(
            download_section.contains("bytes_stream"),
            "binary must stream"
        );
        assert!(
            !download_section.contains(".bytes().await"),
            "binary must not buffer via bytes()"
        );
    }
}
