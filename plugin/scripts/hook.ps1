# Nudge hook shim for Windows PowerShell 5.1 (D4, D15, D31). The PowerShell
# twin of hook.sh. Codex runs hook commands through powershell.exe; its
# SessionStart `commandWindows` (hooks/codex-hooks.json) runs this script with
# the hook JSON on stdin. Every other Codex event calls nudge-hook.exe inline,
# so a tool call never pays for a second PowerShell start (D31).
#
#   powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File hook.ps1 <Event> [agent]
#
# Contract: never write to stdout (SessionStart stdout is injected into the
# agent's context) and always exit 0 (AC #10). Every error is swallowed.
#
# Stdin is forwarded as raw bytes through a redirected child stdin, never
# through a PowerShell pipeline: PowerShell 5.1 re-encodes piped strings with
# $OutputEncoding (ASCII by default), which would turn every non-ASCII
# character of a `cwd` into `?`. (This file stays ASCII: PowerShell 5.1 reads
# BOM-less scripts in the ANSI code page.)

param([string]$Event, [string]$Agent = 'codex')

$ErrorActionPreference = 'SilentlyContinue'
$ProgressPreference = 'SilentlyContinue'

try {
    $nudgeHome = $env:NUDGE_HOME
    if (-not $nudgeHome) { $nudgeHome = Join-Path $HOME '.nudge' }
    $bin = Join-Path $nudgeHome 'bin'
    $hook = Join-Path $bin 'nudge-hook.exe'
    $haveHook = Test-Path -LiteralPath $hook -PathType Leaf

    # Read all of stdin as bytes (also drains it when nothing is installed).
    $payload = New-Object byte[] 0
    if ([Console]::IsInputRedirected) {
        $in = [Console]::OpenStandardInput()
        $buf = New-Object System.IO.MemoryStream
        $in.CopyTo($buf)
        $payload = $buf.ToArray()
    }

    if ($Event -eq 'SessionStart') {
        # The app is single-instance, so launching it on every SessionStart is
        # safe. Start-Process goes through ShellExecute: the app inherits no
        # handles, so the agent never waits on it, and a GUI exe opens no console.
        $app = Join-Path $bin 'nudge.exe'
        if (Test-Path -LiteralPath $app -PathType Leaf) {
            Start-Process -FilePath $app -WorkingDirectory $bin
        }
        if (-not $haveHook) {
            # First run: install in the background, hidden; this event is dropped.
            $installer = Join-Path $PSScriptRoot 'install.ps1'
            Start-Process -FilePath 'powershell.exe' -WindowStyle Hidden -ArgumentList @(
                '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
                '-File', "`"$installer`"", '-Agent', $Agent)
        }
    }

    if ($haveHook) {
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $hook
        if ($Agent) { $psi.Arguments = "--agent $Agent" }
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $psi.RedirectStandardInput = $true
        # nudge-hook never prints (D15); capture anyway so nothing can reach
        # the agent's stdout.
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        # Process.Start wraps the child's stdin in a StreamWriter using
        # [Console]::InputEncoding and writes that encoding's preamble at once.
        # Under code page 65001 that is a UTF-8 BOM, which nudge-hook's JSON
        # parser rejects. A BOM-less UTF-8 encoding writes nothing.
        try { [Console]::InputEncoding = New-Object System.Text.UTF8Encoding $false } catch { }
        $p = [System.Diagnostics.Process]::Start($psi)
        $null = $p.StandardOutput.ReadToEndAsync()
        $null = $p.StandardError.ReadToEndAsync()
        $p.StandardInput.BaseStream.Write($payload, 0, $payload.Length)
        $p.StandardInput.BaseStream.Flush()
        $p.StandardInput.Close()
        # Stay under the 5 s hook timeout; nudge-hook has its own short timeouts.
        $null = $p.WaitForExit(4000)
    }
} catch {
    # Fail soft (AC #10).
}
exit 0
