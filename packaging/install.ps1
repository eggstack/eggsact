param(
    [string]$Version
)

$ErrorActionPreference = "Stop"
$repository = "eggstack/eggsact"
$baseUrl = "https://github.com/$repository/releases"
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
if ($Version -and $Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') { throw "-Version must be X.Y.Z" }

$temporary = Join-Path ([System.IO.Path]::GetTempPath()) ("eggsact-install-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    $releasePath = if ($Version) { "download/v$Version" } else { "latest/download" }
    $candidate = $null
    if ($arch -eq "X64") {
        $binaryName = "eggsact-x86_64-pc-windows-msvc.exe"
        $binaryUrl = "$baseUrl/$releasePath/$binaryName"
        $binaryPath = Join-Path $temporary $binaryName
        $checksumPath = "$binaryPath.sha256"
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $binaryUrl -OutFile $binaryPath
        } catch {
            $status = if ($_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { 0 }
            if ($status -eq 404) {
                Write-Host "No published $binaryName asset was found; using Cargo fallback."
            } else {
                throw "Binary download failed (HTTP $status); checksum and transport failures are hard errors."
            }
        }
        if (Test-Path -LiteralPath $binaryPath) {
            try { Invoke-WebRequest -UseBasicParsing -Uri "$binaryUrl.sha256" -OutFile $checksumPath }
            catch { throw "The binary exists but its checksum sidecar could not be downloaded; refusing to install." }
            $expected = ((Get-Content -LiteralPath $checksumPath -Raw).Trim() -split '\s+')[0]
            if ($expected -notmatch '^[0-9a-fA-F]{64}$') { throw "Checksum sidecar does not contain a 64-hex SHA-256 digest." }
            $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $binaryPath).Hash
            if ($actual -ne $expected.ToUpperInvariant()) { throw "Checksum mismatch; refusing to install." }
            $candidate = $binaryPath
        }
    } else {
        Write-Host "No prebuilt Windows asset exists for detected architecture $arch; using Cargo fallback."
    }
    if (-not $candidate) {
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
            throw "No published Windows binary is available for detected architecture $arch, and Cargo is not installed. Install Rust from https://rustup.rs/."
        }
        $cargoRoot = Join-Path $temporary "cargo-root"
        $cargoArgs = @("install", "eggsact", "--locked", "--root", $cargoRoot)
        if ($Version) { $cargoArgs += @("--version", "=$Version") }
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) { throw "cargo install exited with code $LASTEXITCODE" }
        $candidate = Join-Path $cargoRoot "bin\eggsact.exe"
        if (-not (Test-Path -LiteralPath $candidate)) { throw "Cargo did not produce $candidate" }
    }
    $reported = ((& $candidate --version 2>&1) -join "`n").Trim()
    if ($Version) {
        if ($reported -ne "eggsact $Version") { throw "Candidate reported '$reported', expected eggsact $Version." }
    } elseif ($reported -notmatch '^eggsact [0-9]+\.[0-9]+\.[0-9]+$') { throw "Candidate reported unexpected version '$reported'." }

    $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    $destinationDir = if ($isAdmin) { Join-Path $env:ProgramFiles "Eggsact" } else { Join-Path $env:LOCALAPPDATA "Eggsact" }
    New-Item -ItemType Directory -Force -Path $destinationDir | Out-Null
    $destination = Join-Path $destinationDir "eggsact.exe"
    Copy-Item -LiteralPath $candidate -Destination $destination -Force
    Write-Host "Installed $destination"
    $pathValue = if ($isAdmin) { [Environment]::GetEnvironmentVariable("Path", "Machine") } else { [Environment]::GetEnvironmentVariable("Path", "User") }
    $normalizedDestination = ($destinationDir -replace '[\\/]+$','').ToLowerInvariant()
    $pathEntries = @($pathValue -split ';' | ForEach-Object { ($_ -replace '[\\/]+$','').ToLowerInvariant() })
    if ($normalizedDestination -notin $pathEntries) { Write-Host "Add $destinationDir to your PATH to run eggsact directly." }
} finally {
    Remove-Item -LiteralPath $temporary -Recurse -Force -ErrorAction SilentlyContinue
}
