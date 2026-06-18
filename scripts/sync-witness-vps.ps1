param(
    [string]$RemoteHost = "ubuntu@57.131.49.101",
    [string]$KeyPath = "",
    [string]$Workspace = "",
    [string]$RemoteRoot = "/home/ubuntu/buster-aux-node",
    [switch]$Build,
    [switch]$Quiet
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Workspace)) {
    $Workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}
if ([string]::IsNullOrWhiteSpace($KeyPath)) {
    $KeyPath = Join-Path $HOME ".ssh\buster_aux_node_ed25519"
}
if (-not (Test-Path $KeyPath)) {
    throw "SSH key not found: $KeyPath"
}

$project = Join-Path $Workspace "buster-core"
$state = Join-Path $Workspace "state"
$audit = Join-Path $Workspace "audit"
New-Item -ItemType Directory -Force -Path $state | Out-Null
New-Item -ItemType Directory -Force -Path $audit | Out-Null

$logPath = Join-Path $audit "witness-sync.log"
$lockPath = Join-Path $state "witness-sync.lock"
$lockStream = $null
try {
    $lockStream = [System.IO.File]::Open($lockPath, [System.IO.FileMode]::OpenOrCreate, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
} catch {
    $message = "[witness] another sync epoch is already running"
    Add-Content -Path $logPath -Value "$(Get-Date -Format o) $message"
    if (-not $Quiet) { Write-Host $message }
    exit 0
}

function Write-WitnessLog {
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

$localSnapshot = Join-Path $state "witness-local-snapshot.json"
$remoteSnapshot = Join-Path $state "witness-remote-snapshot.json"
$receiptFromRemote = Join-Path $state "witness-local-receipt-from-remote.json"
$receiptForRemote = Join-Path $state "witness-remote-receipt-from-local.json"

$localSnapshotWsl = Convert-ToWslPath $localSnapshot
$remoteSnapshotWsl = Convert-ToWslPath $remoteSnapshot
$receiptFromRemoteWsl = Convert-ToWslPath $receiptFromRemote
$receiptForRemoteWsl = Convert-ToWslPath $receiptForRemote

try {
Write-WitnessLog "[witness] epoch started remote=$RemoteHost"

if ($Build) {
    Write-WitnessLog "[witness] building local busterd"
    Invoke-WslBuster "cargo build -q -p buster_daemon --bin busterd"
} else {
    Write-WitnessLog "[witness] using existing local busterd"
}

Write-WitnessLog "[witness] exporting local snapshot"
Invoke-WslBuster "target/debug/busterd node snapshot --root .. --out '$localSnapshotWsl' >/tmp/buster-local-snapshot.out"

Write-WitnessLog "[witness] sending local snapshot to remote"
scp.exe -i $KeyPath -o StrictHostKeyChecking=accept-new $localSnapshot "${RemoteHost}:/tmp/buster-local-snapshot.json"
if ($LASTEXITCODE -ne 0) { throw "scp local snapshot failed" }

Write-WitnessLog "[witness] remote witnesses local"
ssh.exe -i $KeyPath -o StrictHostKeyChecking=accept-new $RemoteHost "cd '$RemoteRoot/buster-core' && target/debug/busterd node witness --root '$RemoteRoot' --snapshot /tmp/buster-local-snapshot.json > /tmp/buster-local-receipt-from-remote.json"
if ($LASTEXITCODE -ne 0) { throw "remote witness failed" }
scp.exe -i $KeyPath -o StrictHostKeyChecking=accept-new "${RemoteHost}:/tmp/buster-local-receipt-from-remote.json" $receiptFromRemote
if ($LASTEXITCODE -ne 0) { throw "scp remote receipt failed" }

Write-WitnessLog "[witness] exporting remote snapshot"
ssh.exe -i $KeyPath -o StrictHostKeyChecking=accept-new $RemoteHost "cd '$RemoteRoot/buster-core' && target/debug/busterd node snapshot --root '$RemoteRoot' --out /tmp/buster-remote-snapshot.json >/tmp/buster-remote-snapshot.out"
if ($LASTEXITCODE -ne 0) { throw "remote snapshot failed" }
scp.exe -i $KeyPath -o StrictHostKeyChecking=accept-new "${RemoteHost}:/tmp/buster-remote-snapshot.json" $remoteSnapshot
if ($LASTEXITCODE -ne 0) { throw "scp remote snapshot failed" }

Write-WitnessLog "[witness] local witnesses remote"
Invoke-WslBuster "target/debug/busterd node witness --root .. --snapshot '$remoteSnapshotWsl' > '$receiptForRemoteWsl'"

Write-WitnessLog "[witness] importing remote receipt locally"
Invoke-WslBuster "target/debug/busterd node import-receipt --root .. --receipt '$receiptFromRemoteWsl'"

Write-WitnessLog "[witness] sending local receipt to remote"
scp.exe -i $KeyPath -o StrictHostKeyChecking=accept-new $receiptForRemote "${RemoteHost}:/tmp/buster-remote-receipt-from-local.json"
if ($LASTEXITCODE -ne 0) { throw "scp local receipt failed" }
ssh.exe -i $KeyPath -o StrictHostKeyChecking=accept-new $RemoteHost "cd '$RemoteRoot/buster-core' && target/debug/busterd node import-receipt --root '$RemoteRoot' --receipt /tmp/buster-remote-receipt-from-local.json"
if ($LASTEXITCODE -ne 0) { throw "remote import receipt failed" }

Write-WitnessLog "[witness] local registry"
Invoke-WslBuster "target/debug/busterd node registry --root .."

Write-WitnessLog "[witness] remote registry"
ssh.exe -i $KeyPath -o StrictHostKeyChecking=accept-new $RemoteHost "cd '$RemoteRoot/buster-core' && target/debug/busterd node registry --root '$RemoteRoot'"
if ($LASTEXITCODE -ne 0) { throw "remote registry failed" }

Write-WitnessLog "[witness] epoch complete"
} catch {
    Write-WitnessLog "[witness] epoch failed: $($_.Exception.Message)"
    throw
} finally {
    if ($lockStream) {
        $lockStream.Dispose()
    }
}
