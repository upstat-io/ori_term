# oriterm shell integration for PowerShell.
# Emits OSC 133 prompt markers and OSC 7 CWD reporting.

# Guard: only run once.
if ($env:ORITERM_PS_INTEGRATION_LOADED) { return }
$env:ORITERM_PS_INTEGRATION_LOADED = "1"

# Save the original prompt function.
if (Test-Path Function:\prompt) {
    $script:OriginalPrompt = Get-Content Function:\prompt
    Rename-Item Function:\prompt _oriterm_original_prompt -ErrorAction SilentlyContinue
}

function prompt {
    $lastExit = $LASTEXITCODE
    $exitCode = if ($?) { "0" } else { if ($null -ne $lastExit) { "$lastExit" } else { "1" } }

    # D — end of previous command output
    [Console]::Write("`e]133;D;$exitCode`a")

    # OSC 7 — report CWD
    $cwd = (Get-Location).Path
    $hostname = [System.Net.Dns]::GetHostName()
    [Console]::Write("`e]7;file://$hostname$cwd`a")

    # A — prompt start
    [Console]::Write("`e]133;A`a")

    # Call original prompt or default
    if (Test-Path Function:\_oriterm_original_prompt) {
        $result = _oriterm_original_prompt
    } else {
        $result = "PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) "
    }

    # B — prompt end / input start
    [Console]::Write("`e]133;B`a")

    $global:LASTEXITCODE = $lastExit
    return $result
}

# C marker: Use PSReadLine handler to emit before command execution.
if (Get-Module PSReadLine -ErrorAction SilentlyContinue) {
    $script:OriginalAcceptLine = $null
    try {
        $script:OriginalAcceptLine = (Get-PSReadLineKeyHandler -Bound | Where-Object { $_.Key -eq "Enter" -and $_.Function -eq "AcceptLine" })
    } catch { }

    Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {
        [Console]::Write("`e]133;C`a")
        [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
    }
}
