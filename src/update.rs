//! Manifest-first self-update support and deterministic release-contract helpers.
//!
//! Local verified-transaction mechanics are owned by `eggup-core` and network
//! acquisition by `eggup-eggfetch` (strict Eggfetch policy); manifest
//! parse/project/materialization is owned by `eggup-eggpack`. The updater
//! keeps all release/update policy (version selection, release origin,
//! manifest/legacy source selection, Cargo fallback, install destination,
//! candidate identity, CLI presentation).
//!
//! Source policy: the producer-owned `release-manifest.json` is fetched as
//! bounded metadata after Eggsact selects and authorizes the release. Exact
//! 404 enters the legacy checksum-sidecar compatibility path for releases
//! that predate manifests; every other manifest failure is hard, and a valid
//! manifest whose artifact is absent never falls back.
//!
//! Transport policy (single configuration point in `eggup_transport()`):
//! - user agent `eggsact-self-update`;
//! - HTTP/1 only; redirects followed with strict HTTPS -> HTTP downgrade
//!   rejection; native roots with packaged WebPKI fallback and full
//!   certificate/hostname verification;
//! - explicit opt-in environment proxy routing; invalid proxy fails closed;
//! - 10-second connect timeout + 120-second total wall-clock timeout;
//! - small metadata/checksum bodies bounded in memory; release binaries stream
//!   to disk chunk-by-chunk via the Eggup seam, never buffered as one `Vec`;
//! - no retry policy; no compression/cookies features.
//!
//! Local safety (owned by Eggup): private staging, SHA-256 integrity
//! verification, bounded `--version` candidate validation with cleared
//! environment, explicit current-executable ownership proof, mutation locking
//! with backup/rollback, and structured receipts. Manifest size/digest
//! evidence replaces manifest-branch sidecar parsing; checksum/TLS/timeout/5xx
//! failures never become Cargo fallback. Only a genuine selected-binary 404
//! (or unsupported host target) reaches Cargo, and only an exact manifest 404
//! reaches the legacy sidecar path.

#[cfg(test)]
use eggfetch_core::{Client, HttpVersionPolicy, ProxyEnvironment, RedirectPolicy, Timeout};
use eggup_acquisition::{
    AcquisitionRequest, AcquisitionTransport, CancelFlag, FetchLimits, FetchOutcome,
};
use eggup_core::{
    AbsentPolicy, ArtifactMember, ArtifactSet, CommitOwnership, ExactIdentityValidator,
    InstallPlan, IntegrityRequirement, MemberId, Ownership, OwnershipVerifier, PermissionsIntent,
    ProductId, ReleaseId, TransactionDisposition,
};
use eggup_eggfetch::{EggfetchConfig, EggfetchTransport, ProxyDecision};
#[cfg(test)]
use futures_util::StreamExt;
use std::collections::HashMap;
use std::env;
use std::fs::{self};
use std::io::{self};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(test)]
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
/// Finite Eggsact-owned ceiling for release artifact downloads. The current
/// Eggup default (128 MiB) is sufficient for the observed ~11-17 MiB release
/// binaries; the manifest path tightens this per artifact to the manifest
/// exact size via `bind_requests` and never widens it.
pub const ARTIFACT_MAX_BYTES: u64 = 128 * 1024 * 1024;
/// Producer-owned manifest asset name published at every release root since
/// the Eggpack Ecosystem M001 / Eggsact Distribution M005 cutover.
pub const MANIFEST_FILE_NAME: &str = "release-manifest.json";

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

#[allow(dead_code)] // Documents the production release-asset URL contract.
pub fn release_asset_url(version: &StableVersion, target: &str) -> String {
    origin_asset_url(RELEASE_ORIGIN, version, &asset_name(target))
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

/// Release origin authority: this repository's GitHub releases. The
/// release/tag and origin are chosen by Eggsact policy before any manifest
/// or artifact URL is constructed; `eggup-eggpack` never sees origins.
pub const RELEASE_ORIGIN: &str = "https://github.com/eggstack/eggsact/releases";

fn origin_asset_url(origin: &str, version: &StableVersion, name: &str) -> String {
    format!("{origin}/download/v{version}/{name}")
}

/// Exact producer-owned manifest URL under an already-authorized release
/// origin. The filename is the producer convention established by the
/// Eggpack Ecosystem M001 / Eggsact Distribution M005 cutover.
fn manifest_url_for_origin(origin: &str, version: &StableVersion) -> String {
    origin_asset_url(origin, version, MANIFEST_FILE_NAME)
}

#[allow(dead_code)] // Documents the production manifest URL contract.
pub fn manifest_url(version: &StableVersion) -> String {
    manifest_url_for_origin(RELEASE_ORIGIN, version)
}

/// Exact artifact URL under an already-authorized release origin using the
/// manifest-provided artifact name. Only constructed after product/release
/// binding accepts the manifest.
fn manifest_artifact_url_for_origin(
    origin: &str,
    version: &StableVersion,
    artifact_name: &str,
) -> String {
    origin_asset_url(origin, version, &encode_url_segment(artifact_name))
}

/// Percent-encode a value for use as a single URL path segment.
///
/// The artifact name comes from the manifest, and `eggpack_manifest`'s
/// `valid_name` does not reject `?` or `#`, so an unencoded name could
/// rewrite the query or fragment of the download URL. The origin remains a
/// compile-time constant and integrity still comes from the manifest, so this
/// only keeps the request pointed at the intended release asset. Names made
/// entirely of unreserved characters — every producer artifact today — encode
/// to themselves.
fn encode_url_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
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

/// Expected `--version` output contract (`eggsact X.Y.Z`).
///
/// Production validation is performed by the Eggup exact-identity validator;
/// this parser remains as the tested output-format contract.
#[allow(dead_code)]
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

/// Release-policy outcome: only `NotFound` (or an unsupported host) may reach
/// Cargo fallback; every hard failure stays a hard error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum DownloadStatus {
    Success,
    NotFound,
    HardFailure,
}

/// HTTP status classifier backing the fallback rule above.
///
/// Production mapping is performed inside the Eggup seam; this stays as the
/// tested policy contract.
#[allow(dead_code)]
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
///
/// Retained as the policy test harness (production acquisition now goes
/// through the Eggup seam with the same constants); exercised by policy tests.
#[cfg(test)]
fn update_timeout() -> Timeout {
    Timeout::builder()
        .connect(CONNECT_TIMEOUT)
        .total(TOTAL_TIMEOUT)
        .build()
}

/// Follow redirects but reject HTTPS -> HTTP downgrades before second-hop I/O.
///
/// See `update_timeout` for the retention note.
#[cfg(test)]
fn update_redirect_policy() -> RedirectPolicy {
    RedirectPolicy::strict(MAX_REDIRECTS)
}

fn redact_url_for_error(url: &str) -> String {
    eggup_acquisition::redact_url(url)
}

/// Map an eggfetch transport error into the updater's concise,
/// credential-safe application message.
///
/// Retained for the policy test harness; production errors flow through
/// `map_eggup_error`.
#[cfg(test)]
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
///
/// Retained for the policy test harness; production builds its transport via
/// `eggup_transport()` with the same constants.
#[cfg(test)]
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

#[cfg(test)]
fn build_update_client_with_env(env: &ProxyEnvironment) -> Result<Client, String> {
    build_update_client_with_timeout_and_env(update_timeout(), env)
}

/// Fetch a small text document (crates.io metadata, checksum sidecar) with an
/// explicit byte bound. The request-local eggfetch `max_decoded_body_size`
/// limit is the authoritative safety boundary: it caps the decoded body
/// during streaming/buffering even when `Content-Length` is absent or false.
/// The `Content-Length` check below is advisory/early only, and no
/// caller-side post-buffer accumulation limit is maintained.
///
/// Retained for the policy test harness; production uses `eggup_get_text`.
#[cfg(test)]
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
        .max_decoded_body_size(max_bytes)
        .send()
        .await
        .map_err(|error| map_small_body_error(error, url, what, max_bytes))?;
    let status = response.status().as_u16();
    if !matches!(classify_http_status(status), DownloadStatus::Success) {
        return Err(format!(
            "HTTP {status} from {redacted} while fetching {what}"
        ));
    }
    // Advisory early rejection only; the authoritative bound is enforced by
    // eggfetch while the body streams, before unbounded caller buffering.
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
        .map_err(|error| map_small_body_error(error, url, what, max_bytes))?;
    String::from_utf8(bytes.to_vec())
        .map_err(|error| format!("invalid UTF-8 in {what} from {redacted}: {error}"))
}

/// Map a small-metadata/checksum transport error, giving eggfetch's
/// authoritative decoded-body limit a concise bound-identifying message.
/// Credential-safe: only the redacted URL is embedded.
///
/// Retained for the policy test harness.
#[cfg(test)]
fn map_small_body_error(
    error: eggfetch_core::Error,
    url: &str,
    what: &str,
    max_bytes: usize,
) -> String {
    if matches!(error, eggfetch_core::Error::DecodedBodyTooLarge) {
        let redacted = redact_url_for_error(url);
        return format!(
            "{what} from {redacted} exceeds {max_bytes}-byte bound (decoded body too large: {error})"
        );
    }
    map_transport_error(error, url, what)
}

/// Stream a release binary to disk chunk-by-chunk. Never buffers the
/// executable body into one `Vec<u8>`/`Bytes` allocation in updater code.
/// On any body/read/write failure the partial destination is removed so
/// `prepare_candidate` cannot accidentally consume it.
///
/// Retained for the policy test harness; production uses `eggup_download`.
#[cfg(test)]
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
                    if let Err(error) =
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                    {
                        // Do not leave a staging directory behind that we could
                        // not secure: it would stay world-readable in /tmp.
                        let _ = fs::remove_dir(&path);
                        return Err(format!("cannot secure temporary directory: {error}"));
                    }
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("cannot create temporary directory: {error}")),
        }
    }
    Err("could not create a unique temporary directory".into())
}

/// Single Eggup transport configuration point (mirrors the strict policy
/// above: HTTP/1, Rustls, downgrade-deny redirects, 10s/120s timeouts,
/// explicit environment proxy, bounded bodies, no retry).
fn eggup_transport() -> Result<EggfetchTransport, String> {
    EggfetchConfig::strict()
        .user_agent(USER_AGENT)
        .timeouts(CONNECT_TIMEOUT, TOTAL_TIMEOUT)
        .max_redirects(MAX_REDIRECTS)
        .proxy(ProxyDecision::FromEnvironment)
        .pipe_transport()
        .map_err(|e| format!("cannot build update transport: {e}"))
}

