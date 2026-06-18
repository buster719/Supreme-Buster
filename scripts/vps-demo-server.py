#!/usr/bin/env python3
import html
import json
import os
import subprocess
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(os.environ.get("BUSTER_ROOT", str(Path.home() / "buster-aux-node")))
CORE = ROOT / "buster-core"
PORT = int(os.environ.get("BUSTER_DEMO_PORT", "8788"))
HOST = os.environ.get("BUSTER_DEMO_HOST", "0.0.0.0")
TOKEN_FILE = ROOT / "secrets/relay-token.txt"
RELAY_INBOX = ROOT / "state/relay-inbox"
RELAY_OUTBOX = ROOT / "state/relay-outbox"
MAX_BODY = 256 * 1024
RATE_LIMIT_WINDOW_SECS = int(os.environ.get("BUSTER_DEMO_RATE_WINDOW_SECS", "60"))
PUBLIC_RATE_LIMIT = int(os.environ.get("BUSTER_DEMO_PUBLIC_RATE_LIMIT", "120"))
RELAY_RATE_LIMIT = int(os.environ.get("BUSTER_DEMO_RELAY_RATE_LIMIT", "60"))
RATE_BUCKETS = {}


def read_json(path):
    try:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    except Exception:
        return None


def read_text(path, limit=6000):
    try:
        return Path(path).read_text(encoding="utf-8", errors="replace")[:limit]
    except Exception:
        return ""


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    tmp.replace(path)


def tail_jsonl(path, limit=6):
    try:
        lines = [line for line in Path(path).read_text(encoding="utf-8", errors="replace").splitlines() if line.strip()]
    except Exception:
        return []
    out = []
    for line in lines[-limit:]:
        try:
            out.append(json.loads(line))
        except Exception:
            out.append({"raw": line[:800]})
    return out


def run_busterd(args, input_text=None, timeout=20):
    result = subprocess.run(
        [str(CORE / "target/debug/busterd"), *args],
        cwd=str(CORE),
        input=input_text,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stdout.strip() or f"busterd failed: {args}")
    return result.stdout.strip()


def verify_summary():
    try:
        return run_busterd(["node", "verify", "--root", str(ROOT)], timeout=8) or "no verify output"
    except Exception as exc:
        return f"verify unavailable: {exc}"


def service_status():
    try:
        result = subprocess.run(
            ["systemctl", "--user", "is-active", "buster-daemon.service", "buster-aux-heartbeat.timer"],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            timeout=4,
            check=False,
        )
        return [p.strip() for p in result.stdout.splitlines() if p.strip()] or ["unknown"]
    except Exception:
        return ["unknown"]


def export_snapshot():
    out = ROOT / "state/relay-local-snapshot.json"
    run_busterd(["node", "snapshot", "--root", str(ROOT), "--out", str(out)], timeout=20)
    return read_json(out)


def witness_subject_snapshot(snapshot):
    node_id = snapshot.get("node_identity", {}).get("node_id", "unknown")
    safe_node = safe_id(node_id)
    snapshot_path = RELAY_INBOX / f"{safe_node}-snapshot.json"
    receipt_path = RELAY_OUTBOX / f"{safe_node}-receipt.json"
    write_json(snapshot_path, snapshot)
    output = run_busterd(["node", "witness", "--root", str(ROOT), "--snapshot", str(snapshot_path)], timeout=25)
    receipt = json.loads(output)
    write_json(receipt_path, receipt)
    return receipt


def import_receipt(receipt):
    witness = safe_id(receipt.get("witness_node_id", "unknown"))
    subject = safe_id(receipt.get("subject_node_id", "unknown"))
    path = RELAY_INBOX / f"receipt-{witness}-for-{subject}.json"
    write_json(path, receipt)
    run_busterd(["node", "import-receipt", "--root", str(ROOT), "--receipt", str(path)], timeout=20)
    return {"status": "imported", "path": str(path)}


