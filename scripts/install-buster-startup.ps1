param(
    [string]$Workspace = "",
    [string]$TaskName = "BusterDaemon",
    [int]$WebPort = 8787,
    [int]$TickIntervalMs = 300000
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Workspace)) {
    $Workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

$startScript = Join-Path $Workspace "scripts\start-buster.ps1"
if (-not (Test-Path $startScript)) {
    throw "start script not found: $startScript"
}

$escapedScript = $startScript.Replace('"', '\"')
$escapedWorkspace = $Workspace.Replace('"', '\"')
$argument = "-NoProfile -ExecutionPolicy Bypass -File `"$escapedScript`" -Workspace `"$escapedWorkspace`" -WebPort $WebPort -TickIntervalMs $TickIntervalMs"

$action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument $argument
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
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
    -Description "Start Buster web console and forever runtime after Windows logon." `
    -Force | Out-Null

Write-Host "Installed scheduled task: $TaskName"
Write-Host "Run now with: Start-ScheduledTask -TaskName $TaskName"