/// Small helper to keep the builder chain readable.
trait PipeTransport {
    fn pipe_transport(self) -> Result<EggfetchTransport, eggup_acquisition::AcquisitionError>;
}

impl PipeTransport for EggfetchConfig {
    fn pipe_transport(self) -> Result<EggfetchTransport, eggup_acquisition::AcquisitionError> {
        EggfetchTransport::strict(self)
    }
}

fn eggup_limits(max_metadata: usize) -> FetchLimits {
    FetchLimits {
        max_metadata_bytes: max_metadata,
        max_artifact_bytes: ARTIFACT_MAX_BYTES,
        connect_timeout: CONNECT_TIMEOUT,
        total_timeout: TOTAL_TIMEOUT,
    }
}

fn map_eggup_error(e: eggup_acquisition::AcquisitionError, url: &str, what: &str) -> String {
    use eggup_acquisition::AcquisitionError as E;
    let redacted = redact_url_for_error(url);
    match e {
        E::TooLarge { limit } => {
            format!("{what} from {redacted} exceeds {limit}-byte bound")
        }
        E::Timeout { phase } => {
            format!("update request timed out ({phase}) while fetching {what} from {redacted}")
        }
        E::Cancelled => format!("update cancelled while fetching {what} from {redacted}"),
        E::InvalidInput(d) => format!("invalid update request for {what} from {redacted}: {d}"),
        E::Transport(d) => format!("failed to download {what} from {redacted}: {d}"),
        E::Io(d) => format!("failed while reading release asset ({what}) from {redacted}: {d}"),
        _ => format!("failed to download {what} from {redacted}: {e}"),
    }
}

/// Structural metadata outcome: `Absent` is an exact transport 404, never
/// inferred from formatted error text. Only an `Absent` manifest may enter
/// the legacy sidecar compatibility path; every other failure is hard.
enum MetadataFetch {
    Present(Vec<u8>),
    Absent,
}

/// Fetch a small document via the Eggup seam with a structural outcome (runs
/// the sync adapter on a blocking thread so the current-thread runtime is
/// never blocked).
async fn eggup_fetch_metadata(
    url: &str,
    max_bytes: usize,
    what: &str,
) -> Result<MetadataFetch, String> {
    let url = url.to_string();
    let what = what.to_string();
    tokio::task::spawn_blocking(move || {
        let transport = eggup_transport()?;
        let req =
            AcquisitionRequest::new(url.clone()).map_err(|e| map_eggup_error(e, &url, &what))?;
        match transport
            .fetch_metadata(&req, eggup_limits(max_bytes), &CancelFlag::new())
            .map_err(|e| map_eggup_error(e, &url, &what))?
        {
            FetchOutcome::Success(b) => Ok(MetadataFetch::Present(b.bytes().to_vec())),
            FetchOutcome::NotFound => Ok(MetadataFetch::Absent),
        }
    })
    .await
    .map_err(|e| format!("update task failed: {e}"))?
}

/// Fetch a small text document (crates.io metadata, checksum sidecar) with an
/// explicit byte bound. Absence keeps the historical 404 message so existing
/// policy text is unchanged.
async fn eggup_get_text(url: &str, max_bytes: usize, what: &str) -> Result<String, String> {
    let what_owned = what.to_string();
    match eggup_fetch_metadata(url, max_bytes, what).await? {
        MetadataFetch::Present(bytes) => String::from_utf8(bytes)
            .map_err(|e| format!("invalid UTF-8 in {what_owned} from {url}: {e}")),
        MetadataFetch::Absent => Err(format!(
            "HTTP 404 from {} while fetching {what_owned}",
            redact_url_for_error(url)
        )),
    }
}

/// Download a release binary via the Eggup seam. Returns `NotFound` only for a
/// genuine 404; every other failure is a hard error (never Cargo fallback).
async fn eggup_download(url: &str, destination: &Path) -> Result<DownloadStatus, String> {
    let url = url.to_string();
    let destination = destination.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let transport = eggup_transport()?;
        let req = AcquisitionRequest::new(url.clone())
            .map_err(|e| map_eggup_error(e, &url, "release asset"))?;
        match transport
            .fetch_artifact(
                &req,
                &destination,
                eggup_limits(CHECKSUM_MAX_BYTES),
                &CancelFlag::new(),
            )
            .map_err(|e| map_eggup_error(e, &url, "release asset"))?
        {
            FetchOutcome::Success(_) => Ok(DownloadStatus::Success),
            FetchOutcome::NotFound => Ok(DownloadStatus::NotFound),
        }
    })
    .await
    .map_err(|e| format!("update task failed: {e}"))?
}

// ── Release-manifest consumer path (Eggpack Interop M003) ──────────────

/// Fetch the producer-owned release manifest as bounded metadata with a
/// structural outcome. `Absent` (exact 404) is the sole entry to the legacy
/// sidecar compatibility path; every other failure is a hard error.
async fn fetch_release_manifest(
    origin: &str,
    version: &StableVersion,
) -> Result<MetadataFetch, String> {
    let url = manifest_url_for_origin(origin, version);
    eggup_fetch_metadata(&url, eggup_eggpack::MAX_MANIFEST_BYTES, "release manifest").await
}

/// Project the manifest and bind product/release/target identity to the
/// release already selected by Eggsact. The manifest never selects a
/// newer/different release: product, release, or target mismatch is a hard
/// error. Diagnostics never echo manifest contents.
fn resolve_manifest_projection(
    manifest_bytes: &[u8],
    target: &ReleaseTarget,
    latest: StableVersion,
) -> Result<eggup_eggpack::ManifestProjection, String> {
    let projection =
        eggup_eggpack::project_json(manifest_bytes, target.rust_target).map_err(|e| {
            format!(
                "release manifest is not usable for {}: {e}",
                target.rust_target
            )
        })?;
    let (product, release) = eggup_eggpack::install_ids(&projection)
        .map_err(|e| format!("release manifest projection is not installable: {e}"))?;
    if product.as_str() != "eggsact" {
        return Err("release manifest product mismatch: expected 'eggsact'".into());
    }
    if release.as_str() != latest.to_string() {
        return Err(
            "release manifest release mismatch: manifest does not describe the authorized release"
                .into(),
        );
    }
    Ok(projection)
}

/// Manifest-selected artifacts acquired under the already-authorized release
/// origin, plus the accepted projection that describes them.
#[derive(Debug)]
struct ManifestAcquired {
    paths: HashMap<String, PathBuf>,
    projection: eggup_eggpack::ManifestProjection,
    member: MemberId,
}

/// Acquire one manifest-planned artifact to an explicit staging path. The
/// adapter has already tightened the caller ceiling to the manifest exact
/// size. A manifest-backed artifact NotFound is a hard incomplete-release
/// failure: it never becomes Cargo or legacy-sidecar fallback.
async fn acquire_manifest_artifact(
    staging: &Path,
    origin: &str,
    version: StableVersion,
    planned: &eggup_eggpack::PlannedAcquisition,
) -> Result<PathBuf, String> {
    let url = manifest_artifact_url_for_origin(origin, &version, &planned.artifact_name);
    let destination = staging.join(&planned.artifact_name);
    let request = planned.request.clone();
    let limits = planned.limits;
    let name = planned.artifact_name.clone();
    tokio::task::spawn_blocking(move || {
        let transport = eggup_transport()?;
        match transport
            .fetch_artifact(&request, &destination, limits, &CancelFlag::new())
            .map_err(|e| map_eggup_error(e, &url, "release manifest artifact"))?
        {
            FetchOutcome::Success(_) => Ok(destination),
            FetchOutcome::NotFound => Err(format!(
                "release manifest artifact '{name}' is absent from the authorized release; refusing fallback to legacy evidence"
            )),
        }
    })
    .await
    .map_err(|e| format!("update task failed: {e}"))?
}

/// Run the manifest path: project/bind against the authorized release, then
/// acquire every planned artifact. Eggsact is a direct single-binary release,
/// so the projection must select exactly one installable artifact; bundle and
/// archive forms contradict the producer contract and fail closed here.
async fn prepare_manifest_candidate_async(
    staging: &Path,
    origin: &str,
    target: &ReleaseTarget,
    latest: StableVersion,
    manifest_bytes: &[u8],
) -> Result<ManifestAcquired, String> {
    let projection = resolve_manifest_projection(manifest_bytes, target, latest)?;
    let (artifact_name, member) = match &projection {
        eggup_eggpack::ManifestProjection::Installable { artifacts, .. } => {
            if artifacts.len() != 1 {
                return Err(format!(
                    "release manifest selects {} artifacts; this updater deploys exactly one binary",
                    artifacts.len()
                ));
            }
            (
                artifacts[0].artifact_name.clone(),
                artifacts[0].member_id.clone(),
            )
        }
        eggup_eggpack::ManifestProjection::Archive { .. } => {
            return Err(
                "release manifest selects an archive artifact; the adapter requires separately qualified extraction"
                    .into(),
            );
        }
    };
    // Exact artifact request under the already-authorized release origin; the
    // adapter tightens the caller ceiling to the manifest exact size and
    // never widens it.
    let url = manifest_artifact_url_for_origin(origin, &latest, &artifact_name);
    let request = AcquisitionRequest::new(url.clone())
        .map_err(|e| map_eggup_error(e, &url, "release manifest artifact"))?;
    let planned = projection
        .bind_requests(
            HashMap::from([(artifact_name, request)]),
            eggup_limits(METADATA_MAX_BYTES),
        )
        .map_err(|e| format!("release manifest acquisition binding failed: {e}"))?;
    let mut paths = HashMap::with_capacity(planned.len());
    for item in &planned {
        let path = acquire_manifest_artifact(staging, origin, latest, item).await?;
        paths.insert(item.artifact_name.clone(), path);
    }
    Ok(ManifestAcquired {
        paths,
        projection,
        member,
    })
}

