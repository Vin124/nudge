# Nudge installer for Windows PowerShell 5.1 (D4, D31, spec: Components 1,
# Security). The PowerShell port of install.sh for windows-x86_64, started
# hidden and detached by hook.ps1 on the first Codex SessionStart. Downloads
# the release, verifies its SHA-256 against the release's SHA256SUMS, extracts
# it into ~/.nudge/bin, runs `nudge-status setup` (D14) unless installing for
# Codex, and launches the app.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1 [-Agent claude|codex]
#
# D31: a Codex install must not edit Claude's settings, so -Agent codex skips
# `nudge-status setup` (the Claude statusline); /nudge:setup can run it later.
#
# Environment: NUDGE_HOME (default ~/.nudge), NUDGE_REPO (default
# Vin124/nudge), NUDGE_BASE_URL (overrides the whole download base, e.g. a
# file:/// directory in tests).
#
# Logs to ~/.nudge/install.log. Never writes stdout. On any failure: log and
# exit 0.

param([string]$Agent = 'claude')

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$nudgeHome = $env:NUDGE_HOME
if (-not $nudgeHome) { $nudgeHome = Join-Path $HOME '.nudge' }
$bin = Join-Path $nudgeHome 'bin'
$log = Join-Path $nudgeHome 'install.log'
$lock = Join-Path $nudgeHome 'install.lock'
$repo = $env:NUDGE_REPO
if (-not $repo) { $repo = 'Vin124/nudge' }
$baseUrl = $env:NUDGE_BASE_URL
if (-not $baseUrl) { $baseUrl = "https://github.com/$repo/releases/latest/download" }
$asset = 'nudge-windows-x86_64.zip'

$script:locked = $false
$script:tmp = $null

try { $null = New-Item -ItemType Directory -Force -Path $nudgeHome } catch { exit 0 }

function Write-Log([string]$msg) {
    try {
        $ts = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
        # UTF-8 without BOM, appended, like install.sh's log.
        [System.IO.File]::AppendAllText($log, "$ts $msg`n")
    } catch { }
}

function Invoke-Install {
    # ---- single installer at a time (shares install.sh's lock dir) ----------
    try {
        $null = New-Item -ItemType Directory -Path $lock
    } catch {
        # A lock older than 10 minutes belongs to a crashed install.
        $item = Get-Item -LiteralPath $lock -ErrorAction SilentlyContinue
        if ($item -and $item.LastWriteTimeUtc -lt [DateTime]::UtcNow.AddMinutes(-10)) {
            Remove-Item -LiteralPath $lock -Recurse -Force
            $null = New-Item -ItemType Directory -Path $lock
        } else {
            Write-Log 'another install is running; skipping'
            return
        }
    }
    $script:locked = $true

    # Another session's install may have finished while we waited.
    if (Test-Path -LiteralPath (Join-Path $bin 'nudge-hook.exe') -PathType Leaf) {
        Write-Log 'already installed; skipping'
        return
    }

    # ---- platform -----------------------------------------------------------
    if (-not [Environment]::Is64BitOperatingSystem) { throw 'no release for windows-x86' }
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    if ($arch -ne 'AMD64') { Write-Log "WARN: $arch machine; installing the x86_64 build" }
    Write-Log "installing $asset from $baseUrl (agent: $Agent)"

    # ---- download -----------------------------------------------------------
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $script:tmp = Join-Path ([System.IO.Path]::GetTempPath()) ('nudge-install-' + [guid]::NewGuid().ToString('N'))
    $null = New-Item -ItemType Directory -Path $script:tmp
    $zip = Join-Path $script:tmp $asset
    $sums = Join-Path $script:tmp 'SHA256SUMS'
    $wc = New-Object System.Net.WebClient
    try {
        try { $wc.DownloadFile("$baseUrl/$asset", $zip) } catch { throw "download of $asset failed: $($_.Exception.Message)" }
        try { $wc.DownloadFile("$baseUrl/SHA256SUMS", $sums) } catch { throw "download of SHA256SUMS failed: $($_.Exception.Message)" }
    } finally {
        $wc.Dispose()
    }

    # ---- verify -------------------------------------------------------------
    $expected = $null
    foreach ($line in [System.IO.File]::ReadAllLines($sums)) {
        $parts = $line.Trim() -split '\s+', 2
        if ($parts.Count -eq 2 -and $parts[1].TrimStart('*') -eq $asset) {
            $expected = $parts[0].ToLowerInvariant()
            break
        }
    }
    if (-not $expected) { throw "$asset not listed in SHA256SUMS" }
    $actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($expected -ne $actual) { throw "checksum mismatch for $asset (expected $expected, got $actual)" }
    Write-Log 'checksum ok'

    # ---- extract ------------------------------------------------------------
    $stage = Join-Path $script:tmp 'stage'
    try { Expand-Archive -LiteralPath $zip -DestinationPath $stage -Force } catch { throw "Expand-Archive failed: $($_.Exception.Message)" }
    $null = New-Item -ItemType Directory -Force -Path $bin
    try { Copy-Item -Path (Join-Path $stage '*') -Destination $bin -Recurse -Force } catch { throw "copy into $bin failed: $($_.Exception.Message)" }
    $status = Join-Path $bin 'nudge-status.exe'
    if (-not (Test-Path -LiteralPath $status -PathType Leaf)) { throw "nudge-status missing from $asset" }
    Write-Log "extracted to $bin"

    # ---- statusline (D14), Claude only (D31) --------------------------------
    if ($Agent -eq 'codex') {
        Write-Log 'agent codex: skipping nudge-status setup (Claude statusline untouched)'
    } else {
        # 'Continue' so native stderr lines are logged instead of thrown.
        $ErrorActionPreference = 'Continue'
        $out = & $status setup 2>&1 | Out-String
        $code = $LASTEXITCODE
        $ErrorActionPreference = 'Stop'
        if ($out) { Write-Log $out.TrimEnd() }
        if ($code -ne 0) { Write-Log 'WARN: nudge-status setup failed (run /nudge:setup)' }
    }

    # ---- launch (detached; the app is single-instance) ----------------------
    $app = Join-Path $bin 'nudge.exe'
    if (Test-Path -LiteralPath $app -PathType Leaf) {
        Start-Process -FilePath $app -WorkingDirectory $bin
    }
    Write-Log 'install complete'
}

try {
    Invoke-Install
} catch {
    Write-Log "ERROR: $($_.Exception.Message)"
} finally {
    if ($script:tmp) { Remove-Item -LiteralPath $script:tmp -Recurse -Force -ErrorAction SilentlyContinue }
    if ($script:locked) { Remove-Item -LiteralPath $lock -Recurse -Force -ErrorAction SilentlyContinue }
}
exit 0
