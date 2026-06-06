#!/bin/bash
# ============================================================================
# OpenClaw Nightly Security Audit v2.8
# 13-item full-coverage audit with explicit reporting
# Aligned with: OpenClaw Security Practice Guide v2.8
# Note: Status messages are in Chinese (中文). Modify ok/warn/crit calls
#       if you prefer English output.
# ============================================================================
set -uo pipefail
# NOTE: removed `set -e` — individual command failures are handled per-section
#       to prevent one check from killing the entire audit

OC="${OPENCLAW_STATE_DIR:-$HOME/.openclaw}"
REPORT_DIR="${OC}/security-reports"
DATE=$(date +%Y-%m-%d)
REPORT_FILE="$REPORT_DIR/report-${DATE}.txt"
SUMMARY=""
WARN_COUNT=0
CRIT_COUNT=0
OK_COUNT=0

KNOWN_ISSUES_FILE="${OC}/.security-audit-known-issues.json"

mkdir -p "$REPORT_DIR"

log() { echo "$1" | tee -a "$REPORT_FILE"; }

# Check if a string matches any known issue pattern for a given check
# Usage: is_known_issue "check_name" "text_to_match"
# Returns 0 (true) if matched, 1 (false) if not
is_known_issue() {
  local check="$1"
  local text="$2"
  if [ ! -f "$KNOWN_ISSUES_FILE" ]; then
    return 1
  fi
  # Extract patterns for this check and test against text
  local patterns
  patterns=$(python3 -c "
import json,sys
try:
    issues = json.load(open('$KNOWN_ISSUES_FILE'))
    for i in issues:
        if i.get('check') == '$check':
            print(i['pattern'] + '|||' + i.get('reason',''))
except: pass
" 2>/dev/null || true)
  while IFS= read -r entry; do
    [ -z "$entry" ] && continue
    local pat="${entry%%|||*}"
    if echo "$text" | grep -qiE "$pat"; then
      return 0
    fi
  done <<< "$patterns"
  return 1
}

# Get the reason for a known issue match
get_known_reason() {
  local check="$1"
  local text="$2"
  if [ ! -f "$KNOWN_ISSUES_FILE" ]; then
    echo ""
    return
  fi
  python3 -c "
import json,re,sys
try:
    issues = json.load(open('$KNOWN_ISSUES_FILE'))
    for i in issues:
        if i.get('check') == '$check':
            if re.search(i['pattern'], '$text', re.IGNORECASE):
                print(i.get('reason','已知问题'))
                sys.exit(0)
except: pass
print('')
" 2>/dev/null || echo ""
}
section() { log ""; log "=== [$1] $2 ==="; }
ok()   { SUMMARY="${SUMMARY}\n$1. ✅ $2"; OK_COUNT=$((OK_COUNT+1)); }
warn() { SUMMARY="${SUMMARY}\n$1. ⚠️ $2"; WARN_COUNT=$((WARN_COUNT+1)); }
crit() { SUMMARY="${SUMMARY}\n$1. 🚨 $2"; CRIT_COUNT=$((CRIT_COUNT+1)); }

echo "# OpenClaw Security Audit Report - $(date)" > "$REPORT_FILE"
log "# Timezone: $(date +%Z) | Host: $(hostname)"

# ── 1. OpenClaw Platform Audit ──────────────────────────────────────────────
section 1 "OpenClaw Platform Audit"
OC_AUDIT=$(openclaw security audit --deep 2>&1 || true)

# Annotate known issues in the raw output before logging,
# so the LLM summarizer sees them as acknowledged/ignored
OC_AUDIT_DISPLAY="$OC_AUDIT"
if [ -f "$KNOWN_ISSUES_FILE" ]; then
  KNOWN_PATTERNS=$(python3 -c "
import json
try:
    issues = json.load(open('$KNOWN_ISSUES_FILE'))
    for i in issues:
        if i.get('check') == 'platform_audit':
            print(i['pattern'] + '|||' + i.get('reason','已知问题'))
except: pass
" 2>/dev/null || true)
  while IFS= read -r kp_entry; do
    [ -z "$kp_entry" ] && continue
    kp_pat="${kp_entry%%|||*}"
    kp_reason="${kp_entry#*|||}"
    # For each line matching the pattern, prepend [已知问题-忽略]
    OC_AUDIT_DISPLAY=$(echo "$OC_AUDIT_DISPLAY" | sed -E "/${kp_pat}/I s/^/[已知问题-忽略: ${kp_reason}] /")
  done <<< "$KNOWN_PATTERNS"
fi
log "$OC_AUDIT_DISPLAY"
OC_CRIT_TOTAL=$(echo "$OC_AUDIT" | grep -c "CRITICAL" || true)
OC_WARN=$(echo "$OC_AUDIT" | grep -c "WARN" || true)

# Filter out known issues from CRITICAL count
# openclaw security audit outputs "CRITICAL" on its own line, followed by detail lines.
# We extract blocks: from each detail line (non-empty, starts with a letter) until next section.
OC_CRIT_NEW=0
OC_CRIT_KNOWN=0
if [ "$OC_CRIT_TOTAL" -gt 0 ]; then
  # Grab the CRITICAL section and its detail lines (up to WARN or INFO or end)
  CRIT_BLOCK=$(echo "$OC_AUDIT" | sed -n '/^CRITICAL$/,/^\(WARN\|INFO\|$\)/p' | grep -v "^CRITICAL$" | grep -v "^WARN$" | grep -v "^INFO$" || true)
  # Each finding starts with a non-space identifier like "models.small_params" or "skills.code_safety"
  # Collect full text per finding and match against known issues
  CURRENT_FINDING=""
  while IFS= read -r line; do
    if echo "$line" | grep -qE '^[a-z]'; then
      # New finding starts — process previous if exists
      if [ -n "$CURRENT_FINDING" ]; then
        if is_known_issue "platform_audit" "$CURRENT_FINDING"; then
          OC_CRIT_KNOWN=$((OC_CRIT_KNOWN+1))
        else
          OC_CRIT_NEW=$((OC_CRIT_NEW+1))
        fi
      fi
      CURRENT_FINDING="$line"
    else
      CURRENT_FINDING="${CURRENT_FINDING} ${line}"
    fi
  done <<< "$CRIT_BLOCK"
  # Process last finding
  if [ -n "$CURRENT_FINDING" ]; then
    if is_known_issue "platform_audit" "$CURRENT_FINDING"; then
      OC_CRIT_KNOWN=$((OC_CRIT_KNOWN+1))
    else
      OC_CRIT_NEW=$((OC_CRIT_NEW+1))
    fi
  fi
fi

KNOWN_NOTE=""
[ "$OC_CRIT_KNOWN" -gt 0 ] && KNOWN_NOTE=" (${OC_CRIT_KNOWN} 个已知问题已忽略)"

if [ "$OC_CRIT_NEW" -gt 0 ]; then
  crit 1 "平台审计: ${OC_CRIT_NEW} 个新 Critical / ${OC_WARN} Warn${KNOWN_NOTE}"
elif [ "$OC_WARN" -gt 0 ]; then
  warn 1 "平台审计: ${OC_WARN} Warn${KNOWN_NOTE}"
else
  ok 1 "平台审计通过${KNOWN_NOTE}"
fi

# ── 2. Process & Network Audit ──────────────────────────────────────────────
section 2 "Process & Network Audit"
log "--- Listening Ports (TCP + UDP) ---"
LISTEN_OUT=$(sudo ss -tulpn 2>/dev/null || ss -tulpn 2>/dev/null || echo "ss unavailable")
log "$LISTEN_OUT"
LISTEN_COUNT=$(echo "$LISTEN_OUT" | grep -c "LISTEN" || true)

log "--- Top 15 by CPU/MEM ---"
ps aux --sort=-%cpu,-%mem | head -n 16 | tee -a "$REPORT_FILE"

log "--- Outbound Established Connections ---"
OUTBOUND=$(ss -tnp state established 2>/dev/null | grep -v "127.0.0.1" | grep -v "::1" || true)
log "$OUTBOUND"
OUTBOUND_COUNT=$(echo "$OUTBOUND" | grep -c "." || true)

if [ "$OUTBOUND_COUNT" -gt 20 ]; then
  warn 2 "进程网络: ${LISTEN_COUNT} 监听, ${OUTBOUND_COUNT} 出站连接(偏多)"
else
  ok 2 "进程网络: ${LISTEN_COUNT} 监听, ${OUTBOUND_COUNT} 出站连接"
fi

# ── 3. Sensitive Directory Changes (24h) ────────────────────────────────────
section 3 "Sensitive Directory Changes (24h)"
CHANGES=$(find "$OC/" /etc/ ~/.ssh/ ~/.gnupg/ /usr/local/bin/ \
  -maxdepth 3 -mtime -1 \
  -not -path '*/.git/*' -not -path '*/tmp/*' -not -name '*.tmp' \
  2>/dev/null | head -n 50 || true)
CHANGE_COUNT=$(echo "$CHANGES" | grep -c "." || true)
log "$CHANGES"
if [ "$CHANGE_COUNT" -gt 30 ]; then
  warn 3 "目录变更: ${CHANGE_COUNT} 个文件(24h内)"
else
  ok 3 "目录变更: ${CHANGE_COUNT} 个文件(24h内)"
fi

# ── 4. System Cron & Timers ─────────────────────────────────────────────────
section 4 "System Cron & Timers"
log "--- systemd timers ---"
systemctl list-timers --all --no-pager 2>/dev/null | head -n 20 | tee -a "$REPORT_FILE"
log "--- /etc/cron.d/ ---"
ls -la /etc/cron.d/ 2>/dev/null | tee -a "$REPORT_FILE"
log "--- user crontab ---"
crontab -l 2>/dev/null | tee -a "$REPORT_FILE" || log "(no user crontab)"
log "--- user systemd units ---"
ls ~/.config/systemd/user/*.service 2>/dev/null | tee -a "$REPORT_FILE" || log "(no user units)"
ok 4 "系统定时任务: 已列出(请对比预期)"

# ── 5. OpenClaw Cron Jobs ───────────────────────────────────────────────────
section 5 "OpenClaw Cron Jobs"
OC_CRON=$(openclaw cron list 2>&1 || echo "cron list failed")
log "$OC_CRON"
ok 5 "OpenClaw Cron: 已列出(请对比预期)"

# ── 6. Login & SSH ──────────────────────────────────────────────────────────
section 6 "Login & SSH Security"
log "--- Current sessions ---"
who 2>/dev/null | tee -a "$REPORT_FILE" || log "(who unavailable)"
log "--- Recent logins ---"
lastlog 2>/dev/null | grep -v "Never logged in" | tail -n 10 | tee -a "$REPORT_FILE" || log "(lastlog unavailable)"
log "--- SSH failures (24h) ---"
SSH_FAILS=$(sudo journalctl -u ssh -u sshd --since "24h ago" 2>/dev/null | grep -ci "failed\|invalid\|refused" || true)
log "SSH failure count: $SSH_FAILS"
if [ "$SSH_FAILS" -gt 50 ]; then
  warn 6 "SSH 安全: ${SSH_FAILS} 次失败尝试(24h)"
else
  ok 6 "SSH 安全: ${SSH_FAILS} 次失败尝试(24h)"
fi

# ── 7. File Integrity (Hash + Permissions) ──────────────────────────────────
section 7 "File Integrity Check"
log "--- Permissions ---"
ls -la "$OC/openclaw.json" "$OC/devices/paired.json" 2>/dev/null | tee -a "$REPORT_FILE"
ls -la /etc/ssh/sshd_config ~/.ssh/authorized_keys 2>/dev/null | tee -a "$REPORT_FILE" || true
lsattr "$OC/openclaw.json" "$OC/workspace/scripts/nightly-security-audit.sh" 2>/dev/null | tee -a "$REPORT_FILE" || true

log "--- Hash Baseline Check ---"
BASELINE_FILE="$OC/.config-baseline.sha256"
if [ -f "$BASELINE_FILE" ]; then
  # NOTE: sha256sum -c returns non-zero on mismatch; capture without letting it kill the script
  HASH_RESULT=$(sha256sum -c "$BASELINE_FILE" 2>&1 || true)
  log "$HASH_RESULT"
  HASH_FAIL=$(echo "$HASH_RESULT" | grep -c "FAILED" || true)
  if [ "$HASH_FAIL" -gt 0 ]; then
    crit 7 "配置基线: 哈希校验 FAILED! (${HASH_FAIL} 个文件被修改) — 请确认是否为授权变更"
  else
    ok 7 "配置基线: 哈希校验通过, 权限合规"
  fi
else
  warn 7 "配置基线: 基线文件不存在, 请执行 sha256sum 生成"
fi

# ── 8. Yellow Line Cross-Validation ─────────────────────────────────────────
section 8 "Yellow Line Audit (sudo cross-validation)"
log "--- auth.log sudo entries (24h) ---"
SUDO_LOG=$(sudo grep "sudo:" /var/log/auth.log 2>/dev/null | tail -n 20 || true)
SUDO_COUNT=$(echo "$SUDO_LOG" | grep -c "COMMAND" || true)
log "$SUDO_LOG"

MEMORY_TODAY="$OC/workspace/memory/${DATE}.md"
MEMORY_YESTERDAY="$OC/workspace/memory/$(date -d 'yesterday' +%Y-%m-%d 2>/dev/null || date -v-1d +%Y-%m-%d 2>/dev/null || echo 'unknown').md"
log "--- Memory logs ---"
[ -f "$MEMORY_TODAY" ] && log "Today's memory log: exists" || log "Today's memory log: MISSING"
[ -f "$MEMORY_YESTERDAY" ] && log "Yesterday's memory log: exists" || log "Yesterday's memory log: N/A"

if [ "$SUDO_COUNT" -gt 0 ]; then
  warn 8 "黄线审计: ${SUDO_COUNT} 次 sudo 记录, 请与 memory 日志比对"
else
  ok 8 "黄线审计: 0 次 sudo 记录"
fi

# ── 9. Disk Usage ───────────────────────────────────────────────────────────
section 9 "Disk Usage"
DISK_USAGE=$(df -h / | tail -1 | awk '{print $5}' | tr -d '%')
log "Root partition: ${DISK_USAGE}% used"
log "--- Large files created in 24h (>100MB) ---"
LARGE_FILES=$(find / -maxdepth 5 -mtime -1 -size +100M -not -path '/proc/*' -not -path '/sys/*' -not -path '/dev/*' 2>/dev/null | head -n 10 || true)
LARGE_COUNT=$(echo "$LARGE_FILES" | grep -c "." || true)
[ -n "$LARGE_FILES" ] && log "$LARGE_FILES" || log "(none)"

if [ "$DISK_USAGE" -gt 85 ]; then
  warn 9 "磁盘: 根分区 ${DISK_USAGE}%, 新增大文件 ${LARGE_COUNT} 个"
else
  ok 9 "磁盘: 根分区 ${DISK_USAGE}%, 新增大文件 ${LARGE_COUNT} 个"
fi

# ── 10. Gateway Environment Variables ───────────────────────────────────────
section 10 "Gateway Environment (Keys/Tokens)"
GW_PID=$(pgrep -f "openclaw-gatewa" | head -1 || true)
if [ -z "$GW_PID" ]; then
  # fallback: try matching node process with openclaw
  GW_PID=$(pgrep -af "node.*openclaw" 2>/dev/null | grep -i "gateway\|dist/index" | head -1 | awk '{print $1}' || true)
fi
if [ -n "$GW_PID" ]; then
  log "Gateway PID: $GW_PID"
  ENV_KEYS=$(sudo cat /proc/$GW_PID/environ 2>/dev/null | tr '\0' '\n' | grep -iE "KEY|TOKEN|SECRET|PASS" | cut -d= -f1 || true)
  ENV_COUNT=$(echo "$ENV_KEYS" | grep -c "." || true)
  log "Sensitive env var names (values redacted): $ENV_KEYS"
  ok 10 "环境变量: ${ENV_COUNT} 个含敏感关键词(值已脱敏)"
else
  warn 10 "环境变量: Gateway 进程未找到"
fi

# ── 11. Sensitive Credential DLP Scan ───────────────────────────────────────
section 11 "Sensitive Credential DLP Scan (memory/logs)"
DLP_HITS=0
DLP_DETAILS=""

# Scan directories: workspace/memory, workspace/logs, workspace/*.md
SCAN_DIRS=("$OC/workspace/memory" "$OC/workspace/logs" "$OC/workspace")

for SCAN_DIR in "${SCAN_DIRS[@]}"; do
  [ -d "$SCAN_DIR" ] || continue

  if [ "$SCAN_DIR" = "$OC/workspace" ]; then
    # Only scan top-level .md files in workspace root (not recursing into skills/)
    SCAN_FILES=$(find "$SCAN_DIR" -maxdepth 1 -name '*.md' -type f 2>/dev/null || true)
  else
    SCAN_FILES=$(find "$SCAN_DIR" -type f -name '*.md' -o -name '*.log' -o -name '*.txt' -o -name '*.json' 2>/dev/null || true)
  fi

  while IFS= read -r f; do
    [ -z "$f" ] && continue
    [ -f "$f" ] || continue

    # Ethereum private key (64 hex chars after 0x or standalone)
    ETH_HIT=$(grep -cnE '(^|[^a-fA-F0-9])(0x)?[a-fA-F0-9]{64}([^a-fA-F0-9]|$)' "$f" 2>/dev/null || true)

    # BIP39 mnemonic pattern: 12 or 24 lowercase words separated by spaces
    # Heuristic: look for lines with 12+ consecutive lowercase words (common mnemonic pattern)
    MNEMONIC_HIT=$(grep -cnE '(\b[a-z]{3,8}\b[[:space:]]){11,23}\b[a-z]{3,8}\b' "$f" 2>/dev/null || true)

    # WIF Bitcoin private key (base58, starts with 5, K, or L, 51-52 chars)
    WIF_HIT=$(grep -cnE '(^|[^a-zA-Z0-9])[5KL][1-9A-HJ-NP-Za-km-z]{50,51}([^a-zA-Z0-9]|$)' "$f" 2>/dev/null || true)

    # xprv (BIP32 extended private key)
    XPRV_HIT=$(grep -cnE 'xprv[a-zA-Z0-9]{100,}' "$f" 2>/dev/null || true)

    TOTAL=$((ETH_HIT + MNEMONIC_HIT + WIF_HIT + XPRV_HIT))
    if [ "$TOTAL" -gt 0 ]; then
      DLP_HITS=$((DLP_HITS + TOTAL))
      REL_PATH="${f#$OC/}"
      DLP_DETAILS="${DLP_DETAILS}  - ${REL_PATH}: eth=${ETH_HIT} mnemonic=${MNEMONIC_HIT} wif=${WIF_HIT} xprv=${XPRV_HIT}\n"
    fi
  done <<< "$SCAN_FILES"
done

log "DLP scan completed. Hits: $DLP_HITS"
if [ -n "$DLP_DETAILS" ]; then
  log "$(echo -e "$DLP_DETAILS")"
fi

# Filter DLP hits through known issues
DLP_HITS_NEW=0
DLP_HITS_KNOWN=0
if [ "$DLP_HITS" -gt 0 ] && [ -n "$DLP_DETAILS" ]; then
  while IFS= read -r dlp_line; do
    [ -z "$dlp_line" ] && continue
    if is_known_issue "dlp_scan" "$dlp_line"; then
      DLP_HITS_KNOWN=$((DLP_HITS_KNOWN+1))
    else
      DLP_HITS_NEW=$((DLP_HITS_NEW+1))
    fi
  done <<< "$(echo -e "$DLP_DETAILS")"
fi

DLP_KNOWN_NOTE=""
[ "$DLP_HITS_KNOWN" -gt 0 ] && DLP_KNOWN_NOTE=" (${DLP_HITS_KNOWN} 处为已知误报)"

if [ "$DLP_HITS_NEW" -gt 0 ]; then
  crit 11 "凭证泄露扫描: 发现 ${DLP_HITS_NEW} 处新的疑似明文私钥/助记词!${DLP_KNOWN_NOTE}"
elif [ "$DLP_HITS_KNOWN" -gt 0 ]; then
  ok 11 "凭证泄露扫描: ${DLP_HITS} 处命中均为已知误报"
else
  ok 11 "凭证泄露扫描: 未发现明文私钥或助记词"
fi

# ── 12. Skill/MCP Integrity ─────────────────────────────────────────────────
section 12 "Skill/MCP Integrity"
SKILL_DIR="$OC/workspace/skills"
SKILL_BASELINE="$OC/.skill-baseline.sha256"
if [ -d "$SKILL_DIR" ]; then
  log "--- Installed Skills ---"
  ls -la "$SKILL_DIR" 2>/dev/null | grep "^d" | tee -a "$REPORT_FILE"

  # Generate current fingerprint
  CURRENT_HASH=$(find "$SKILL_DIR" -type f -not -path '*/.git/*' -not -name '*.pyc' -exec sha256sum {} \; 2>/dev/null | sort | sha256sum | awk '{print $1}')
  log "Current skill fingerprint: $CURRENT_HASH"

  if [ -f "$SKILL_BASELINE" ]; then
    BASELINE_HASH=$(cat "$SKILL_BASELINE")
    if [ "$CURRENT_HASH" = "$BASELINE_HASH" ]; then
      ok 12 "Skill 基线: 指纹校验通过"
    else
      warn 12 "Skill 基线: 指纹变化! (新: ${CURRENT_HASH:0:16}... 旧: ${BASELINE_HASH:0:16}...)"
    fi
  else
    log "No skill baseline found. Generating initial baseline."
    echo "$CURRENT_HASH" > "$SKILL_BASELINE"
    ok 12 "Skill 基线: 已生成初始指纹"
  fi
else
  ok 12 "Skill 基线: 无 skill 目录"
fi

# ── 13. Git Backup (Brain Disaster Recovery) ────────────────────────────────
section 13 "Brain Disaster Recovery (Git Backup)"
if [ -d "$OC/.git" ]; then
  cd "$OC"
  git add -A 2>/dev/null || true
  CHANGES_STAGED=$(git diff --cached --stat 2>/dev/null || true)
  if [ -n "$CHANGES_STAGED" ]; then
    git commit -m "chore(auto): nightly backup ${DATE}" 2>/dev/null || true
    PUSH_RESULT=$(git push 2>&1 || true)
    log "$PUSH_RESULT"
    if echo "$PUSH_RESULT" | grep -qiE "error|fatal|rejected"; then
      warn 13 "灾备: Git commit 成功但 push 失败(请检查远程仓库认证)"
    else
      ok 13 "灾备: 已自动推送至 Git 远程仓库"
    fi
  else
    ok 13 "灾备: 无变更, 跳过推送"
  fi
else
  warn 13 "灾备: $OC 未初始化 Git 仓库"
fi

# ── Summary ─────────────────────────────────────────────────────────────────
echo "" >> "$REPORT_FILE"
echo "=== SUMMARY ===" >> "$REPORT_FILE"
echo "Critical: $CRIT_COUNT | Warning: $WARN_COUNT | OK: $OK_COUNT" >> "$REPORT_FILE"

# Output summary for cron delivery
HEADER="🛡️ OpenClaw 每日安全巡检简报 (${DATE})"
if [ "$CRIT_COUNT" -gt 0 ]; then
  HEADER="🚨 OpenClaw 安全巡检告警 (${DATE}) — ${CRIT_COUNT} Critical"
elif [ "$WARN_COUNT" -gt 0 ]; then
  HEADER="⚠️ OpenClaw 安全巡检 (${DATE}) — ${WARN_COUNT} Warning"
fi

echo ""
echo "$HEADER"
echo "Summary: ${CRIT_COUNT} critical · ${WARN_COUNT} warn · ${OK_COUNT} ok"
echo -e "$SUMMARY"
echo ""
echo "📝 详细报告: $REPORT_FILE"

# ── Report Rotation (keep 30 days) ──────────────────────────────────────────
find "$REPORT_DIR" -name "report-*.txt" -mtime +30 -delete 2>/dev/null || true
