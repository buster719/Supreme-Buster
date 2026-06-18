param(
    [string]$Workspace = "",
    [string]$TaskName = "BusterWitnessSync",
    [string]$RelayUrl = "http://57.131.49.101:8788",
    [string]$TokenPath = "",
    [int]$EpochMinutes = 5
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Workspace)) {
    $Workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}
if ([string]::IsNullOrWhiteSpace($TokenPath)) {
    $TokenPath = Join-Path $Workspace "secrets\vps-relay-token.txt"
}

$syncScript = Join-Path $Workspace "scripts\sync-witness-relay.ps1"
$hiddenRunner = Join-Path $Workspace "scripts\run-hidden-powershell.vbs"
if (-not (Test-Path $syncScript)) {
    throw "witness sync script not found: $syncScript"
}
if (-not (Test-Path $hiddenRunner)) {
    throw "hidden runner not found: $hiddenRunner"
}
if (-not (Test-Path $TokenPath)) {
    throw "relay token not found: $TokenPath"
}
if ($EpochMinutes -lt 1) {
    throw "EpochMinutes must be >= 1"
}

$escapedScript = $syncScript.Replace('"', '\"')
$escapedWorkspace = $Workspace.Replace('"', '\"')
$escapedHiddenRunner = $hiddenRunner.Replace('"', '\"')
$escapedRelayUrl = $RelayUrl.Replace('"', '\"')
$escapedTokenPath = $TokenPath.Replace('"', '\"')

$argument = "`"$escapedHiddenRunner`" -NoProfile -ExecutionPolicy Bypass -File `"$escapedScript`" -Workspace `"$escapedWorkspace`" -RelayUrl `"$escapedRelayUrl`" -TokenPath `"$escapedTokenPath`" -Quiet"

$action = New-ScheduledTaskAction -Execute "wscript.exe" -Argument $argument
$trigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) `
    -RepetitionInterval (New-TimeSpan -Minutes $EpochMinutes) `
    -RepetitionDuration (New-TimeSpan -Days 3650)
$settings = New-ScheduledTaskSettingsSet `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries `
    -MultipleInstances IgnoreNew `
    -RestartCount 3 `
    -RestartInterval (New-TimeSpan -Minutes 1)

Register-ScheduledTask `
    -TaskName $TaskName `
    -Action $action `
    -Trigger $trigger `
    -Settings $settings `
    -Description "Run Buster witness relay epochs against the auxiliary VPS boot/relay node." `
    -Force | Out-Null

Write-Host "Installed scheduled task: $TaskName"
Write-Host "Epoch: every $EpochMinutes minute(s)"
Write-Host "Relay: $RelayUrl"
Write-Host "Run now with: Start-ScheduledTask -TaskName $TaskName"
