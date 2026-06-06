//! Minimal local web console for Buster v0.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::executor::respond_to_human;
use crate::{BusterDaemon, DaemonConfig, DaemonTickRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebConfig {
    pub daemon: DaemonConfig,
    pub host: String,
    pub port: u16,
    pub open_browser: bool,
}

impl WebConfig {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            daemon: DaemonConfig::new(root),
            host: "127.0.0.1".to_string(),
            port: 8787,
            open_browser: true,
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

struct WebState {
    root: PathBuf,
    daemon: Mutex<BusterDaemon>,
    next_tick: Mutex<usize>,
}

pub fn serve(config: WebConfig) -> std::io::Result<()> {
    let url = config.url();
    let listener = TcpListener::bind((&config.host[..], config.port))?;
    let state = Arc::new(WebState {
        root: config.daemon.root.clone(),
        daemon: Mutex::new(BusterDaemon::new(config.daemon)?),
        next_tick: Mutex::new(0),
    });

    if config.open_browser {
        let _ = open_browser(&url);
    }
    println!("Buster web console: {url}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                if let Err(error) = handle_connection(stream, state) {
                    eprintln!("web connection error: {error}");
                }
            }
            Err(error) => eprintln!("web accept error: {error}"),
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, state: Arc<WebState>) -> std::io::Result<()> {
    let mut buffer = vec![0u8; 128 * 1024];
    let read = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let Some(first_line) = request.lines().next() else {
        return respond_text(&mut stream, 400, "bad request");
    };
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    let body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or("");

    match (method, path) {
        ("GET", "/") => respond_html(&mut stream, 200, INDEX_HTML),
        ("GET", "/api/status") => respond_json(&mut stream, &status_json(&state.root)),
        ("POST", "/api/tick") => {
            let record = tick_json(&state)?;
            respond_json(&mut stream, &record)
        }
        ("POST", "/api/chat") => {
            let request: ChatRequest = serde_json::from_str(body).unwrap_or(ChatRequest {
                message: body.trim().to_string(),
            });
            let record = respond_to_human(&state.root, &request.message)?;
            respond_json(&mut stream, &record)
        }
        _ => respond_text(&mut stream, 404, "not found"),
    }
}

fn tick_json(state: &WebState) -> std::io::Result<DaemonTickRecord> {
    let mut index = state
        .next_tick
        .lock()
        .map_err(|_| std::io::Error::other("tick lock poisoned"))?;
    let mut daemon = state
        .daemon
        .lock()
        .map_err(|_| std::io::Error::other("daemon lock poisoned"))?;
    let record = daemon.tick(*index)?;
    *index += 1;
    Ok(record)
}

fn status_json(root: &Path) -> serde_json::Value {
    json!({
        "state": read_string(root.join("state").join("buster-daemon.json")),
        "runtime_tail": tail_lines(root.join("audit").join("runtime-cycle.jsonl"), 10),
        "research_tail": tail_lines(root.join("audit").join("research.jsonl"), 10),
        "chat_tail": tail_lines(root.join("audit").join("chat.jsonl"), 20),
    })
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    message: String,
}

fn respond_html(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    respond(stream, status, "text/html; charset=utf-8", body.as_bytes())
}

fn respond_json(stream: &mut TcpStream, value: &impl Serialize) -> std::io::Result<()> {
    let body = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    respond(stream, 200, "application/json; charset=utf-8", &body)
}

fn respond_text(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    respond(stream, status, "text/plain; charset=utf-8", body.as_bytes())
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

fn read_string(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn tail_lines(path: impl AsRef<Path>, max_lines: usize) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines = text.lines().map(ToString::to_string).collect::<Vec<_>>();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    lines
}

fn open_browser(url: &str) -> std::io::Result<()> {
    if cfg!(target_os = "windows") {
        Command::new("cmd.exe")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
    } else if Command::new("cmd.exe")
        .args(["/C", "start", "", url])
        .spawn()
        .is_ok()
    {
        Ok(())
    } else {
        Command::new("xdg-open").arg(url).spawn().map(|_| ())
    }
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Buster Console</title>
  <style>
    :root { color-scheme: light; font-family: Inter, ui-sans-serif, system-ui, "Segoe UI", sans-serif; }
    * { box-sizing: border-box; }
    body { margin: 0; background: #f5f6f8; color: #17202a; }
    main { max-width: 1180px; margin: 0 auto; min-height: 100vh; padding: 22px; display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(320px, .8fr); gap: 16px; }
    header { grid-column: 1 / -1; display: flex; align-items: center; justify-content: space-between; gap: 12px; }
    h1 { font-size: 22px; margin: 0; letter-spacing: 0; }
    h2 { font-size: 14px; margin: 0; letter-spacing: 0; }
    section { background: white; border: 1px solid #d9dee7; border-radius: 8px; padding: 14px; min-width: 0; }
    button { border: 1px solid #1f5eff; background: #1f5eff; color: white; border-radius: 6px; padding: 8px 12px; cursor: pointer; font-weight: 600; }
    button.secondary { background: white; color: #1f5eff; }
    button:disabled { cursor: wait; opacity: .65; }
    textarea { width: 100%; box-sizing: border-box; border: 1px solid #cbd3df; border-radius: 8px; padding: 11px 12px; resize: vertical; font: inherit; line-height: 1.45; background: white; color: #17202a; }
    pre { white-space: pre-wrap; overflow-wrap: anywhere; background: #f8fafc; border: 1px solid #e2e8f0; border-radius: 6px; padding: 10px; max-height: 320px; overflow: auto; margin: 10px 0 0; }
    .wide { grid-column: 1 / -1; }
    .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
    .muted { color: #5b6775; font-size: 13px; }
    .chat-shell { grid-row: span 2; padding: 0; display: flex; height: clamp(520px, calc(100vh - 112px), 760px); overflow: hidden; }
    .chat-panel { display: flex; flex-direction: column; width: 100%; min-height: 0; }
    .chat-top { padding: 14px 16px; border-bottom: 1px solid #e5e9f0; display: flex; justify-content: space-between; gap: 12px; align-items: center; }
    .chat-log { flex: 1; overflow: auto; padding: 22px 18px; background: #fbfbfc; }
    .empty-chat { color: #667085; text-align: center; padding: 80px 18px; }
    .message { display: grid; grid-template-columns: 34px minmax(0, 1fr); gap: 10px; max-width: 860px; margin: 0 auto 18px; }
    .message.user { grid-template-columns: minmax(0, 1fr) 34px; }
    .avatar { width: 34px; height: 34px; border-radius: 50%; display: grid; place-items: center; font-size: 12px; font-weight: 700; background: #17202a; color: white; }
    .message.user .avatar { background: #2f6f5e; grid-column: 2; grid-row: 1; }
    .bubble { border: 1px solid #e3e8ef; background: white; border-radius: 8px; padding: 11px 13px; line-height: 1.58; white-space: pre-wrap; overflow-wrap: anywhere; }
    .message.user .bubble { background: #e9f3ef; border-color: #d3e4dc; grid-column: 1; grid-row: 1; justify-self: end; max-width: min(760px, 100%); }
    .message.assistant .bubble { max-width: min(820px, 100%); }
    .message.error .bubble { border-color: #f1b8b8; background: #fff5f5; }
    .meta { margin-top: 7px; color: #667085; font-size: 12px; }
    .composer { border-top: 1px solid #e5e9f0; background: white; padding: 12px; }
    .composer-inner { max-width: 860px; margin: 0 auto; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 10px; align-items: end; }
    .composer textarea { min-height: 48px; max-height: 160px; }
    .side-note { margin-top: 8px; line-height: 1.45; }
    .tick-list { display: grid; gap: 8px; margin-top: 10px; max-height: 320px; overflow: auto; }
    .tick-card { border: 1px solid #e3e8ef; border-radius: 8px; padding: 10px; background: #fbfcfe; }
    .tick-card.latest { border-color: #9db8ff; background: #f3f6ff; }
    .tick-title { font-size: 13px; font-weight: 700; margin-bottom: 4px; }
    .tick-detail { color: #4d5968; font-size: 12px; line-height: 1.45; }
    .research-list { display: grid; gap: 10px; margin-top: 12px; }
    .research-card { border: 1px solid #e3e8ef; border-radius: 8px; padding: 12px; background: #fbfcfe; }
    .research-card.error { border-color: #f1b8b8; background: #fff8f8; }
    .research-card.ok { border-color: #b8dcc9; background: #f6fbf8; }
    .research-head { display: flex; justify-content: space-between; gap: 10px; align-items: center; margin-bottom: 6px; }
    .research-title { font-weight: 700; font-size: 13px; }
    .badge { border-radius: 999px; padding: 3px 8px; font-size: 12px; font-weight: 700; background: #e8edf5; color: #344054; }
    .badge.ok { background: #dff3e8; color: #17633d; }
    .badge.error { background: #fde2e2; color: #9f1d1d; }
    .research-summary { color: #344054; line-height: 1.5; white-space: pre-wrap; overflow-wrap: anywhere; }
    .research-meta { margin-top: 8px; color: #667085; font-size: 12px; line-height: 1.45; }
    @media (max-width: 900px) {
      main { grid-template-columns: 1fr; padding: 14px; }
      header { align-items: flex-start; flex-direction: column; }
      .wide, .chat-shell { grid-column: auto; }
      .chat-shell { height: calc(100vh - 132px); min-height: 500px; }
      .composer-inner { grid-template-columns: 1fr; }
    }
  </style>
</head>
<body>
<main>
  <header>
    <div>
      <h1>Buster Console</h1>
      <div class="muted">local runtime v0 · tick / research / chat</div>
    </div>
    <div class="row">
      <button id="tick-button" onclick="tick()">Tick + Execute</button>
      <button class="secondary" onclick="refresh()">Refresh</button>
      <span id="tick-status" class="muted"></span>
    </div>
  </header>

  <section class="chat-shell">
    <div class="chat-panel">
      <div class="chat-top">
        <div>
          <h2>Talk To Buster</h2>
          <div class="muted">Human dialogue channel</div>
        </div>
        <button class="secondary" onclick="refresh()">Refresh</button>
      </div>
      <div id="chat" class="chat-log">
        <div class="empty-chat">No chat yet.</div>
      </div>
      <div class="composer">
        <div class="composer-inner">
          <textarea id="message" rows="1" placeholder="和 Buster 说点什么..."></textarea>
          <button id="send-button" onclick="sendChat()">Send</button>
        </div>
      </div>
    </div>
  </section>

  <section>
    <h2>State</h2>
    <div class="muted side-note">当前身体模式、上一次行动选择和调度状态。</div>
    <pre id="state">loading...</pre>
  </section>

  <section>
    <h2>Runtime Cycles</h2>
    <div class="muted side-note">每个 tick 是 Buster 的一次运行周期：读取状态、评估价值模型、选择行动，并写入审计。</div>
    <div id="ticks" class="tick-list"></div>
  </section>

  <section class="wide">
    <h2>Research</h2>
    <div class="muted side-note">Buster 主动获取二手信息和前沿科学资料时，会把研究动作、模型、结果和报告路径记录在这里。</div>
    <div id="research" class="research-list"></div>
  </section>
</main>
<script>
let isSending = false;

function parseJsonLine(line) {
  try { return JSON.parse(line); } catch (_) { return null; }
}

function setText(el, text) {
  el.textContent = text || '';
}

function addMessage(root, role, text, meta, status) {
  const wrap = document.createElement('div');
  wrap.className = 'message ' + role + (status === 'error' ? ' error' : '');
  const avatar = document.createElement('div');
  avatar.className = 'avatar';
  avatar.textContent = role === 'user' ? 'YOU' : 'B';
  const bubble = document.createElement('div');
  bubble.className = 'bubble';
  setText(bubble, text);
  if (meta) {
    const metaEl = document.createElement('div');
    metaEl.className = 'meta';
    setText(metaEl, meta);
    bubble.appendChild(metaEl);
  }
  wrap.appendChild(avatar);
  wrap.appendChild(bubble);
  root.appendChild(wrap);
}

function renderChat(lines) {
  const root = document.getElementById('chat');
  root.innerHTML = '';
  const records = (lines || []).map(parseJsonLine).filter(Boolean);
  if (!records.length) {
    root.innerHTML = '<div class="empty-chat">No chat yet.</div>';
    return;
  }
  for (const record of records) {
    addMessage(root, 'user', record.human_message || '', '', record.status);
    const meta = [
      record.model ? 'model: ' + record.model : '',
      record.provider ? 'provider: ' + record.provider : '',
      record.total_tokens ? 'tokens: ' + record.total_tokens : ''
    ].filter(Boolean).join(' · ');
    addMessage(root, 'assistant', record.buster_reply || '', meta, record.status);
  }
  root.scrollTop = root.scrollHeight;
}

function renderTicks(lines, latestRecord) {
  const root = document.getElementById('ticks');
  root.innerHTML = '';
  const records = (lines || []).map(parseJsonLine).filter(Boolean).slice().reverse();
  if (latestRecord) records.unshift(latestRecord);
  if (!records.length) {
    root.innerHTML = '<div class="muted">No cycles yet.</div>';
    return;
  }
  const seen = new Set();
  records.forEach((record, index) => {
    const key = `${record.tick_index ?? '?'}-${record.timestamp_secs ?? '?'}-${record.selected_action_kind || ''}`;
    if (seen.has(key)) return;
    seen.add(key);
    const card = document.createElement('div');
    card.className = 'tick-card' + (index === 0 ? ' latest' : '');
    const title = document.createElement('div');
    title.className = 'tick-title';
    setText(title, `#${record.tick_index ?? '?'} · ${record.selected_action_kind || 'Unknown'} · ${record.body_mode || 'unknown'}`);
    const detail = document.createElement('div');
    detail.className = 'tick-detail';
    setText(detail, `${record.selected_action_summary || ''}\nscore: ${record.value_score ?? 'n/a'} · governance: ${record.governance_level ?? 'n/a'}${record.execution_summary ? '\n' + record.execution_summary : ''}`);
    card.appendChild(title);
    card.appendChild(detail);
    root.appendChild(card);
  });
}

function formatTime(seconds) {
  if (!seconds) return '';
  try { return new Date(seconds * 1000).toLocaleString(); } catch (_) { return ''; }
}

function renderResearch(lines) {
  const root = document.getElementById('research');
  root.innerHTML = '';
  const records = (lines || []).map(parseJsonLine).filter(Boolean).slice().reverse();
  if (!records.length) {
    root.innerHTML = '<div class="muted">No research yet.</div>';
    return;
  }
  for (const record of records) {
    const card = document.createElement('div');
    const status = record.status || 'unknown';
    card.className = 'research-card ' + status;

    const head = document.createElement('div');
    head.className = 'research-head';
    const title = document.createElement('div');
    title.className = 'research-title';
    setText(title, `${record.action_kind || 'Research'} · ${formatTime(record.timestamp_secs)}`);
    const badge = document.createElement('div');
    badge.className = 'badge ' + status;
    setText(badge, status);
    head.appendChild(title);
    head.appendChild(badge);

    const summary = document.createElement('div');
    summary.className = 'research-summary';
    setText(summary, record.summary || 'No summary.');

    const meta = document.createElement('div');
    meta.className = 'research-meta';
    const parts = [
      record.model ? 'model: ' + record.model : '',
      record.provider ? 'provider: ' + record.provider : '',
      record.total_tokens ? 'tokens: ' + record.total_tokens : '',
      record.output_path ? 'report: ' + record.output_path : ''
    ].filter(Boolean);
    setText(meta, parts.join(' · ') || 'No report generated.');

    card.appendChild(head);
    card.appendChild(summary);
    card.appendChild(meta);
    root.appendChild(card);
  }
}

async function refresh() {
  try {
    const res = await fetch('/api/status');
    const data = await res.json();
    document.getElementById('state').textContent = data.state || 'No state yet.';
    renderTicks(data.runtime_tail || []);
    renderResearch(data.research_tail || []);
    if (!isSending) renderChat(data.chat_tail || []);
  } catch (_) {
    // The v0 web server can be busy while an LLM request is in flight.
  }
}
async function tick() {
  const button = document.getElementById('tick-button');
  const status = document.getElementById('tick-status');
  button.disabled = true;
  status.textContent = 'running cycle...';
  try {
    const res = await fetch('/api/tick', { method: 'POST' });
    const data = await res.json();
    const statusRes = await fetch('/api/status');
    const statusData = await statusRes.json();
    document.getElementById('state').textContent = statusData.state || 'No state yet.';
    renderTicks(statusData.runtime_tail || [], data);
    renderResearch(statusData.research_tail || []);
    if (!isSending) renderChat(statusData.chat_tail || []);
    status.textContent = `done: ${data.selected_action_kind || 'cycle recorded'}`;
  } catch (_) {
    status.textContent = 'tick failed';
  } finally {
    button.disabled = false;
  }
}
async function sendChat() {
  const message = document.getElementById('message');
  const chat = document.getElementById('chat');
  const sendButton = document.getElementById('send-button');
  const text = message.value.trim();
  if (!text) return;
  isSending = true;
  message.value = '';
  sendButton.disabled = true;
  chat.innerHTML = '';
  addMessage(chat, 'user', text, '', 'ok');
  addMessage(chat, 'assistant', '正在思考...', '', 'ok');
  chat.scrollTop = chat.scrollHeight;
  try {
    const res = await fetch('/api/chat', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({message: text}) });
    const data = await res.json();
    renderChat([JSON.stringify(data)]);
    await refresh();
  } finally {
    isSending = false;
    sendButton.disabled = false;
  }
}
document.getElementById('message').addEventListener('keydown', (event) => {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault();
    sendChat();
  }
});
refresh();
setInterval(refresh, 5000);
</script>
</body>
</html>"#;