def safe_id(value):
    return "".join(ch if ch.isalnum() or ch in "._-" else "_" for ch in str(value))[:120]


def relay_token():
    return TOKEN_FILE.read_text(encoding="utf-8").strip() if TOKEN_FILE.exists() else ""


def authorized(handler):
    token = relay_token()
    if not token:
        return False
    return handler.headers.get("X-Buster-Relay-Token", "") == token


def rate_limited(client_ip, scope, limit):
    now = time.time()
    key = (client_ip, scope)
    bucket = [stamp for stamp in RATE_BUCKETS.get(key, []) if now - stamp < RATE_LIMIT_WINDOW_SECS]
    if len(bucket) >= limit:
        RATE_BUCKETS[key] = bucket
        return True
    bucket.append(now)
    RATE_BUCKETS[key] = bucket
    return False


def research_cards():
    records = tail_jsonl(ROOT / "audit/research.jsonl", 8)
    cards = []
    for item in reversed(records):
        status = item.get("status", "unknown")
        summary = item.get("summary", item.get("raw", ""))
        model = item.get("model") or ""
        provider = item.get("provider") or ""
        sources = item.get("source_count")
        output_path = item.get("output_path") or ""
        title = item.get("action_kind", "Research")
        meta = " · ".join(str(x) for x in [status, provider, model, f"sources={sources}" if sources is not None else ""] if x)
        cards.append(f"""
        <article class=\"card\">
          <div class=\"card-head\"><strong>{esc(title)}</strong><span>{esc(meta)}</span></div>
          <p>{esc(summary)}</p>
          <small>{esc(output_path)}</small>
        </article>
        """)
    return "\n".join(cards) or "<p class='muted'>No research records yet.</p>"


def witness_cards():
    registry = read_json(ROOT / "state/known-nodes.json") or {}
    nodes = registry.get("nodes") or []
    receipts = tail_jsonl(ROOT / "audit/witness-receipts.jsonl", 6)
    if not nodes:
        return "<p class='muted'>No witnessed nodes yet.</p>"
    cards = []
    for node in nodes:
        head = node.get("last_chain_head") or "none"
        cards.append(f"""
        <article class=\"card\">
          <div class=\"card-head\"><strong>{esc(node.get('node_id','unknown'))}</strong><span>{esc(node.get('role','unknown'))}</span></div>
          <p>Last observed chain head: <code>{esc(head)}</code></p>
          <small>events={esc(node.get('last_event_count','0'))} · first_seen={esc(node.get('first_seen_secs',''))} · last_seen={esc(node.get('last_seen_secs',''))}</small>
        </article>
        """)
    if receipts:
        items = []
        for receipt in reversed(receipts):
            items.append(
                f"<li><strong>{esc(receipt.get('witness_node_id','?'))}</strong> witnessed "
                f"<strong>{esc(receipt.get('subject_node_id','?'))}</strong> · "
                f"events={esc(receipt.get('subject_event_count','?'))}</li>"
            )
        cards.append("<ul>" + "\n".join(items) + "</ul>")
    return "\n".join(cards)


def event_cards():
    events = tail_jsonl(ROOT / "audit/events.jsonl", 6)
    rows = []
    for event in reversed(events):
        rows.append(f"<li><strong>{esc(event.get('event_type','event'))}</strong> <span>{esc(event.get('event_id',''))}</span></li>")
    return "\n".join(rows) or "<li>No events yet.</li>"


def digest_html():
    summary = read_text(ROOT / "research/RESEARCH_SUMMARY.md", 8000)
    if not summary:
        return "<p class='muted'>No research summary yet.</p>"
    return "<pre>" + esc(summary) + "</pre>"


def esc(value):
    return html.escape(str(value), quote=True)