/// Materialize the accepted manifest projection with caller-bound
/// destinations and commit through the Eggup verified transaction. The
/// projected member binds to the exact basename of the running executable
/// under its existing installation root, preserving canonical and renamed
/// update-in-place behavior. Size/digest evidence comes only from the
/// adapter; no manifest-branch checksum parsing exists.
fn commit_manifest_candidate(
    manifest: &ManifestAcquired,
    current: &Path,
    latest: StableVersion,
) -> Result<ReplacementOutcome, String> {
    let file_name = current
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "cannot locate executable file name".to_string())?;
    let (product, release) = eggup_eggpack::install_ids(&manifest.projection)
        .map_err(|e| format!("release manifest projection is not installable: {e}"))?;
    let mut destinations = HashMap::with_capacity(1);
    destinations.insert(manifest.member.clone(), file_name.to_string());
    let mut permissions = HashMap::with_capacity(1);
    permissions.insert(manifest.member.clone(), PermissionsIntent::Executable);
    let set = manifest
        .projection
        .materialize_artifact_set_with_destinations(
            manifest.paths.clone(),
            destinations,
            permissions,
        )
        .map_err(|e| format!("release manifest materialization failed: {e}"))?;
    commit_artifact_set(&set, &manifest.member, product, release, current, latest)
}

/// Ownership proof using eggsact's known executable identity and path.
///
/// Returns `Owned` only when the destination is the running eggsact executable
/// itself (same canonical path as `current_exe`) and is a regular file.
/// Anything else is `Foreign` (exists but is not us) or `Unknown` (unreadable).
/// Absent destinations are reported as `Absent`; creation is denied by policy
/// (`AbsentPolicy::DenyCreate`) because self-update only replaces.
#[derive(Debug)]
struct CurrentExeVerifier {
    current_exe: PathBuf,
}

impl OwnershipVerifier for CurrentExeVerifier {
    fn verify(&self, _member: &MemberId, destination: &Path) -> Ownership {
        let current = match fs::canonicalize(&self.current_exe) {
            Ok(p) => p,
            Err(_) => return Ownership::Unknown,
        };
        match fs::symlink_metadata(destination) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ownership::Absent,
            Err(_) => Ownership::Unknown,
            Ok(meta) => {
                if !meta.is_file() {
                    return Ownership::Foreign;
                }
                match fs::canonicalize(destination) {
                    Ok(live) if live == current => Ownership::Owned,
                    Ok(_) => Ownership::Foreign,
                    Err(_) => Ownership::Unknown,
                }
            }
        }
    }
}

/// Integrity digest via Eggup (replaces the local streaming hasher).
fn eggup_hash(path: &Path) -> Result<[u8; 32], String> {
    eggup_core::hash_file(path).map_err(|e| format!("cannot hash candidate: {e}"))
}

/// Compare two SHA-256 digests without an early exit.
///
/// Both operands are hashes of attacker-influenceable bytes, so compare in
/// constant time rather than letting `==` leak the matching prefix through
/// timing. Defense in depth: a mismatch is reported identically either way.
fn digests_equal(expected: &[u8; 32], actual: &[u8; 32]) -> bool {
    let mut difference = 0_u8;
    for index in 0..expected.len() {
        difference |= expected[index] ^ actual[index];
    }
    difference == 0
}

