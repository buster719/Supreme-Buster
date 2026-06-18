param(
    [string]$RelayUrl = "http://57.131.49.101:8788",
    [string]$TokenPath = "",
    [string]$Workspace = "",
    [switch]$Build,
    [switch]$Quiet
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Workspace)) {
    $Workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}
if ([string]::IsNullOrWhiteSpace($TokenPath)) {
    $TokenPath = Join-Path $Workspace "secrets\vps-relay-token.txt"
}
if (-not (Test-Path $TokenPath)) {
    throw "Relay token not found: $TokenPath"
}

$project = Join-Path $Workspace "buster-core"
$state = Join-Path $Workspace "state"
$audit = Join-Path $Workspace "audit"
New-Item -ItemType Directory -Force -Path $state | Out-Null
New-Item -ItemType Directory -Force -Path $audit | Out-Null

$logPath = Join-Path $audit "witness-relay-sync.log"
$lockPath = Join-Path $state "witness-relay-sync.lock"
$lockStream = $null
try {
    $lockStream = [System.IO.File]::Open($lockPath, [System.IO.FileMode]::OpenOrCreate, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
} catch {
    $message = "[relay] another sync epoch is already running"
    Add-Content -Path $logPath -Value "$(Get-Date -Format o) $message"
    if (-not $Quiet) { Write-Host $message }
    exit 0
}

function Write-RelayLog {
    param([Parameter(Mandatory = $true)][string]$Message)
    $line = "$(Get-Date -Format o) $Message"
    Add-Content -Path $logPath -Value $line
    if (-not $Quiet) {
        Write-Host $Message
    }
}

function Convert-ToWslPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    $full = [System.IO.Path]::GetFullPath($Path)
    if ($full -match '^([A-Za-z]):\\(.*)$') {
        $drive = $Matches[1].ToLowerInvariant()
        $rest = $Matches[2] -replace '\\', '/'
        return "/mnt/$drive/$rest"
    }
    throw "Unsupported path for WSL conversion: $full"
}

function Invoke-WslBuster {
    param([Parameter(Mandatory = $true)][string]$Command)
    $wslProject = Convert-ToWslPath $project
    $wrapped = "source /home/buster/.cargo/env 2>/dev/null || true; cd '$wslProject'; $Command"
    wsl -e bash -lc $wrapped
    if ($LASTEXITCODE -ne 0) {
        throw "WSL command failed: $Command"
    }
}

function Save-Json {
    param(
        [Parameter(Mandatory = $true)]$Value,
        [Parameter(Mandatory = $true)][string]$Path
    )
    $json = $Value | ConvertTo-Json -Depth 80
    [System.IO.File]::WriteAllText(
        [System.IO.Path]::GetFullPath($Path),
        $json + [Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
}

$token = (Get-Content -Raw -Path $TokenPath).Trim()
$headers = @{ "X-Buster-Relay-Token" = $token }
$relay = $RelayUrl.TrimEnd("/")

$localSnapshot = Join-Path $state "relay-local-snapshot.json"
$remoteSnapshot = Join-Path $state "relay-remote-snapshot.json"
$receiptFromRemote = Join-Path $state "relay-local-receipt-from-remote.json"
$receiptForRemote = Join-Path $state "relay-remote-receipt-from-local.json"

$localSnapshotWsl = Convert-ToWslPath $localSnapshot
$remoteSnapshotWsl = Convert-ToWslPath $remoteSnapshot
$receiptFromRemoteWsl = Convert-ToWslPath $receiptFromRemote
$receiptForRemoteWsl = Convert-ToWslPath $receiptForRemote

try {
    Write-RelayLog "[relay] epoch started relay=$relay"

    if ($Build) {
        Write-RelayLog "[relay] building local busterd"
        Invoke-WslBuster "cargo build -q -p buster_daemon --bin busterd"
    } else {
        Write-RelayLog "[relay] using existing local busterd"
    }

    Write-RelayLog "[relay] exporting local snapshot"
    Invoke-WslBuster "target/debug/busterd node snapshot --root .. --out '$localSnapshotWsl' >/tmp/buster-relay-local-snapshot.out"

    Write-RelayLog "[relay] pushing local snapshot to relay"
    $localSnapshotBody = Get-Content -Raw -Path $localSnapshot
    $remoteReceipt = Invoke-RestMethod `
        -Uri "$relay/relay/snapshots" `
        -Method Post `
        -Headers $headers `
        -ContentType "application/json; charset=utf-8" `
        -Body $localSnapshotBody `
        -TimeoutSec 45
    Save-Json -Value $remoteReceipt -Path $receiptFromRemote

    Write-RelayLog "[relay] pulling remote snapshot"
    $snapshot = Invoke-RestMethod `
        -Uri "$relay/relay/snapshot" `
        -Method Get `
        -Headers $headers `
        -TimeoutSec 45
    Save-Json -Value $snapshot -Path $remoteSnapshot

    Write-RelayLog "[relay] local witnesses remote snapshot"
    Invoke-WslBuster "target/debug/busterd node witness --root .. --snapshot '$remoteSnapshotWsl' > '$receiptForRemoteWsl'"

    Write-RelayLog "[relay] importing relay receipt locally"
    Invoke-WslBuster "target/debug/busterd node import-receipt --root .. --receipt '$receiptFromRemoteWsl'"

    Write-RelayLog "[relay] pushing local receipt to relay"
    $localReceiptBody = Get-Content -Raw -Path $receiptForRemote
    $importResult = Invoke-RestMethod `
        -Uri "$relay/relay/receipts" `
        -Method Post `
        -Headers $headers `
        -ContentType "application/json; charset=utf-8" `
        -Body $localReceiptBody `
        -TimeoutSec 45
    Write-RelayLog "[relay] relay import result=$($importResult.status)"

    Write-RelayLog "[relay] epoch complete"
} catch {
    Write-RelayLog "[relay] epoch failed: $($_.Exception.Message)"
    throw
} finally {
    if ($lockStream) {
        $lockStream.Dispose()
    }
}