def render_page():
    identity = read_json(ROOT / "state/node-identity.json") or {}
    digest = read_json(ROOT / "state/research-digest.json") or {}
    verify = verify_summary()
    svc = service_status()
    now = time.strftime("%Y-%m-%d %H:%M:%S UTC", time.gmtime())
    checked = "unknown"
    if "checked_events=" in verify:
        checked = verify.split("checked_events=", 1)[1].split()[0]
    valid = "valid=true" in verify
    status_class = "ok" if valid else "bad"
    daemon_status = " / ".join(svc)
    latest_topic = digest.get("latest_report", {}).get("question") if isinstance(digest.get("latest_report"), dict) else None
    return f"""<!doctype html>
<html lang=\"zh-CN\"><head><meta charset=\"utf-8\" /><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />
<title>Buster Public Demo</title><style>
:root {{ font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif; color: #17202a; background: #f4f6f8; }}
* {{ box-sizing: border-box; }} body {{ margin: 0; }} main {{ max-width: 1120px; margin: 0 auto; padding: 28px 18px 42px; }}
header {{ display: flex; justify-content: space-between; gap: 18px; align-items: flex-start; margin-bottom: 18px; }}
h1 {{ margin: 0; font-size: clamp(28px, 4vw, 44px); letter-spacing: 0; }} h2 {{ margin: 0 0 10px; font-size: 17px; letter-spacing: 0; }} p {{ line-height: 1.62; }}
.grid {{ display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }} section, .metric, .card {{ background: white; border: 1px solid #dbe2ea; border-radius: 8px; padding: 16px; }} section {{ margin-top: 12px; }}
.metric strong {{ display: block; font-size: 22px; margin-top: 4px; overflow-wrap: anywhere; }} .muted, small, span {{ color: #667085; }}
.badge {{ display: inline-flex; align-items: center; border: 1px solid #cfd7e3; border-radius: 999px; padding: 5px 9px; font-size: 12px; background: white; }} .ok {{ color: #0f6b3f; border-color: #a7d8bd; background: #f1fbf5; }} .bad {{ color: #9b1c1c; border-color: #efb4b4; background: #fff5f5; }}
.card {{ margin-top: 10px; }} .card-head {{ display: flex; justify-content: space-between; gap: 12px; align-items: baseline; }} .card p {{ margin-bottom: 8px; }} code {{ overflow-wrap: anywhere; }}
pre {{ white-space: pre-wrap; overflow-wrap: anywhere; margin: 0; background: #f8fafc; border: 1px solid #e2e8f0; border-radius: 8px; padding: 12px; max-height: 520px; overflow: auto; line-height: 1.55; }} ul {{ margin: 0; padding-left: 20px; }} li {{ margin: 7px 0; overflow-wrap: anywhere; }}
@media (max-width: 860px) {{ header {{ display: block; }} .grid {{ grid-template-columns: 1fr; }} }}
</style></head><body><main>
<header><div><h1>Buster Public Demo</h1><p class=\"muted\">只读展示页：这是 VPS 上的 Buster 辅助节点，不提供聊天、tick 或执行权限。</p></div><div class=\"badge {status_class}\">{esc(verify)}</div></header>
<div class=\"grid\"><div class=\"metric\"><span>Node</span><strong>{esc(identity.get('node_id','unknown'))}</strong><small>{esc(identity.get('role','unknown'))}</small></div><div class=\"metric\"><span>Services</span><strong>{esc(daemon_status)}</strong><small>daemon / heartbeat</small></div><div class=\"metric\"><span>Signed Events</span><strong>{esc(checked)}</strong><small>{esc(now)}</small></div></div>
<section><h2>What This Shows</h2><p>Buster 当前不是普通网页聊天机器人，而是一个正在 VPS 上持续运行的 agent 节点。它维护节点身份、签名事件链、研究记录和运行摘要。这个页面只展示它的外部可观察状态。</p><p><strong>Latest topic:</strong> {esc(latest_topic or 'No latest topic yet.')}</p></section>
<section><h2>Witness Network</h2><p class=\"muted\">这些记录表示本节点已经观察并绑定过其他 Buster 节点的公开见证密钥和事件链 head。</p>{witness_cards()}</section>
<section><h2>Recent Research</h2>{research_cards()}</section>
<section><h2>Research Digest</h2>{digest_html()}</section>
<section><h2>Recent Signed Events</h2><ul>{event_cards()}</ul></section>
</main></body></html>""".encode("utf-8")


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"{self.client_address[0]} - {fmt % args}")

    def send_bytes(self, status, body, content_type):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header("Content-Security-Policy", "default-src 'self'; style-src 'unsafe-inline'")
        self.end_headers()
        self.wfile.write(body)

    def send_json(self, value, status=200):
        self.send_bytes(status, json.dumps(value, ensure_ascii=False).encode("utf-8"), "application/json; charset=utf-8")

    def read_json_body(self):
        length = int(self.headers.get("Content-Length", "0") or "0")
        if length > MAX_BODY:
            raise ValueError("body too large")
        raw = self.rfile.read(length)
        return json.loads(raw.decode("utf-8"))

    def require_auth(self):
        if authorized(self):
            return True
        self.send_json({"error": "unauthorized"}, 401)
        return False

    def do_GET(self):
        path = urlparse(self.path).path
        scope = "relay" if path.startswith("/relay/") else "public"
        limit = RELAY_RATE_LIMIT if scope == "relay" else PUBLIC_RATE_LIMIT
        if rate_limited(self.client_address[0], scope, limit):
            return self.send_json({"error": "rate limit exceeded"}, 429)
        if path in ("/", "/index.html"):
            return self.send_bytes(200, render_page(), "text/html; charset=utf-8")
        if path == "/healthz":
            return self.send_bytes(200, b"ok\n", "text/plain; charset=utf-8")
        if path == "/relay/healthz":
            return self.send_json({"status": "ok", "node_id": (read_json(ROOT / "state/node-identity.json") or {}).get("node_id")})
        if path == "/relay/snapshot":
            if not self.require_auth():
                return
            try:
                return self.send_json(export_snapshot())
            except Exception as exc:
                return self.send_json({"error": str(exc)}, 500)
        if path.startswith("/relay/receipts/"):
            if not self.require_auth():
                return
            node_id = safe_id(path.rsplit("/", 1)[-1])
            receipt = read_json(RELAY_OUTBOX / f"{node_id}-receipt.json")
            if receipt is None:
                return self.send_json({"error": "not found"}, 404)
            return self.send_json(receipt)
        if path == "/relay/registry":
            if not self.require_auth():
                return
            return self.send_json(read_json(ROOT / "state/known-nodes.json") or {"version": 1, "nodes": []})
        self.send_error(404, "not found")

    def do_POST(self):
        path = urlparse(self.path).path
        if rate_limited(self.client_address[0], "relay", RELAY_RATE_LIMIT):
            return self.send_json({"error": "rate limit exceeded"}, 429)
        if path == "/relay/snapshots":
            if not self.require_auth():
                return
            try:
                receipt = witness_subject_snapshot(self.read_json_body())
                return self.send_json(receipt)
            except Exception as exc:
                return self.send_json({"error": str(exc)}, 400)
        if path == "/relay/receipts":
            if not self.require_auth():
                return
            try:
                return self.send_json(import_receipt(self.read_json_body()))
            except Exception as exc:
                return self.send_json({"error": str(exc)}, 400)
        self.send_error(405, "read only demo")


if __name__ == "__main__":
    RELAY_INBOX.mkdir(parents=True, exist_ok=True)
    RELAY_OUTBOX.mkdir(parents=True, exist_ok=True)
    server = ThreadingHTTPServer((HOST, PORT), Handler)
    print(f"Buster public demo + relay listening on http://{HOST}:{PORT}/ root={ROOT}", flush=True)
    server.serve_forever()
