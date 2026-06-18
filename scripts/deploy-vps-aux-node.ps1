param(
    [string]$RemoteHost = "ubuntu@57.131.49.101",
    [string]$KeyPath = "",
    [string]$Workspace = "",
    [string]$RemoteRoot = "/home/ubuntu/buster-aux-node",
    [int]$HeartbeatMinutes = 5,
    [int]$DaemonIntervalMs = 300000,
    [bool]$InstallDaemon = $true
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

$archive = Join-Path $env:TEMP "buster-aux-node.tgz"
if (Test-Path $archive) {
    Remove-Item -LiteralPath $archive -Force
}

$items = @(
    "buster-core",
    "self.md",
    "GOVERNANCE.md",
    "BODY.md",
    "BUSTER_PROFILE.md",
    "VALUE_MODEL.md",
    "RUNTIME_CYCLE.md",
    "CYBER_ORGANISM.md",
    "CYBER_ORGANISM_ROADMAP.md",
    "SECURITY_REVIEW.md",
    "scripts/vps-demo-server.py"
) | Where-Object { Test-Path (Join-Path $Workspace $_) }

if (-not $items) {
    throw "No deployable Buster files found in $Workspace"
}

& tar.exe `
    -czf $archive `
    --exclude "*/target" `
    --exclude "secrets" `
    --exclude "state" `
    --exclude "audit" `
    --exclude "inbox" `
    --exclude "research" `
    -C $Workspace `
    @items

& scp.exe -i $KeyPath -o StrictHostKeyChecking=accept-new $archive "${RemoteHost}:/tmp/buster-aux-node.tgz"

$remoteScript = @"
set -euo pipefail
REMOTE_ROOT='$RemoteRoot'
HEARTBEAT_MINUTES='$HeartbeatMinutes'
DAEMON_INTERVAL_MS='$DaemonIntervalMs'
INSTALL_DAEMON='$InstallDaemon'

echo '[buster-aux] installing host dependencies'
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev ca-certificates curl git

if ! command -v cargo >/dev/null 2>&1; then
  echo '[buster-aux] installing rust toolchain'
  curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
fi

. "`$HOME/.cargo/env"

systemctl --user stop buster-daemon.service buster-aux-heartbeat.timer 2>/dev/null || true

echo '[buster-aux] unpacking Buster snapshot'
PRESERVE_DIR="`$(mktemp -d)"
if [ -d "`$REMOTE_ROOT" ]; then
  for name in .env secrets state audit research inbox; do
    if [ -e "`$REMOTE_ROOT/`$name" ]; then
      cp -a "`$REMOTE_ROOT/`$name" "`$PRESERVE_DIR/"
    fi
  done
fi
rm -rf "`$REMOTE_ROOT"
mkdir -p "`$REMOTE_ROOT"
tar -xzf /tmp/buster-aux-node.tgz -C "`$REMOTE_ROOT"
for name in .env secrets state audit research inbox; do
  if [ -e "`$PRESERVE_DIR/`$name" ]; then
    rm -rf "`$REMOTE_ROOT/`$name"
    cp -a "`$PRESERVE_DIR/`$name" "`$REMOTE_ROOT/"
  fi
done
if [ -f "`$REMOTE_ROOT/scripts/vps-demo-server.py" ]; then
  cp "`$REMOTE_ROOT/scripts/vps-demo-server.py" "`$REMOTE_ROOT/demo_server.py"
fi
rm -rf "`$PRESERVE_DIR"

cd "`$REMOTE_ROOT/buster-core"
echo '[buster-aux] building busterd'
cargo build -p buster_daemon --bin busterd

echo '[buster-aux] initializing independent node identity'
target/debug/busterd node init --root ..
python3 - <<'PY'
import json
from pathlib import Path
path = Path("../state/node-identity.json")
if path.exists():
    data = json.loads(path.read_text())
    data["role"] = "remote-auxiliary-witness"
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
PY
target/debug/busterd node heartbeat --root ..

mkdir -p "`$HOME/.config/systemd/user"
cat > "`$HOME/.config/systemd/user/buster-aux-heartbeat.service" <<'SERVICE'
[Unit]
Description=Buster auxiliary node heartbeat

[Service]
Type=oneshot
WorkingDirectory=%h/buster-aux-node/buster-core
ExecStart=%h/buster-aux-node/buster-core/target/debug/busterd node heartbeat --root ..
SERVICE

cat > "`$HOME/.config/systemd/user/buster-aux-heartbeat.timer" <<TIMER
[Unit]
Description=Run Buster auxiliary heartbeat every $HeartbeatMinutes minutes

[Timer]
OnBootSec=60
OnUnitActiveSec=$HeartbeatMinutes min
Persistent=true

[Install]
WantedBy=timers.target
TIMER

if [ "`$INSTALL_DAEMON" = "True" ] || [ "`$INSTALL_DAEMON" = "true" ] || [ "`$INSTALL_DAEMON" = "1" ]; then
cat > "`$HOME/.config/systemd/user/buster-daemon.service" <<SERVICE
[Unit]
Description=Buster persistent daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=$RemoteRoot/buster-core
ExecStart=$RemoteRoot/buster-core/target/debug/busterd run --root .. --forever --interval-ms $DaemonIntervalMs --execute-actions --no-body-reports
Restart=always
RestartSec=10
KillMode=control-group

[Install]
WantedBy=default.target
SERVICE
fi

systemctl --user daemon-reload
systemctl --user enable --now buster-aux-heartbeat.timer
if [ "`$INSTALL_DAEMON" = "True" ] || [ "`$INSTALL_DAEMON" = "true" ] || [ "`$INSTALL_DAEMON" = "1" ]; then
  systemctl --user enable --now buster-daemon.service
fi
sudo loginctl enable-linger "`$(whoami)" || true

echo '[buster-aux] status'
systemctl --user --no-pager status buster-aux-heartbeat.timer || true
if [ "`$INSTALL_DAEMON" = "True" ] || [ "`$INSTALL_DAEMON" = "true" ] || [ "`$INSTALL_DAEMON" = "1" ]; then
  systemctl --user --no-pager status buster-daemon.service || true
fi
cd "`$REMOTE_ROOT/buster-core"
target/debug/busterd node verify --root ..
"@

$remoteScript = $remoteScript -replace "`r`n", "`n"
$remoteScript | & ssh.exe -i $KeyPath -o StrictHostKeyChecking=accept-new $RemoteHost "bash -s"

Write-Host "Buster auxiliary node deploy requested on $RemoteHost at $RemoteRoot"
