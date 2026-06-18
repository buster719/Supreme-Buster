param(
    [string]$Workspace = "",
    [int]$WebPort = 8787,
    [int]$TickIntervalMs = 300000
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Workspace)) {
    $Workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

$project = Join-Path $Workspace "buster-core"

function Convert-ToWslPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    $full = (Resolve-Path $Path).Path
    if ($full -match '^([A-Za-z]):\\(.*)$') {
        $drive = $Matches[1].ToLowerInvariant()
        $rest = $Matches[2] -replace '\\', '/'
        return "/mnt/$drive/$rest"
    }
    throw "Unsupported path for WSL conversion: $full"
}

$wslProject = Convert-ToWslPath $project

function Start-BusterWslProcess {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [string]$BashCommand
    )

    $prefix = "source /home/buster/.cargo/env 2>/dev/null || true; export HTTP_PROXY=http://127.0.0.1:7897 HTTPS_PROXY=http://127.0.0.1:7897 http_proxy=http://127.0.0.1:7897 https_proxy=http://127.0.0.1:7897; cd '$wslProject'"
    $wrapped = "$prefix; $BashCommand"

    $escaped = $wrapped.Replace('"', '\"')
    Start-Process -FilePath "wsl.exe" -ArgumentList "bash -lc `"$escaped`"" -WindowStyle Hidden
    Write-Host "started $Name"
}

if (-not (Test-Path $project)) {
    throw "Buster project not found: $project"
}

Start-BusterWslProcess -Name "busterd-web" -BashCommand "if pgrep -f '[t]arget/debug/busterd web --root .. --port $WebPort' >/dev/null; then echo busterd-web already running >> /tmp/busterd-web-$WebPort.log; else cargo run -p buster_daemon --bin busterd -- web --root .. --port $WebPort --execute-actions --no-open >> /tmp/busterd-web-$WebPort.log 2>&1; fi"

Start-BusterWslProcess -Name "busterd-run" -BashCommand "if pgrep -f '[t]arget/debug/busterd run --root .. --forever' >/dev/null; then echo busterd-run already running >> /tmp/busterd-run.log; else cargo run -p buster_daemon --bin busterd -- run --root .. --forever --interval-ms $TickIntervalMs --execute-actions --no-body-reports >> /tmp/busterd-run.log 2>&1; fi"

Write-Host "Buster startup requested. Web console: http://127.0.0.1:$WebPort/"