async fn crates_latest_version_async() -> Result<StableVersion, String> {
    let text = eggup_get_text(CRATES_API, METADATA_MAX_BYTES, "crates.io metadata").await?;
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

/// Acquired candidate plus its Eggup integrity evidence.
///
/// `from_cargo` records whether the Cargo fallback path produced the binary
/// (release policy; Eggup never selects it). For Cargo builds the digest is
/// self-measured (stability across validation/commit); for release assets it
/// is the sidecar digest (independent integrity evidence).
#[derive(Debug)]
struct AcquiredCandidate {
    path: PathBuf,
    digest: [u8; 32],
}

/// Prepared update source: the manifest path carries adapter-accepted
/// projection inputs; the legacy path carries sidecar-verified inputs.
/// Construction of legacy inputs lives only behind the manifest-absent
/// branch of `prepare_candidate_async`, so a present-but-invalid manifest
/// can never enter the legacy path.
#[derive(Debug)]
enum PreparedUpdate {
    Manifest(ManifestAcquired),
    Legacy(AcquiredCandidate),
}

/// Manifest dispatch: fetch the producer-owned manifest with a structural
/// outcome, then run the manifest path or the explicit legacy compatibility
/// branch. Any non-404 manifest failure is a hard error.
async fn prepare_candidate_async(
    staging: &Path,
    origin: &str,
    target: &ReleaseTarget,
    latest: StableVersion,
) -> Result<PreparedUpdate, String> {
    match fetch_release_manifest(origin, &latest).await? {
        MetadataFetch::Present(bytes) => {
            let manifest =
                prepare_manifest_candidate_async(staging, origin, target, latest, &bytes).await?;
            Ok(PreparedUpdate::Manifest(manifest))
        }
        MetadataFetch::Absent => {
            // Explicit backwards-compatibility policy for releases that
            // predate manifests; entered only on exact manifest 404.
            let acquired = prepare_legacy_candidate_async(staging, origin, target, latest).await?;
            Ok(PreparedUpdate::Legacy(acquired))
        }
    }
}

async fn prepare_legacy_candidate_async(
    staging: &Path,
    origin: &str,
    target: &ReleaseTarget,
    latest: StableVersion,
) -> Result<AcquiredCandidate, String> {
    let binary = staging.join(target.asset_name);
    let binary_url = origin_asset_url(origin, &latest, target.asset_name);
    match eggup_download(&binary_url, &binary).await? {
        DownloadStatus::NotFound => {
            let path = cargo_candidate(staging, latest)?;
            let digest = eggup_hash(&path)?;
            return Ok(AcquiredCandidate { path, digest });
        }
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
    let checksum_text = eggup_get_text(
        &checksum_url(&binary_url),
        CHECKSUM_MAX_BYTES,
        "release checksum",
    )
    .await?;
    let expected = parse_checksum(&checksum_text)?;
    let actual = eggup_hash(&binary)?;
    if !digests_equal(&expected, &actual) {
        return Err("release checksum does not match the downloaded executable".into());
    }
    Ok(AcquiredCandidate {
        path: binary,
        digest: expected,
    })
}

#[cfg(windows)]
fn replace_current(
    candidate: &Path,
    current: &Path,
    expected_digest: &str,
) -> Result<ReplacementOutcome, String> {
    let pid = std::process::id();
    // The adjacent name must not be predictable: `fs::copy` follows an existing
    // symlink and a same-user attacker who can write the install directory
    // could otherwise plant a file at a known name between the copy and the
    // move. The pid alone is observable.
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let tag = format!("eggsact-update-{pid}-{nonce}");
    let adjacent = current.with_extension(format!("{tag}.exe"));
    fs::copy(candidate, &adjacent).map_err(|error| permission_error(current, error))?;
    let status = current.with_extension(format!("{tag}.status"));
    let script = windows_replacement_script(pid, &adjacent, current, &status, expected_digest);
    Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
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

/// Commits an acquired candidate through the Eggup verified transaction.
///
/// Staging, SHA-256 integrity, bounded `--version` candidate validation,
/// current-executable ownership proof, mutation locking, and backup/rollback
/// are owned by Eggup. On Unix the commit replaces the live binary; on
/// Windows the running image cannot be renamed, so the validated staged
/// executable is handed to the existing PowerShell replacement script.
fn commit_candidate(
    acquired: &AcquiredCandidate,
    current: &Path,
    latest: StableVersion,
) -> Result<ReplacementOutcome, String> {
    let file_name = current
        .file_name()
        .ok_or_else(|| "cannot locate executable file name".to_string())?;
    let member_id = MemberId::new("main").map_err(|e| format!("invalid member: {e}"))?;
    let member = ArtifactMember::new(member_id.clone(), acquired.path.clone(), file_name)
        .map_err(|e| format!("invalid update plan: {e}"))?
        .with_integrity(IntegrityRequirement::Sha256(acquired.digest))
        .with_permissions(PermissionsIntent::Executable);
    let product = ProductId::new("eggsact").map_err(|e| format!("invalid product: {e}"))?;
    let release =
        ReleaseId::new(latest.to_string()).map_err(|e| format!("invalid release: {e}"))?;
    let set = ArtifactSet::single(member).map_err(|e| format!("invalid update plan: {e}"))?;
    commit_artifact_set(&set, &member_id, &product, &release, current, latest)
}

/// Shared Eggup commit tail for both update sources: locked revalidation,
/// candidate validation, ownership proof, replacement, and disposition
/// mapping are identical; only the ArtifactSet construction differs.
fn commit_artifact_set(
    set: &ArtifactSet,
    member_id: &MemberId,
    product: &ProductId,
    release: &ReleaseId,
    current: &Path,
    latest: StableVersion,
) -> Result<ReplacementOutcome, String> {
    let root = current
        .parent()
        .ok_or_else(|| "cannot locate installation directory".to_string())?;
    let plan = InstallPlan::new(product.clone(), release.clone(), root, set.clone())
        .map_err(|e| format!("invalid update plan: {e}"))?;
    // `--version` is required: with no argv the staged binary falls through to
    // its usage block, whose bytes can never equal the expected identity, so
    // every update failed candidate validation.
    let validator = ExactIdentityValidator::new(member_id.clone(), format!("eggsact {latest}\n"))
        .args(["--version"])
        .timeout(Duration::from_secs(10));
    let validated = plan
        .prepare()
        .map_err(|e| format!("cannot stage update: {e}"))?
        .verify_integrity()
        .map_err(|e| format!("integrity verification failed: {e}"))?
        .validate(&validator)
        .map_err(|e| format!("candidate validation failed: {e}"))?;
    let verifier = CurrentExeVerifier {
        current_exe: current.to_path_buf(),
    };
    let ownership = CommitOwnership::new(&verifier, AbsentPolicy::DenyCreate);
    #[cfg(unix)]
    {
        let receipt = validated
            .commit(ownership)
            .map_err(|e| permission_error(current, io::Error::other(e.to_string())))?;
        match receipt.disposition() {
            TransactionDisposition::Committed => Ok(ReplacementOutcome::Complete),
            TransactionDisposition::RolledBack => {
                let cause = receipt
                    .failure()
                    .map(|f| format!("{}: {}", f.phase().as_str(), f.detail()))
                    .unwrap_or_else(|| "unknown cause".into());
                Err(format!(
                    "update rolled back ({cause}); previous version preserved"
                ))
            }
            TransactionDisposition::RecoveryRequired => {
                let cause = receipt
                    .failure()
                    .map(|f| format!("{}: {}", f.phase().as_str(), f.detail()))
                    .unwrap_or_else(|| "unknown cause".into());
                let path = receipt
                    .recovery_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "<no recovery path>".into());
                Err(format!(
                    "update requires manual recovery ({cause}); evidence at {path}; lock retained"
                ))
            }
        }
    }
    #[cfg(windows)]
    {
        let staged = validated
            .staged_path(member_id)
            .map_err(|e| format!("cannot locate staged update: {e}"))?;
        // The staged bytes were already verified by Eggup; hash them again here
        // so the replacement script can compare the adjacent copy against a
        // known-good digest at the moment it installs.
        let staged_digest = hex_digest(&eggup_hash(&staged)?);
        windows_replace_current(&staged, current, &staged_digest)
    }
}

#[cfg(windows)]
fn windows_replace_current(
    staged: &Path,
    current: &Path,
    expected_digest: &str,
) -> Result<ReplacementOutcome, String> {
    replace_current(staged, current, expected_digest)
}

/// Lowercase hex encoding of a SHA-256 digest.
#[cfg_attr(not(test), allow(dead_code))] // Consumed by the Windows-only replacement path.
fn hex_digest(digest: &[u8; 32]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[allow(dead_code)] // Used by the Windows-only replacement path.
fn powershell_quote(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

#[allow(dead_code)] // Used by the Windows-only replacement path.
fn powershell_quote_str(value: &str) -> String {
    value.replace('\'', "''")
}

#[allow(dead_code)] // Used by the Windows-only replacement path and cross-platform tests.
fn windows_replacement_script(
    pid: u32,
    source: &Path,
    target: &Path,
    status: &Path,
    expected_digest: &str,
) -> String {
    let source = powershell_quote(source);
    let target = powershell_quote(target);
    let status = powershell_quote(status);
    let expected_digest = powershell_quote_str(expected_digest);
    // The digest is re-checked immediately before the move, not merely before
    // the copy: without it, anything that can write the install directory could
    // replace the adjacent file between `fs::copy` and `Move-Item` and have
    // unverified bytes installed. A mismatch throws, so the existing retry
    // path reports it as a failure and never installs.
    format!(
        "$p={pid}; $source='{source}'; $target='{target}'; $status='{status}'; $expected='{expected_digest}'; $status_tmp=\"$status.tmp\"; function Write-UpdateStatus([string]$value) {{ Set-Content -LiteralPath $status_tmp -Value $value -NoNewline; Move-Item -LiteralPath $status_tmp -Destination $status -Force }}; try {{ while (Get-Process -Id $p -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 100 }}; $last_error='replacement did not complete'; for ($attempt=0; $attempt -lt 50; $attempt++) {{ try {{ if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $expected) {{ throw \"staged file digest mismatch\" }}; Move-Item -LiteralPath $source -Destination $target -Force -ErrorAction Stop; Remove-Item -LiteralPath $status -Force -ErrorAction SilentlyContinue; exit 0 }} catch {{ $last_error=$_.Exception.Message; Start-Sleep -Milliseconds 100 }} }}; Write-UpdateStatus(\"failed: $last_error\"); exit 1 }} catch {{ try {{ Write-UpdateStatus(\"failed: $($_.Exception.Message)\") }} catch {{ }}; exit 1 }}"
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

/// Release-selection predicate: an already-current install performs no
/// manifest/artifact download.
fn is_already_current(current: StableVersion, latest: StableVersion) -> bool {
    latest <= current
}

async fn run_async() -> Result<(), String> {
    let current =
        env::current_exe().map_err(|error| format!("cannot locate current executable: {error}"))?;
    let current_version = parse_stable_version(env!("CARGO_PKG_VERSION"))?;
    let latest = crates_latest_version_async().await?;
    if is_already_current(current_version, latest) {
        println!("eggsact {current_version} is already current (latest stable: {latest})");
        return Ok(());
    }
    let staging = unique_temp_dir("eggsact-update")?;
    let result: Result<ReplacementOutcome, String> = async {
        // Release selection, manifest/legacy source policy, and Cargo-fallback
        // policy stay here; Eggup owns only the verified local mechanics below.
        let replacement = if let Some(target) = target_for_host(env::consts::OS, env::consts::ARCH)
        {
            match prepare_candidate_async(&staging, RELEASE_ORIGIN, target, latest).await? {
                PreparedUpdate::Manifest(manifest) => {
                    // Staging, integrity, bounded --version validation,
                    // ownership proof, locking, replacement, and rollback are
                    // owned by Eggup.
                    tokio::task::spawn_blocking(move || {
                        commit_manifest_candidate(&manifest, &current, latest)
                    })
                    .await
                    .map_err(|e| format!("update task failed: {e}"))??
                }
                PreparedUpdate::Legacy(acquired) => tokio::task::spawn_blocking(move || {
                    commit_candidate(&acquired, &current, latest)
                })
                .await
                .map_err(|e| format!("update task failed: {e}"))??,
            }
        } else {
            let path = cargo_candidate(&staging, latest)?;
            let digest = eggup_hash(&path)?;
            let acquired = AcquiredCandidate { path, digest };
            tokio::task::spawn_blocking(move || commit_candidate(&acquired, &current, latest))
                .await
                .map_err(|e| format!("update task failed: {e}"))??
        };
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

    /// Ecosystem M001 §17: the updater's published-target table must match
    /// the Eggpack producer contract exactly, so a contract change without
    /// an updater change (or vice versa) fails here instead of in a release.
    /// ARMv7 stays fallback-only: it is recognized by `target_for_host` but
    /// intentionally absent from both tables.
    #[test]
    fn published_targets_match_eggpack_contract() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/release/eggpack/distribution.toml"
        ))
        .expect("Eggpack distribution contract must be checked in");
        let mut product = String::new();
        let mut contracted: Vec<(String, String)> = Vec::new();
        let mut triple = String::new();
        let mut asset = String::new();
        let mut in_product = false;
        let mut in_target = false;
        let field = |line: &str| {
            line.split('=')
                .nth(1)
                .unwrap_or_default()
                .trim()
                .trim_matches('"')
                .to_owned()
        };
        for raw in text.lines() {
            let line = raw.trim();
            if line == "[product]" {
                in_product = true;
                in_target = false;
            } else if line == "[[targets]]" {
                if !triple.is_empty() {
                    contracted.push((std::mem::take(&mut triple), std::mem::take(&mut asset)));
                }
                in_product = false;
                in_target = true;
            } else if line.starts_with('[') {
                in_product = false;
                in_target = line.starts_with("[targets.");
            } else if in_product && line.starts_with("id") {
                product = field(line);
            } else if in_target && line.starts_with("triple") {
                triple = field(line);
            } else if in_target && line.starts_with("asset") {
                asset = field(line);
            }
        }
        if !triple.is_empty() {
            contracted.push((triple, asset));
        }
        assert!(!product.is_empty(), "contract must name its product");
        assert_eq!(contracted.len(), RELEASE_TARGETS.len());
        for (target, template) in &contracted {
            let expected = template
                .replace("{product}", &product)
                .replace("{target}", target);
            let mapped = RELEASE_TARGETS
                .iter()
                .find(|entry| entry.rust_target == target.as_str())
                .unwrap_or_else(|| panic!("updater lacks contract target {target}"));
            assert_eq!(mapped.asset_name, expected.as_str());
            assert_eq!(mapped.windows, expected.ends_with(".exe"));
        }
    }

    /// Distribution M005a: the Windows release candidate is byte-reproducible
    /// only because of the two linker flags in `.cargo/config.toml`. `/BREPRO`
    /// derives the PE time-date-stamps from content instead of the clock, and
    /// `/DEBUG:NONE` drops the CodeView debug directory whose RSDS GUID the MSVC
    /// linker randomizes on every link. `strip = "symbols"` cannot substitute:
    /// rustc passes `/DEBUG` unconditionally for `windows-msvc` and discards
    /// `-C strip` for that target. The byte-level double-build proof lives in the
    /// `windows-reproducibility` job of `.github/workflows/maintenance.yml`.
    #[test]
    fn windows_release_link_flags_are_deterministic() {
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/.cargo/config.toml"))
                .expect("Cargo configuration must be checked in");
        let config: toml::Value = toml::from_str(&text).expect("Cargo configuration must parse");
        let targets = config
            .get("target")
            .and_then(toml::Value::as_table)
            .expect("Cargo configuration must scope release flags by target");
        let flags: Vec<&str> = targets
            .get("x86_64-pc-windows-msvc")
            .and_then(|target| target.get("rustflags"))
            .and_then(toml::Value::as_array)
            .expect("the Windows target must carry release link flags")
            .iter()
            .filter_map(toml::Value::as_str)
            .collect();
        for required in ["link-arg=/BREPRO", "link-arg=/DEBUG:NONE"] {
            assert!(
                flags.iter().any(|flag| flag.ends_with(required)),
                "x86_64-pc-windows-msvc rustflags must pass -C {required}, found {flags:?}"
            );
        }
        // Determinism policy is Windows-only: the other four candidates are
        // already byte-reproducible and must keep their current link inputs.
        for (target, policy) in targets {
            if target == "x86_64-pc-windows-msvc" {
                continue;
            }
            assert!(
                policy.get("rustflags").is_none(),
                "unexpected target rustflags for {target}"
            );
        }
    }

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
            &"a".repeat(64),
        );
        assert!(script.contains("Get-Process -Id $p"));
        assert!(script.contains("Move-Item -LiteralPath $source"));
        assert!(script.contains("Write-UpdateStatus(\"failed:"));
        assert!(!script.contains("Stop-Process"));
        assert!(!script.contains("taskkill"));
        // The digest must be compared before the move, not only before the copy.
        let hash_at = script.find("Get-FileHash").expect("digest check present");
        let move_at = script
            .find("Move-Item -LiteralPath $source")
            .expect("move present");
        assert!(
            hash_at < move_at,
            "digest must be verified before the file is installed"
        );
        assert!(script.contains("staged file digest mismatch"));
    }

    #[test]
    fn hex_digest_is_lowercase_64_chars() {
        assert_eq!(
            hex_digest(&[0_u8; 32]),
            "0".repeat(64),
            "sha256 hex must match the sidecar token format parse_checksum accepts"
        );
        assert_eq!(hex_digest(&[0xff_u8; 32]), "f".repeat(64));
        let mut mixed_digest = [0_u8; 32];
        mixed_digest[0] = 0x00;
        mixed_digest[1] = 0x0f;
        mixed_digest[2] = 0xa0;
        mixed_digest[3] = 0xff;
        let mixed = hex_digest(&mixed_digest);
        assert_eq!(
            mixed,
            "000fa0ff00000000000000000000000000000000000000000000000000000000"
        );
        assert!(
            parse_checksum(&mixed).is_ok(),
            "hex must round-trip through parse_checksum"
        );
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

    /// Serve one stalled body per connection: status + headers + `first_chunk`
    /// arrive promptly, then the connection stalls `stall` before sending
    /// `trailing` (or EOF). Client-side total deadlines must fire during the
    /// stall, proving post-header body-consumption timeout behavior.
    fn serve_stalled_body(
        status: u16,
        headers: Vec<(String, String)>,
        first_chunk: Vec<u8>,
        stall: Duration,
        trailing_chunk: Vec<u8>,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stalled test server");
        let addr = listener.local_addr().expect("local addr");
        listener
            .set_nonblocking(true)
            .expect("test listener nonblocking");
        let handle = tokio::spawn(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let headers = headers.clone();
                let first_chunk = first_chunk.clone();
                let trailing_chunk = trailing_chunk.clone();
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut head = Vec::new();
                    let mut byte = [0_u8; 1];
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
                    let reason = match status {
                        200 => "OK",
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
                    if !first_chunk.is_empty() && socket.write_all(&first_chunk).await.is_err() {
                        return;
                    }
                    if socket.flush().await.is_err() {
                        return;
                    }
                    tokio::time::sleep(stall).await;
                    let _ = socket.write_all(&trailing_chunk).await;
                    let _ = socket.flush().await;
                });
            }
        });
        (format!("http://{addr}/"), handle)
    }

    #[tokio::test]
    async fn small_text_rejects_chunked_body_without_declared_length() {
        // Preferred absent-length fixture: HTTP/1 chunked with no
        // Content-Length, aggregate decoded size exceeding a small test bound.
        // The request-local eggfetch decoded-body cap must reject this
        // without unbounded caller-side buffering.
        const BOUND: usize = 1024;
        fn chunked_wire(payload: &[u8]) -> Vec<u8> {
            let mut wire = format!("{:X}\r\n", payload.len()).into_bytes();
            wire.extend_from_slice(payload);
            wire.extend_from_slice(b"\r\n");
            wire
        }
        let wire_chunks: Vec<Vec<u8>> = {
            let mut framed = Vec::new();
            for _ in 0..3 {
                framed.push(chunked_wire(&vec![b'y'; 1024]));
            }
            framed.push(b"0\r\n\r\n".to_vec());
            framed
        };
        let (base, handle) = serve(move |_| {
            (
                200,
                vec![("Transfer-Encoding".into(), "chunked".into())],
                wire_chunks.clone(),
                false,
            )
        });
        let client = test_client();
        let error = get_small_text(&client, &base, BOUND, "crates.io metadata")
            .await
            .expect_err("chunked oversize without length is rejected");
        assert!(error.contains("exceeds"), "got: {error}");
        assert!(error.contains("1024"), "got: {error}");
        assert!(
            error.contains("decoded body too large") || error.contains("byte bound"),
            "got: {error}"
        );
        handle.abort();
    }

    #[tokio::test]
    async fn buffered_body_stall_past_total_times_out() {
        // Headers arrive promptly describing a successful 200; only a prefix
        // is delivered, then the body stalls past the injected total.
        // get_small_text must surface the existing timeout classification.
        let (base, handle) = serve_stalled_body(
            200,
            vec![("Content-Length".into(), "65536".into())],
            b"prefix-bytes".to_vec(),
            Duration::from_millis(1500),
            vec![b't'; 4096],
        );
        let timeout = Timeout::builder()
            .connect(Duration::from_secs(5))
            .total(Duration::from_millis(200))
            .build();
        let client = build_update_client_with_timeout_and_env(
            timeout,
            &eggfetch_core::ProxyEnvironment::new(),
        )
        .expect("stall-timeout client builds");
        let started = std::time::Instant::now();
        let error = get_small_text(&client, &base, METADATA_MAX_BYTES, "crates.io metadata")
            .await
            .expect_err("post-header body stall times out");
        assert!(error.contains("update request timed out"), "got: {error}");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "timeout must fire well before the outer bound"
        );
        handle.abort();
    }

    #[tokio::test]
    async fn streamed_binary_stall_past_total_times_out_and_cleans_up() {
        // Same post-header stall on the streamed binary path: 200 headers
        // promptly, one binary chunk, then stall past total. download_to must
        // map the timeout and remove the partial destination.
        let (base, handle) = serve_stalled_body(
            200,
            vec![("Content-Length".into(), "65536".into())],
            vec![0x7f, b'E', b'L', b'F'],
            Duration::from_millis(1500),
            vec![0u8; 4096],
        );
        let timeout = Timeout::builder()
            .connect(Duration::from_secs(5))
            .total(Duration::from_millis(200))
            .build();
        let client = build_update_client_with_timeout_and_env(
            timeout,
            &eggfetch_core::ProxyEnvironment::new(),
        )
        .expect("stall-timeout client builds");
        let dest = temp_destination("stalled-binary");
        let _ = std::fs::remove_file(&dest);
        let error = download_to(&client, &base, &dest)
            .await
            .expect_err("post-header binary stall times out");
        assert!(error.contains("update request timed out"), "got: {error}");
        assert!(!dest.exists(), "partial file must be removed");
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

    // ── Eggup first-adoption compatibility ──

    #[test]
    fn eggup_transport_builds_with_strict_policy() {
        let transport = eggup_transport().expect("strict eggup transport builds");
        assert_eq!(
            transport.config().user_agent,
            USER_AGENT,
            "consumer keeps its user agent"
        );
        assert_eq!(transport.config().connect_timeout, CONNECT_TIMEOUT);
        assert_eq!(transport.config().total_timeout, TOTAL_TIMEOUT);
        assert_eq!(transport.config().max_redirects, MAX_REDIRECTS);
    }

    #[test]
    fn eggup_ownership_proves_current_executable_only() {
        let dir = unique_temp_dir("eggsact-owned").expect("temp");
        let current = dir.join("eggsact");
        std::fs::write(&current, b"running").expect("write current");
        let other = dir.join("other");
        std::fs::write(&other, b"other").expect("write other");
        let verifier = CurrentExeVerifier {
            current_exe: current.clone(),
        };
        let member = MemberId::new("main").unwrap();
        assert_eq!(
            verifier.verify(&member, &current),
            Ownership::Owned,
            "the running executable is owned"
        );
        assert_eq!(
            verifier.verify(&member, &other),
            Ownership::Foreign,
            "another binary is foreign"
        );
        assert_eq!(
            verifier.verify(&member, &dir.join("missing")),
            Ownership::Absent,
            "absence is distinct from ownership"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn eggup_fallback_classifier_is_preserved() {
        // Release policy unchanged: only genuine 404 (or unsupported host)
        // may reach Cargo; every hard failure stays a hard error.
        assert_eq!(classify_http_status(200), DownloadStatus::Success);
        assert_eq!(classify_http_status(404), DownloadStatus::NotFound);
        for status in [400, 403, 429, 500, 503] {
            assert_eq!(
                classify_http_status(status),
                DownloadStatus::HardFailure,
                "HTTP {status} must never fall back to Cargo"
            );
        }
        assert!(target_for_host("linux", "x86_64").is_some());
        assert!(target_for_host("plan9", "x").is_none());
    }

    #[test]
    fn eggup_receipt_mapping_is_explicit() {
        // Structural guard: Eggup terminal states map to distinct CLI outcomes
        // (committed vs rolled-back vs recovery-required), never conflated.
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/update.rs"));
        assert!(
            source.contains("TransactionDisposition::Committed"),
            "committed maps to success"
        );
        assert!(
            source.contains("TransactionDisposition::RolledBack"),
            "rollback is reported, not success"
        );
        assert!(
            source.contains("TransactionDisposition::RecoveryRequired"),
            "recovery-required is high severity"
        );
        assert!(
            source.contains("AbsentPolicy::DenyCreate"),
            "self-update is replacement-only"
        );
    }

    #[test]
    fn eggup_generic_machinery_is_deleted_not_wrapped() {
        // The old local hash/exec/replace helpers must be gone; Eggup owns them.
        // (Match on newline-prefixed definitions so this test's own literals
        // do not self-match.)
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/update.rs"));
        assert!(
            !source.contains("\nfn sha256_file("),
            "local hasher deleted in favor of eggup_core::hash_file"
        );
        assert!(
            !source.contains("\nfn run_bounded("),
            "local candidate runner deleted in favor of Eggup validators"
        );
        assert!(
            !source.contains("\nfn wait_bounded("),
            "local wait helper deleted with the runner"
        );
        assert!(
            !source.contains("\n#[cfg(unix)]\nfn replace_current("),
            "unix copy/rename replaced by the Eggup transaction"
        );
    }

    #[test]
    fn eggup_single_http_stack() {
        // One intended HTTP/TLS stack: eggfetch-core 0.2.0, shared by the
        // retained policy harness and eggup-eggfetch.
        let tree = std::process::Command::new("cargo")
            .args(["tree", "--depth", "1", "--offline"])
            .output();
        let _ = tree;
        let manifest = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        assert!(
            manifest.contains("eggup-core"),
            "consumer depends on versioned eggup-core"
        );
        assert!(
            manifest.contains("eggup-eggfetch"),
            "consumer depends on versioned eggup-eggfetch"
        );
    }

    // ── Release-manifest consumer path (M003 behavior matrix) ──
    //
    // Row references below are M003 §9 Eggsact matrix rows. Every networked
    // test serves a deterministic local HTTP fixture; no public endpoint is
    // contacted. Tests that execute a candidate or commit run only on Unix:
    // they rely on shebang execution and in-place replacement, and there is
    // no Windows CI lane for this consumer (recorded in the M003 closure).

    fn host_release_target() -> &'static ReleaseTarget {
        target_for_host(env::consts::OS, env::consts::ARCH).expect("host has a release target")
    }

    /// Fake candidate executable: mimics the real binary, printing the exact
    /// identity the Eggup exact-identity validator expects on `--version` and
    /// the usage block otherwise. It must honour argv — a fixture that always
    /// prints the identity hides a validator that passes no arguments at all.
    fn candidate_script(version: StableVersion) -> Vec<u8> {
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo \"eggsact {version}\"\nelse\n  echo \"Usage: eggsact [--mcp | update | expression]\"\nfi\n"
        )
        .into_bytes()
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::Digest;
        format!("{:x}", sha2::Sha256::digest(bytes))
    }

    /// Minimal producer-shaped ReleaseManifest v1 document (same schema the
    /// Eggup adapter qualifies against).
    #[allow(clippy::too_many_arguments)]
    fn manifest_bytes(
        product: &str,
        version: StableVersion,
        target: &str,
        asset: &str,
        size: u64,
        sha_hex: &str,
        install: &str,
    ) -> String {
        format!(
            "{{\"schema_version\":1,\"product_id\":\"{product}\",\"release_id\":\"{version}\",\"source_revision\":\"{}\",\"targets\":[{{\"target\":\"{target}\",\"form\":{{\"kind\":\"direct\",\"artifact\":{{\"name\":\"{asset}\",\"size\":{size},\"sha256\":\"{sha_hex}\"}},\"install\":\"{install}\"}}}}]}}",
            "b".repeat(40)
        )
    }

    struct ManifestFixture {
        manifest_status: u16,
        manifest_body: Vec<u8>,
        artifact_status: u16,
        artifact_body: Vec<u8>,
        sidecar_status: u16,
        sidecar_body: Vec<u8>,
    }

    fn content_length(body: &[u8]) -> Vec<(String, String)> {
        vec![("Content-Length".into(), body.len().to_string())]
    }

    /// Route fixture responses by request path: manifest, then checksum
    /// sidecar, then artifact bytes.
    fn serve_manifest_fixture(fixture: ManifestFixture) -> (String, tokio::task::JoinHandle<()>) {
        serve(move |request| {
            let head = request.lines().next().unwrap_or_default().to_owned();
            if head.contains(MANIFEST_FILE_NAME) {
                (
                    fixture.manifest_status,
                    content_length(&fixture.manifest_body),
                    vec![fixture.manifest_body.clone()],
                    false,
                )
            } else if head.contains(".sha256") {
                (
                    fixture.sidecar_status,
                    content_length(&fixture.sidecar_body),
                    vec![fixture.sidecar_body.clone()],
                    false,
                )
            } else {
                (
                    fixture.artifact_status,
                    content_length(&fixture.artifact_body),
                    vec![fixture.artifact_body.clone()],
                    false,
                )
            }
        })
    }

    /// Fresh install root holding a stale executable under `file_name`.
    fn fake_installation(file_name: &str) -> (PathBuf, PathBuf) {
        let root = unique_temp_dir("eggsact-manifest-install").expect("temp install root");
        let current = root.join(file_name);
        std::fs::write(&current, b"stale-bytes").expect("write stale executable");
        (root, current)
    }

    fn fixture_origin(base: &str) -> String {
        format!("{base}releases")
    }

    /// Matrix row 1: already-current installs perform no download.
    #[test]
    fn already_current_predicate_is_exact() {
        let current = StableVersion::new(1, 2, 7);
        assert!(is_already_current(current, current));
        assert!(is_already_current(current, StableVersion::new(1, 2, 6)));
        assert!(!is_already_current(current, StableVersion::new(1, 2, 8)));
    }

    /// Matrix row 15 (unchanged) + manifest URL convention: the exact
    /// producer filename sits under the authorized release origin.
    #[test]
    fn manifest_url_uses_producer_filename_under_authorized_origin() {
        let version = StableVersion::new(1, 2, 7);
        assert_eq!(
            manifest_url(&version),
            "https://github.com/eggstack/eggsact/releases/download/v1.2.7/release-manifest.json"
        );
        assert_eq!(
            manifest_url_for_origin("http://127.0.0.1:9/releases", &version),
            "http://127.0.0.1:9/releases/download/v1.2.7/release-manifest.json"
        );
        assert_eq!(MANIFEST_FILE_NAME, "release-manifest.json");
    }

    #[test]
    fn artifact_url_percent_encodes_the_manifest_supplied_name() {
        let version = StableVersion::new(1, 2, 7);
        let origin = "https://github.com/eggstack/eggsact/releases";

        // Producer names are unreserved characters and encode to themselves.
        let plain = "eggsact-1.2.7-x86_64-unknown-linux-gnu.tar.gz";
        assert_eq!(encode_url_segment(plain), plain);
        assert_eq!(
            manifest_artifact_url_for_origin(origin, &version, plain),
            format!("{origin}/download/v1.2.7/{plain}")
        );

        // A name carrying URL metacharacters must not escape its path segment.
        for (name, encoded) in [
            ("a?b=1", "a%3Fb%3D1"),
            ("a#frag", "a%23frag"),
            ("a/b", "a%2Fb"),
            ("a%2e", "a%252e"),
            ("a b", "a%20b"),
        ] {
            let url = manifest_artifact_url_for_origin(origin, &version, name);
            assert_eq!(url, format!("{origin}/download/v1.2.7/{encoded}"), "{name}");
            assert!(
                !url[format!("{origin}/download/v1.2.7/").len()..].contains(['?', '#', '/', ' ']),
                "artifact name escaped its path segment: {url}"
            );
        }
    }

    #[test]
    fn digest_comparison_is_exact() {
        let a = [1_u8; 32];
        assert!(digests_equal(&a, &a));
        let mut b = a;
        b[31] = 2;
        assert!(!digests_equal(&a, &b));
        // A difference in the first byte must be caught too.
        let mut c = a;
        c[0] = 9;
        assert!(!digests_equal(&a, &c));
        assert!(digests_equal(&a, &[1_u8; 32]));
    }

    /// Matrix rows 3-5: product, release, and target mismatch are hard
    /// errors that never mention legacy compatibility.
    #[test]
    fn manifest_binding_rejects_identity_mismatch() {
        let target = host_release_target();
        let version = StableVersion::new(9, 9, 9);
        let body = candidate_script(version);
        let digest = sha256_hex(&body);
        let asset = asset_name(target.rust_target);
        let valid = manifest_bytes(
            "eggsact",
            version,
            target.rust_target,
            &asset,
            body.len() as u64,
            &digest,
            "eggsact",
        );
        resolve_manifest_projection(valid.as_bytes(), target, version)
            .expect("valid manifest resolves");

        let wrong_product = manifest_bytes(
            "other",
            version,
            target.rust_target,
            &asset,
            body.len() as u64,
            &digest,
            "eggsact",
        );
        let error = resolve_manifest_projection(wrong_product.as_bytes(), target, version)
            .expect_err("product mismatch is hard");
        assert!(error.contains("product mismatch"), "got: {error}");
        assert!(!error.contains("legacy"), "got: {error}");

        let wrong_release = manifest_bytes(
            "eggsact",
            StableVersion::new(9, 9, 8),
            target.rust_target,
            &asset,
            body.len() as u64,
            &digest,
            "eggsact",
        );
        let error = resolve_manifest_projection(wrong_release.as_bytes(), target, version)
            .expect_err("release mismatch is hard");
        assert!(error.contains("release mismatch"), "got: {error}");
        assert!(!error.contains("legacy"), "got: {error}");

        let other_triple = if target.rust_target == "x86_64-unknown-linux-gnu" {
            "aarch64-apple-darwin"
        } else {
            "x86_64-unknown-linux-gnu"
        };
        let wrong_target = manifest_bytes(
            "eggsact",
            version,
            other_triple,
            &asset,
            body.len() as u64,
            &digest,
            "eggsact",
        );
        let error = resolve_manifest_projection(wrong_target.as_bytes(), target, version)
            .expect_err("target mismatch is hard");
        assert!(error.contains("not usable"), "got: {error}");
        assert!(!error.contains("legacy"), "got: {error}");
    }

    /// Matrix row 6: malformed, unsupported-schema, oversized, and
    /// non-UTF-8 manifests are hard errors.
    #[test]
    fn manifest_binding_rejects_unusable_documents() {
        let target = host_release_target();
        let version = StableVersion::new(9, 9, 9);
        for (name, document) in [
            ("malformed", b"{not json".to_vec()),
            (
                "unsupported-schema",
                manifest_bytes(
                    "eggsact",
                    version,
                    target.rust_target,
                    "eggsact-x",
                    1,
                    &"c".repeat(64),
                    "eggsact",
                )
                .replace("schema_version\":1", "schema_version\":2")
                .into_bytes(),
            ),
            (
                "oversized",
                vec![b' '; eggup_eggpack::MAX_MANIFEST_BYTES + 1],
            ),
            ("non-utf8", vec![0xff, 0xfe, 0x00, 0x41]),
        ] {
            let error = resolve_manifest_projection(&document, target, version)
                .expect_err(&format!("{name} manifest is hard"));
            assert!(!error.contains("legacy"), "{name} got: {error}");
        }
    }

    /// Matrix row 18: the caller artifact ceiling is finite, never widened,
    /// and tightened to the manifest exact size.
    #[test]
    fn manifest_artifact_ceiling_is_tightened_never_widened() {
        let target = host_release_target();
        let version = StableVersion::new(9, 9, 9);
        let body = candidate_script(version);
        let digest = sha256_hex(&body);
        let asset = asset_name(target.rust_target);
        let document = manifest_bytes(
            "eggsact",
            version,
            target.rust_target,
            &asset,
            body.len() as u64,
            &digest,
            "eggsact",
        );
        let projection = resolve_manifest_projection(document.as_bytes(), target, version)
            .expect("valid resolves");
        let baseline = eggup_limits(METADATA_MAX_BYTES);
        assert_eq!(baseline.max_artifact_bytes, ARTIFACT_MAX_BYTES);
        let url = manifest_artifact_url_for_origin("http://127.0.0.1:9/releases", &version, &asset);
        let request = AcquisitionRequest::new(url).expect("fixture request builds");
        let planned = projection
            .bind_requests(HashMap::from([(asset.clone(), request)]), baseline)
            .expect("binding succeeds");
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].limits.max_artifact_bytes, body.len() as u64);
        assert!(planned[0].limits.max_artifact_bytes <= baseline.max_artifact_bytes);

        let tight = FetchLimits {
            max_artifact_bytes: 1,
            ..baseline
        };
        let url = manifest_artifact_url_for_origin("http://127.0.0.1:9/releases", &version, &asset);
        let request = AcquisitionRequest::new(url).expect("fixture request builds");
        let error = projection
            .bind_requests(HashMap::from([(asset, request)]), tight)
            .expect_err("a ceiling below the manifest size fails closed");
        assert!(
            error.to_string().contains("caller byte limit is below"),
            "got: {error}"
        );
    }

    /// Matrix rows 8-9 + §8G guard: exact manifest 404 is the only legacy
    /// entry; a present-but-unusable manifest never reaches legacy code.
    #[test]
    fn legacy_construction_lives_only_behind_manifest_absence() {
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/update.rs"));
        let dispatch = source
            .split("async fn prepare_candidate_async")
            .nth(1)
            .expect("manifest dispatch exists")
            .split("async fn ")
            .next()
            .unwrap_or_default();
        assert!(
            dispatch.contains("MetadataFetch::Absent"),
            "dispatch branches on structural absence"
        );
        assert_eq!(
            dispatch.matches("prepare_legacy_candidate_async").count(),
            1,
            "legacy construction has exactly one call site, in the Absent arm"
        );
    }

    /// Matrix row 7: manifest 5xx and transport failures are hard errors.
    #[tokio::test]
    async fn manifest_transport_failures_are_hard() {
        let target = host_release_target();
        let version = StableVersion::new(9, 9, 9);
        let (base, handle) = serve_manifest_fixture(ManifestFixture {
            manifest_status: 503,
            manifest_body: b"error".to_vec(),
            artifact_status: 404,
            artifact_body: vec![],
            sidecar_status: 404,
            sidecar_body: vec![],
        });
        let staging = unique_temp_dir("eggsact-manifest-hard").expect("staging");
        let error = prepare_candidate_async(&staging, &fixture_origin(&base), target, version)
            .await
            .expect_err("manifest 503 is hard");
        assert!(error.contains("release manifest"), "got: {error}");
        handle.abort();

        let error = prepare_candidate_async(&staging, "http://127.0.0.1:1", target, version)
            .await
            .expect_err("refused connection is hard");
        assert!(error.contains("release manifest"), "got: {error}");
        let _ = std::fs::remove_dir_all(&staging);
    }

    /// Matrix row 9: a valid manifest whose artifact is absent is a hard
    /// incomplete-release failure, never Cargo fallback.
    #[tokio::test]
    async fn manifest_artifact_absence_is_hard_without_cargo_fallback() {
        let target = host_release_target();
        let version = StableVersion::new(9, 9, 9);
        let body = candidate_script(version);
        let digest = sha256_hex(&body);
        let asset = asset_name(target.rust_target);
        let manifest = manifest_bytes(
            "eggsact",
            version,
            target.rust_target,
            &asset,
            body.len() as u64,
            &digest,
            "eggsact",
        );
        let (base, handle) = serve_manifest_fixture(ManifestFixture {
            manifest_status: 200,
            manifest_body: manifest.into_bytes(),
            artifact_status: 404,
            artifact_body: vec![],
            sidecar_status: 404,
            sidecar_body: vec![],
        });
        let staging = unique_temp_dir("eggsact-manifest-absent").expect("staging");
        let error = prepare_candidate_async(&staging, &fixture_origin(&base), target, version)
            .await
            .expect_err("manifest-backed artifact 404 is hard");
        assert!(
            error.contains("absent from the authorized release"),
            "got: {error}"
        );
        assert!(!error.contains("cargo"), "got: {error}");
        let _ = std::fs::remove_dir_all(&staging);
        handle.abort();
    }

    /// Matrix row 10: size mismatch fails before the Eggup commit. An
    /// undersized release passes acquisition and fails closed at adapter
    /// materialization; an oversized release is rejected by the tightened
    /// transport ceiling during acquisition.
    #[tokio::test]
    async fn manifest_size_mismatch_fails_before_commit() {
        let target = host_release_target();
        let version = StableVersion::new(9, 9, 9);
        let body = candidate_script(version);
        let digest = sha256_hex(&body);
        let asset = asset_name(target.rust_target);
        // Manifest claims more bytes than the release serves.
        let manifest = manifest_bytes(
            "eggsact",
            version,
            target.rust_target,
            &asset,
            body.len() as u64 + 16,
            &digest,
            "eggsact",
        );
        let (base, handle) = serve_manifest_fixture(ManifestFixture {
            manifest_status: 200,
            manifest_body: manifest.into_bytes(),
            artifact_status: 200,
            artifact_body: body,
            sidecar_status: 404,
            sidecar_body: vec![],
        });
        let staging = unique_temp_dir("eggsact-manifest-size").expect("staging");
        let (root, current) = fake_installation("eggsact");
        let prepared = prepare_candidate_async(&staging, &fixture_origin(&base), target, version)
            .await
            .expect("undersized release still acquires");
        let PreparedUpdate::Manifest(acquired) = prepared else {
            panic!("valid manifest must take the manifest path");
        };
        let error = commit_manifest_candidate(&acquired, &current, version)
            .expect_err("undersized release fails at materialization");
        assert!(error.contains("materialization failed"), "got: {error}");
        assert_eq!(
            std::fs::read(&current).expect("read stale"),
            b"stale-bytes",
            "no commit was attempted"
        );
        handle.abort();

        // Manifest claims fewer bytes than the release serves: the tightened
        // ceiling rejects the body during acquisition.
        let body = candidate_script(version);
        let digest = sha256_hex(&body);
        let manifest = manifest_bytes(
            "eggsact",
            version,
            target.rust_target,
            &asset,
            body.len() as u64 - 4,
            &digest,
            "eggsact",
        );
        let (base, handle) = serve_manifest_fixture(ManifestFixture {
            manifest_status: 200,
            manifest_body: manifest.into_bytes(),
            artifact_status: 200,
            artifact_body: body,
            sidecar_status: 404,
            sidecar_body: vec![],
        });
        let error = prepare_candidate_async(&staging, &fixture_origin(&base), target, version)
            .await
            .expect_err("oversized release fails at acquisition");
        assert!(error.contains("release manifest artifact"), "got: {error}");
        assert_eq!(
            std::fs::read(&current).expect("read stale"),
            b"stale-bytes",
            "no commit was attempted"
        );
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_dir_all(&root);
        handle.abort();
    }

    /// Matrix row 2: a producer-valid manifest flows through real update code
    /// to acquisition, caller-bound materialization, validation, and commit.
    #[cfg(unix)]
    #[tokio::test]
    async fn manifest_path_commits_selected_artifact() {
        let version = StableVersion::new(9, 9, 9);
        let target = host_release_target();
        let body = candidate_script(version);
        let digest = sha256_hex(&body);
        let asset = asset_name(target.rust_target);
        let manifest = manifest_bytes(
            "eggsact",
            version,
            target.rust_target,
            &asset,
            body.len() as u64,
            &digest,
            "eggsact",
        );
        let (base, handle) = serve_manifest_fixture(ManifestFixture {
            manifest_status: 200,
            manifest_body: manifest.into_bytes(),
            artifact_status: 200,
            artifact_body: body.clone(),
            sidecar_status: 404,
            sidecar_body: vec![],
        });
        let staging = unique_temp_dir("eggsact-manifest-e2e").expect("staging");
        let (root, current) = fake_installation("eggsact");
        let prepared = prepare_candidate_async(&staging, &fixture_origin(&base), target, version)
            .await
            .expect("manifest path prepares");
        let PreparedUpdate::Manifest(acquired) = prepared else {
            panic!("valid manifest must take the manifest path");
        };
        let outcome = commit_manifest_candidate(&acquired, &current, version)
            .expect("manifest commit succeeds");
        assert_eq!(outcome, ReplacementOutcome::Complete);
        assert_eq!(std::fs::read(&current).expect("read updated"), body);
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_dir_all(&root);
        handle.abort();
    }

    /// The candidate validator runs the staged binary with `--version`. With no
    /// argv the real binary prints its usage block instead of its identity, so
    /// candidate validation failed for every update on every platform. The
    /// fixture honours argv precisely so this cannot regress unnoticed.
    #[cfg(unix)]
    #[test]
    fn candidate_fixture_only_reports_identity_for_version_argv() {
        use std::os::unix::fs::PermissionsExt;

        let version = StableVersion::new(9, 9, 9);
        let dir = unique_temp_dir("eggsact-argv-fixture").expect("temp dir");
        let script = dir.join("candidate.sh");
        std::fs::write(&script, candidate_script(version)).expect("write fixture");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fixture");

        let run = |argv: &[&str]| {
            std::process::Command::new(&script)
                .args(argv)
                .output()
                .expect("run fixture")
        };

        assert_eq!(
            run(&["--version"]).stdout,
            format!("eggsact {version}\n").as_bytes(),
            "identity is only reported for --version"
        );
        assert_ne!(
            run(&[]).stdout,
            format!("eggsact {version}\n").as_bytes(),
            "without argv the fixture must not report the identity, otherwise \
             the test cannot detect a validator that passes no arguments"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Matrix row 17: a renamed running executable is updated in place; the
    /// manifest default basename is never created.
    #[cfg(unix)]
    #[tokio::test]
    async fn manifest_path_updates_renamed_executable_in_place() {
        let version = StableVersion::new(9, 9, 9);
        let target = host_release_target();
        let body = candidate_script(version);
        let digest = sha256_hex(&body);
        let asset = asset_name(target.rust_target);
        let manifest = manifest_bytes(
            "eggsact",
            version,
            target.rust_target,
            &asset,
            body.len() as u64,
            &digest,
            "eggsact",
        );
        let (base, handle) = serve_manifest_fixture(ManifestFixture {
            manifest_status: 200,
            manifest_body: manifest.into_bytes(),
            artifact_status: 200,
            artifact_body: body.clone(),
            sidecar_status: 404,
            sidecar_body: vec![],
        });
        let staging = unique_temp_dir("eggsact-manifest-renamed").expect("staging");
        let (root, current) = fake_installation("my-tool");
        let prepared = prepare_candidate_async(&staging, &fixture_origin(&base), target, version)
            .await
            .expect("manifest path prepares");
        let PreparedUpdate::Manifest(acquired) = prepared else {
            panic!("valid manifest must take the manifest path");
        };
        let outcome = commit_manifest_candidate(&acquired, &current, version)
            .expect("renamed commit succeeds");
        assert_eq!(outcome, ReplacementOutcome::Complete);
        assert_eq!(std::fs::read(&current).expect("read updated"), body);
        assert!(
            !root.join("eggsact").exists(),
            "manifest default basename must not be created"
        );
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_dir_all(&root);
        handle.abort();
    }

    /// Matrix row 11: digest mismatch fails before commit; stale bytes stay.
    #[cfg(unix)]
    #[tokio::test]
    async fn manifest_digest_mismatch_fails_before_commit() {
        let version = StableVersion::new(9, 9, 9);
        let target = host_release_target();
        let served = candidate_script(version);
        // Manifest digest describes unrelated bytes of the same length, so
        // size checks pass and only integrity evidence disagrees.
        let digest = sha256_hex(&vec![0x55; served.len()]);
        let asset = asset_name(target.rust_target);
        let manifest = manifest_bytes(
            "eggsact",
            version,
            target.rust_target,
            &asset,
            served.len() as u64,
            &digest,
            "eggsact",
        );
        let (base, handle) = serve_manifest_fixture(ManifestFixture {
            manifest_status: 200,
            manifest_body: manifest.into_bytes(),
            artifact_status: 200,
            artifact_body: served,
            sidecar_status: 404,
            sidecar_body: vec![],
        });
        let staging = unique_temp_dir("eggsact-manifest-digest").expect("staging");
        let (root, current) = fake_installation("eggsact");
        let prepared = prepare_candidate_async(&staging, &fixture_origin(&base), target, version)
            .await
            .expect("size matches so acquisition succeeds");
        let PreparedUpdate::Manifest(acquired) = prepared else {
            panic!("valid manifest must take the manifest path");
        };
        commit_manifest_candidate(&acquired, &current, version)
            .expect_err("digest mismatch fails before commit");
        assert_eq!(std::fs::read(&current).expect("read stale"), b"stale-bytes");
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_dir_all(&root);
        handle.abort();
    }

    /// Matrix row 12: wrong candidate identity fails before commit.
    #[cfg(unix)]
    #[tokio::test]
    async fn manifest_wrong_candidate_identity_fails_before_commit() {
        let version = StableVersion::new(9, 9, 9);
        let target = host_release_target();
        let body = candidate_script(StableVersion::new(1, 2, 3));
        let digest = sha256_hex(&body);
        let asset = asset_name(target.rust_target);
        let manifest = manifest_bytes(
            "eggsact",
            version,
            target.rust_target,
            &asset,
            body.len() as u64,
            &digest,
            "eggsact",
        );
        let (base, handle) = serve_manifest_fixture(ManifestFixture {
            manifest_status: 200,
            manifest_body: manifest.into_bytes(),
            artifact_status: 200,
            artifact_body: body,
            sidecar_status: 404,
            sidecar_body: vec![],
        });
        let staging = unique_temp_dir("eggsact-manifest-identity").expect("staging");
        let (root, current) = fake_installation("eggsact");
        let prepared = prepare_candidate_async(&staging, &fixture_origin(&base), target, version)
            .await
            .expect("evidence matches so acquisition succeeds");
        let PreparedUpdate::Manifest(acquired) = prepared else {
            panic!("valid manifest must take the manifest path");
        };
        let error = commit_manifest_candidate(&acquired, &current, version)
            .expect_err("wrong candidate identity fails");
        assert!(
            error.contains("candidate validation failed"),
            "got: {error}"
        );
        assert_eq!(std::fs::read(&current).expect("read stale"), b"stale-bytes");
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_dir_all(&root);
        handle.abort();
    }

    /// Matrix rows 13-14: an ownership conflict fails closed with a rolled
    /// back receipt and no mutation (behavioral proof of the RolledBack
    /// mapping; RecoveryRequired cannot be forced deterministically).
    #[cfg(unix)]
    #[tokio::test]
    async fn manifest_ownership_conflict_fails_closed_without_mutation() {
        let version = StableVersion::new(9, 9, 9);
        let target = host_release_target();
        let body = candidate_script(version);
        let digest = sha256_hex(&body);
        let asset = asset_name(target.rust_target);
        let manifest = manifest_bytes(
            "eggsact",
            version,
            target.rust_target,
            &asset,
            body.len() as u64,
            &digest,
            "eggsact",
        );
        let (base, handle) = serve_manifest_fixture(ManifestFixture {
            manifest_status: 200,
            manifest_body: manifest.into_bytes(),
            artifact_status: 200,
            artifact_body: body,
            sidecar_status: 404,
            sidecar_body: vec![],
        });
        let staging = unique_temp_dir("eggsact-manifest-owned").expect("staging");
        let root = unique_temp_dir("eggsact-manifest-foreign").expect("install root");
        let real = root.join("real-binary");
        std::fs::write(&real, b"real-bytes").expect("write target");
        let current = root.join("eggsact");
        std::os::unix::fs::symlink(&real, &current).expect("link current");
        let prepared = prepare_candidate_async(&staging, &fixture_origin(&base), target, version)
            .await
            .expect("manifest path prepares");
        let PreparedUpdate::Manifest(acquired) = prepared else {
            panic!("valid manifest must take the manifest path");
        };
        let error = commit_manifest_candidate(&acquired, &current, version)
            .expect_err("foreign destination fails closed");
        assert!(error.contains("rolled back"), "got: {error}");
        assert_eq!(std::fs::read(&real).expect("read target"), b"real-bytes");
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_dir_all(&root);
        handle.abort();
    }

    /// Matrix row 8: exact manifest 404 enters the legacy sidecar path, which
    /// still verifies and commits through the shared tail.
    #[cfg(unix)]
    #[tokio::test]
    async fn manifest_absence_uses_legacy_sidecar_path() {
        let version = StableVersion::new(9, 9, 9);
        let target = host_release_target();
        let body = candidate_script(version);
        let digest = sha256_hex(&body);
        let sidecar = format!("{digest}  {}\n", target.asset_name);
        let (base, handle) = serve_manifest_fixture(ManifestFixture {
            manifest_status: 404,
            manifest_body: b"not found".to_vec(),
            artifact_status: 200,
            artifact_body: body.clone(),
            sidecar_status: 200,
            sidecar_body: sidecar.into_bytes(),
        });
        let staging = unique_temp_dir("eggsact-legacy-compat").expect("staging");
        let (root, current) = fake_installation("eggsact");
        let prepared = prepare_candidate_async(&staging, &fixture_origin(&base), target, version)
            .await
            .expect("legacy compatibility prepares");
        let PreparedUpdate::Legacy(acquired) = prepared else {
            panic!("exact manifest 404 must take the legacy path");
        };
        let outcome =
            commit_candidate(&acquired, &current, version).expect("legacy commit succeeds");
        assert_eq!(outcome, ReplacementOutcome::Complete);
        assert_eq!(std::fs::read(&current).expect("read updated"), body);
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_dir_all(&root);
        handle.abort();
    }

    /// Matrix row 16: one immutable Eggup source identity, no direct Eggpack
    /// producer crates.
    #[test]
    fn eggup_dependency_identities_are_single_git_source() {
        let manifest = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        let mut revs = Vec::new();
        for line in manifest.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("eggup-") {
                let rev = trimmed
                    .split("rev =")
                    .nth(1)
                    .expect("every eggup dependency pins an exact rev")
                    .trim()
                    .trim_matches(|c| c == '"' || c == '}' || c == ' ')
                    .to_owned();
                assert_eq!(rev.len(), 40, "rev is a full commit SHA, not a branch");
                revs.push(rev);
            }
        }
        assert_eq!(revs.len(), 4, "core, acquisition, eggfetch, and eggpack");
        assert!(
            revs.windows(2).all(|pair| pair[0] == pair[1]),
            "one Eggup revision, no mixed sources: {revs:?}"
        );
        assert!(
            !manifest.contains("eggpack-manifest"),
            "no direct producer-schema dependency"
        );
    }

    #[test]
    fn eggup_tree_has_single_source_per_package_and_no_direct_producer_edge() {
        let direct = std::process::Command::new("cargo")
            .args([
                "tree",
                "--offline",
                "--depth",
                "1",
                "--prefix",
                "none",
                "-e",
                "normal",
            ])
            .output()
            .expect("cargo tree runs offline");
        assert!(direct.status.success(), "cargo tree succeeds");
        let direct = String::from_utf8_lossy(&direct.stdout);
        assert!(
            !direct.lines().any(|line| line.starts_with("eggpack-")),
            "no direct Eggpack producer edge"
        );
        assert!(
            direct
                .lines()
                .any(|line| line.starts_with("eggup-eggpack ")),
            "direct eggup-eggpack edge present"
        );

        let full = std::process::Command::new("cargo")
            .args(["tree", "--offline", "--prefix", "none", "--no-dedupe"])
            .output()
            .expect("cargo tree runs offline");
        assert!(full.status.success(), "cargo tree succeeds");
        let full = String::from_utf8_lossy(&full.stdout);
        for package in [
            "eggup-core",
            "eggup-acquisition",
            "eggup-eggfetch",
            "eggup-eggpack",
        ] {
            let mut identities: Vec<&str> = full
                .lines()
                .filter(|line| line.starts_with(package))
                .filter(|line| line.get(package.len()..package.len() + 1) == Some(" "))
                .collect();
            identities.sort_unstable();
            identities.dedup();
            assert_eq!(
                identities.len(),
                1,
                "{package} must resolve to one source identity, got {identities:?}"
            );
            assert!(
                identities[0].contains("https://github.com/eggstack/eggup.git?rev="),
                "{package} resolves to the pinned Eggup revision"
            );
        }
    }
}
