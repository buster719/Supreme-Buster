//! Minimal local web console for Buster v0.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use buster_skills::SkillWorkspace;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::append_daemon_jsonl;
use crate::executor::respond_to_human;
use crate::{now_secs, BusterDaemon, DaemonConfig, DaemonTickRecord};

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
    daemon_config: DaemonConfig,
    next_tick: Mutex<usize>,
    owner_token: String,
}

pub fn serve(config: WebConfig) -> std::io::Result<()> {
    let url = config.url();
    let listener = TcpListener::bind((&config.host[..], config.port))?;
    let owner_token = ensure_owner_token(&config.daemon.root)?;
    let state = Arc::new(WebState {
        root: config.daemon.root.clone(),
        daemon_config: config.daemon.clone(),
        next_tick: Mutex::new(0),
        owner_token,
    });

    if config.open_browser {
        let _ = open_browser(&url);
    }
    println!("Buster web console: {url}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, state) {
                        eprintln!("web connection error: {error}");
                    }
                });
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
    let headers = parse_headers(&request);
    let body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or("");

    if method == "POST" {
        if !is_console_post(&headers) {
            return respond_text(&mut stream, 403, "missing Buster console header");
        }
        if !is_owner_post(&headers, &state.owner_token) {
            return respond_text(&mut stream, 403, "missing or invalid Buster owner token");
        }
    }
    if body.len() > 64 * 1024 {
        return respond_text(&mut stream, 413, "request body too large");
    }

    match (method, path) {
        ("GET", "/") => {
            let html = index_html(&state.owner_token);
            respond_html(&mut stream, 200, &html)
        }
        ("GET", "/api/status") => respond_json(&mut stream, &status_json(&state.root)),
        ("POST", "/api/tick") => {
            let record = tick_json(&state)?;
            respond_json(&mut stream, &record)
        }
        ("POST", "/api/chat") => {
            let request: ChatRequest = serde_json::from_str(body).unwrap_or(ChatRequest {
                message: body.trim().to_string(),
            });
            let record = respond_to_human(&state.root, &request.message, true)?;
            respond_json(&mut stream, &record)
        }
        ("POST", "/api/skills/approve") => {
            let request: SkillApproveRequest =
                serde_json::from_str(body).unwrap_or(SkillApproveRequest { names: Vec::new() });
            let response = approve_skills_json(&state.root, request)?;
            respond_json(&mut stream, &response)
        }
        _ => respond_text(&mut stream, 404, "not found"),
    }
}

fn tick_json(state: &WebState) -> std::io::Result<DaemonTickRecord> {
    let mut index = state
        .next_tick
        .lock()
        .map_err(|_| std::io::Error::other("tick lock poisoned"))?;
    let mut daemon = BusterDaemon::new(state.daemon_config.clone())?;
    let record = daemon.tick(*index)?;
    *index += 1;
    Ok(record)
}

fn status_json(root: &Path) -> serde_json::Value {
    let _ = crate::autobiography::refresh(root);
    json!({
        "state": read_string(root.join("state").join("buster-daemon.json")),
        "autobiography": read_json_value(root.join("state").join("autobiography.json")),
        "runtime_tail": tail_lines(root.join("audit").join("runtime-cycle.jsonl"), 10),
        "research_tail": tail_lines(root.join("audit").join("research.jsonl"), 10),
        "research_ledger_tail": tail_lines(root.join("audit").join("research-ledger.jsonl"), 10),
        "source_fetch_tail": tail_lines(root.join("audit").join("source-fetch.jsonl"), 10),
        "research_queue": read_json_value(root.join("state").join("research-queue.json")),
        "research_digest": read_json_value(root.join("state").join("research-digest.json")),
        "research_quality_digest": read_json_value(root.join("state").join("research-quality-digest.json")),
        "paper_acquisition_tail": tail_lines(root.join("audit").join("paper-acquisition.jsonl"), 10),
        "paper_brief_tail": tail_lines(root.join("audit").join("paper-brief.jsonl"), 10),
        "paperqa_tail": tail_lines(root.join("audit").join("paperqa.jsonl"), 10),
        "skill_registry": read_json_value(root.join("state").join("skill-registry.json")),
        "skill_events_tail": tail_lines(root.join("audit").join("skill-events.jsonl"), 20),
        "tool_registry": read_json_value(root.join("state").join("tool-registry.json")),
        "tool_events_tail": tail_lines(root.join("audit").join("tool-events.jsonl"), 20),
        "chat_agent_steps_tail": tail_lines(root.join("audit").join("chat-agent-steps.jsonl"), 20),
        "harness_contract_tail": tail_lines(root.join("audit").join("harness-contracts.jsonl"), 10),
        "harness_outcome_tail": tail_lines(root.join("audit").join("harness-outcomes.jsonl"), 10),
        "security_taskflows": read_json_value(root.join("state").join("security-taskflows.json")),
        "security_research_tail": tail_lines(root.join("audit").join("security-research.jsonl"), 10),
        "protocol_security": read_json_value(root.join("state").join("protocol-security-report.json")),
        "protocol_security_tail": tail_lines(root.join("audit").join("protocol-security.jsonl"), 10),
        "chat_tail": tail_lines(root.join("audit").join("chat.jsonl"), 20),
    })
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    message: String,
}

#[derive(Debug, Deserialize)]
struct SkillApproveRequest {
    #[serde(default)]
    names: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SkillApproveResponse {
    status: String,
    approved: Vec<String>,
    failed: Vec<String>,
}

fn approve_skills_json(
    root: &Path,
    request: SkillApproveRequest,
) -> std::io::Result<SkillApproveResponse> {
    let mut approved = Vec::new();
    let mut failed = Vec::new();
    let names = request
        .names
        .into_iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .take(24)
        .collect::<Vec<_>>();
    if names.is_empty() {
        return Ok(SkillApproveResponse {
            status: "empty".to_string(),
            approved,
            failed: vec!["no skill selected".to_string()],
        });
    }
    let workspace = SkillWorkspace::new(root);
    for name in names {
        match workspace.approve_installed_skill(&name, "local-owner-console-ui") {
            Ok(entry) => {
                append_daemon_jsonl(
                    &root.join("audit").join("owner-consent.jsonl"),
                    &json!({
                        "timestamp_secs": now_secs(),
                        "authority": "local_owner_console",
                        "action": "approve_skill",
                        "target": entry.name,
                        "status": "approved",
                        "consent_basis": "valid X-Buster-Owner-Token on localhost web console checkbox approval",
                    }),
                )?;
                approved.push(entry.name);
            }
            Err(error) => failed.push(format!("{name}: {error}")),
        }
    }
    Ok(SkillApproveResponse {
        status: if failed.is_empty() { "ok" } else { "partial" }.to_string(),
        approved,
        failed,
    })
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
        403 => "Forbidden",
        413 => "Payload Too Large",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

fn parse_headers(request: &str) -> Vec<(String, String)> {
    request
        .lines()
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect()
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    let name = name.to_ascii_lowercase();
    headers
        .iter()
        .find(|(header_name, _)| header_name == &name)
        .map(|(_, value)| value.as_str())
}

fn is_console_post(headers: &[(String, String)]) -> bool {
    header_value(headers, "x-buster-console") == Some("1")
}

fn is_owner_post(headers: &[(String, String)], owner_token: &str) -> bool {
    header_value(headers, "x-buster-owner-token") == Some(owner_token)
}

fn ensure_owner_token(root: &Path) -> std::io::Result<String> {
    let path = root.join("state").join("local-owner-token.txt");
    if let Ok(token) = fs::read_to_string(&path) {
        let token = token.trim();
        if token.len() >= 24 {
            return Ok(token.to_string());
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = URL_SAFE_NO_PAD.encode(bytes);
    fs::write(&path, &token)?;
    Ok(token)
}

fn index_html(owner_token: &str) -> String {
    INDEX_HTML.replace("__BUSTER_OWNER_TOKEN__", owner_token)
}

fn read_string(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn read_json_value(path: impl AsRef<Path>) -> Option<serde_json::Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_post_requires_custom_header() {
        let headers = parse_headers(
            "POST /api/chat HTTP/1.1\r\nHost: 127.0.0.1:8787\r\nContent-Type: application/json\r\n\r\n{}",
        );
        assert!(!is_console_post(&headers));

        let headers = parse_headers(
            "POST /api/chat HTTP/1.1\r\nHost: 127.0.0.1:8787\r\nX-Buster-Console: 1\r\n\r\n{}",
        );
        assert!(is_console_post(&headers));
    }

    #[test]
    fn approve_skills_api_activates_selected_pending_skill() {
        let root =
            std::env::temp_dir().join(format!("buster-web-skill-approve-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let workspace = SkillWorkspace::new(&root);
        workspace
            .install_markdown_skill(
                "# research-synthesis\n\n## Trigger\nWhen sources need a summary.\n\n## Purpose\nSummarize evidence.\n\n## Procedure\nRead source snippets and write claims.\n\n## Validation\nCheck each claim has support.\n",
                "unit-test",
                None,
                false,
            )
            .unwrap();

        let response = approve_skills_json(
            &root,
            SkillApproveRequest {
                names: vec!["research-synthesis".to_string()],
            },
        )
        .unwrap();

        assert_eq!(response.status, "ok");
        assert_eq!(response.approved, vec!["research-synthesis".to_string()]);
        let registry = workspace.refresh_registry().unwrap();
        let entry = registry
            .entries
            .iter()
            .find(|entry| entry.name == "research-synthesis")
            .unwrap();
        assert_eq!(format!("{:?}", entry.status), "Active");
        assert!(root.join("audit/owner-consent.jsonl").exists());
        let _ = fs::remove_dir_all(root);
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
    .view-tabs { grid-column: 1 / -1; position: sticky; top: 0; z-index: 20; background: #f5f6f8; border: 0; padding: 0 0 4px; display: flex; gap: 8px; flex-wrap: wrap; }
    .view-tab { background: white; color: #344054; border-color: #cbd3df; }
    .view-tab.active { background: #17202a; border-color: #17202a; color: white; }
    .tab-section { display: none; }
    .tab-section.active { display: block; }
    .chat-shell.tab-section.active { display: flex; }
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
    .queue-strip { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 8px; margin-top: 12px; }
    .queue-card { border: 1px solid #e3e8ef; border-radius: 8px; padding: 10px; background: #fff; min-width: 0; }
    .queue-title { font-weight: 700; font-size: 12px; color: #1f2937; overflow-wrap: anywhere; }
    .queue-meta { margin-top: 6px; color: #667085; font-size: 12px; line-height: 1.4; }
    .digest-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: 8px; margin-top: 12px; }
    .digest-stat { border: 1px solid #e3e8ef; border-radius: 8px; padding: 10px; background: #fff; }
    .digest-stat strong { display: block; font-size: 18px; color: #101828; }
    .digest-stat span { color: #667085; font-size: 12px; }
    .review-bar { margin-top: 10px; display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
    .review-card { border: 1px solid #f2c47d; background: #fffaf0; }
    .review-card.high-risk { border-color: #f1a5a5; background: #fff7f7; }
    .review-row { display: grid; grid-template-columns: 22px minmax(0, 1fr); gap: 10px; align-items: start; }
    .review-row input { width: 18px; height: 18px; margin-top: 2px; }
    .review-source { overflow-wrap: anywhere; }
    .review-status { font-size: 13px; color: #5b6775; }
    @media (max-width: 900px) {
      main { grid-template-columns: 1fr; padding: 14px; }
      header { align-items: flex-start; flex-direction: column; }
      .wide, .chat-shell { grid-column: auto; }
      .view-tabs { grid-column: auto; }
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

  <nav class="view-tabs">
    <button class="view-tab active" data-view-button="chat" onclick="setActiveView('chat')">Chat</button>
    <button class="view-tab" data-view-button="skills" onclick="setActiveView('skills')">Reviews / Skills</button>
    <button class="view-tab" data-view-button="research" onclick="setActiveView('research')">Research</button>
    <button class="view-tab" data-view-button="runtime" onclick="setActiveView('runtime')">Runtime</button>
    <button class="view-tab" data-view-button="safety" onclick="setActiveView('safety')">Safety</button>
    <button class="view-tab" data-view-button="tools" onclick="setActiveView('tools')">Tools</button>
  </nav>

  <section class="chat-shell tab-section active" data-view="chat">
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

  <section class="tab-section" data-view="runtime">
    <h2>State</h2>
    <div class="muted side-note">当前身体模式、上一次行动选择和调度状态。</div>
    <pre id="state">loading...</pre>
  </section>

  <section class="tab-section" data-view="runtime">
    <h2>Runtime Cycles</h2>
    <div class="muted side-note">每个 tick 是 Buster 的一次运行周期：读取状态、评估价值模型、选择行动，并写入审计。</div>
    <div id="ticks" class="tick-list"></div>
  </section>

  <section class="wide tab-section" data-view="safety">
    <h2>Harness</h2>
    <div class="muted side-note">Harness 是每次行动的 Plan / Execute / Verify 合同：说明目标、边界、证据和结果。</div>
    <div id="harness" class="research-list"></div>
  </section>

  <section class="wide tab-section" data-view="safety">
    <h2>Agent Steps</h2>
    <div class="muted side-note">对话中的计划、工具调用和观察结果会记录在这里，方便确认 Buster 是否真的执行了指令。</div>
    <div id="agent-steps" class="research-list"></div>
  </section>

  <section class="wide tab-section" data-view="safety">
    <h2>Security Research</h2>
    <div class="muted side-note">防御性安全研究，只审计授权范围、本地仓库和沙盒；外部目标必须显式授权并走 BodyGate。</div>
    <div id="security-research" class="research-list"></div>
  </section>

  <section class="wide tab-section" data-view="safety">
    <h2>Protocol Security</h2>
    <div class="muted side-note">见证层和事件链的 invariant 检查：重放、签名域、见证回执和节点注册一致性。</div>
    <div id="protocol-security" class="research-list"></div>
  </section>

  <section class="wide tab-section" data-view="skills">
    <h2>Skills</h2>
    <div class="muted side-note">Skill 是可复用的任务方法。它们不等同于工具或身份；涉及工具、网络、账号或进程时仍需要通过身体层。</div>
    <div id="skill-review-panel"></div>
    <div id="skills" class="research-list"></div>
    <pre id="skill-events"></pre>
  </section>

  <section class="wide tab-section" data-view="tools">
    <h2>Tools</h2>
    <div class="muted side-note">Tool 是第三层可执行能力。Buster 可以查询工具，但实际调用、网络、secret 和外部副作用仍由第四层 BodyGate 控制。</div>
    <div id="tools" class="research-list"></div>
    <pre id="tool-events"></pre>
  </section>

  <section class="wide tab-section" data-view="research">
    <h2>Research</h2>
    <div class="muted side-note">Buster 主动获取二手信息和前沿科学资料时，会把研究动作、模型、结果和报告路径记录在这里。</div>
    <div id="research-digest"></div>
    <div id="research-queue" class="queue-strip"></div>
    <div id="paper-acquisition" class="research-list"></div>
    <div id="paper-brief" class="research-list"></div>
    <div id="paperqa" class="research-list"></div>
    <div id="source-fetch" class="research-list"></div>
    <div id="research" class="research-list"></div>
    <div id="research-ledger" class="research-list"></div>
  </section>
</main>
<script>
let isSending = false;
let shouldStickToChatBottom = true;
let forceNextChatScroll = false;
let activeView = 'chat';
let userSelectedView = false;
let pendingSkillSelections = new Set();
const OWNER_TOKEN = "__BUSTER_OWNER_TOKEN__";

function setActiveView(view, userInitiated = true) {
  activeView = view;
  if (userInitiated) userSelectedView = true;
  for (const section of document.querySelectorAll('.tab-section')) {
    section.classList.toggle('active', section.dataset.view === view);
  }
  for (const button of document.querySelectorAll('.view-tab')) {
    button.classList.toggle('active', button.dataset.viewButton === view);
  }
}

function maybeAutoShowSkillReviews(registry) {
  const pendingCount = ((registry && registry.entries) || [])
    .filter(entry => entry.scope === 'Installed' && entry.status === 'NeedsReview')
    .length;
  if (pendingCount > 0 && !userSelectedView && activeView === 'chat') {
    setActiveView('skills', false);
  }
}

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

function isNearBottom(el) {
  return el.scrollHeight - el.scrollTop - el.clientHeight < 48;
}

function renderChat(lines, options = {}) {
  const root = document.getElementById('chat');
  const previousScrollTop = root.scrollTop;
  const previousScrollHeight = root.scrollHeight;
  const wasNearBottom = isNearBottom(root);
  root.innerHTML = '';
  const records = (lines || []).map(parseJsonLine).filter(Boolean);
  if (!records.length) {
    root.innerHTML = '<div class="empty-chat">No chat yet.</div>';
    forceNextChatScroll = false;
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
  if (options.forceScroll || forceNextChatScroll || wasNearBottom || shouldStickToChatBottom) {
    root.scrollTop = root.scrollHeight;
    shouldStickToChatBottom = true;
  } else {
    const heightDelta = root.scrollHeight - previousScrollHeight;
    root.scrollTop = Math.max(0, previousScrollTop + heightDelta);
  }
  forceNextChatScroll = false;
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

function renderHarness(contractLines, outcomeLines) {
  const root = document.getElementById('harness');
  if (!root) return;
  root.innerHTML = '';
  const contracts = (contractLines || []).map(parseJsonLine).filter(Boolean).slice().reverse();
  const outcomes = (outcomeLines || []).map(parseJsonLine).filter(Boolean).slice().reverse();
  const outcomeByContract = new Map();
  for (const outcome of outcomes) outcomeByContract.set(outcome.contract_id, outcome);
  const records = contracts.slice(0, 6);
  if (!records.length) {
    root.innerHTML = '<div class="muted">No harness contracts yet.</div>';
    return;
  }
  for (const contract of records) {
    const outcome = outcomeByContract.get(contract.contract_id);
    const card = document.createElement('div');
    card.className = 'research-card';
    const title = document.createElement('div');
    title.className = 'research-title';
    setText(title, `${contract.action_kind || 'Action'} · ${contract.risk_level || 'risk'} · L${contract.change_level ?? '?'}`);
    const summary = document.createElement('div');
    summary.className = 'research-summary';
    const plan = (contract.plan || []).slice(0, 4).map(step => `- ${step.step_id}: ${step.summary}`).join('\n');
    const verification = (contract.verification || []).slice(0, 3).map(step => `- ${step.step_id}: ${step.summary}`).join('\n');
    setText(summary, [
      contract.goal ? 'goal: ' + contract.goal : '',
      outcome ? `outcome: ${outcome.status} · ${outcome.summary || ''}` : 'outcome: pending',
      plan ? 'plan:\n' + plan : '',
      verification ? 'verify:\n' + verification : '',
    ].filter(Boolean).join('\n\n'));
    const meta = document.createElement('div');
    meta.className = 'research-meta';
    setText(meta, `contract: ${contract.contract_id || 'n/a'} · trigger: ${contract.trigger || 'n/a'}`);
    card.appendChild(title);
    card.appendChild(summary);
    card.appendChild(meta);
    root.appendChild(card);
  }
}

function renderAgentSteps(lines) {
  const root = document.getElementById('agent-steps');
  if (!root) return;
  root.innerHTML = '';
  const records = (lines || []).map(parseJsonLine).filter(Boolean).slice().reverse().slice(0, 8);
  if (!records.length) {
    root.innerHTML = '<div class="muted">No agent steps yet.</div>';
    return;
  }
  for (const record of records) {
    const card = document.createElement('div');
    card.className = 'research-card';
    const title = document.createElement('div');
    title.className = 'research-title';
    setText(title, `${record.step || 'step'} · ${formatTime(record.timestamp_secs)}`);
    const summary = document.createElement('div');
    summary.className = 'research-summary';
    setText(summary, record.summary || JSON.stringify(record.details || {}, null, 2));
    const meta = document.createElement('div');
    meta.className = 'research-meta';
    setText(meta, `contract: ${record.contract_id || 'n/a'}`);
    card.appendChild(title);
    card.appendChild(summary);
    card.appendChild(meta);
    root.appendChild(card);
  }
}

function renderSecurityResearch(lines, taskflows) {
  const root = document.getElementById('security-research');
  if (!root) return;
  root.innerHTML = '';
  const records = (lines || []).map(parseJsonLine).filter(Boolean).slice().reverse();
  const flows = Array.isArray(taskflows) ? taskflows : [];
  if (!records.length && !flows.length) {
    root.innerHTML = '<div class="muted">No security research yet.</div>';
    return;
  }
  for (const report of records.slice(0, 4)) {
    const card = document.createElement('div');
    card.className = 'research-card';
    const title = document.createElement('div');
    title.className = 'research-title';
    setText(title, `Audit · ${report.mode || 'mode'} · findings ${report.finding_count ?? 0}`);
    const summary = document.createElement('div');
    summary.className = 'research-summary';
    const findings = (report.findings || []).slice(0, 5).map(finding =>
      `- [${finding.severity}] ${finding.path}:${finding.line} ${finding.vulnerability_class}`
    ).join('\n');
    setText(summary, [
      report.summary || '',
      report.authorization_basis ? 'authorization: ' + report.authorization_basis : '',
      findings ? 'findings:\n' + findings : '',
    ].filter(Boolean).join('\n\n'));
    card.appendChild(title);
    card.appendChild(summary);
    root.appendChild(card);
  }
  if (flows.length) {
    const card = document.createElement('div');
    card.className = 'research-card';
    const title = document.createElement('div');
    title.className = 'research-title';
    setText(title, 'Taskflows');
    const summary = document.createElement('div');
    summary.className = 'research-summary';
    setText(summary, flows.slice(0, 5).map(flow => `- ${flow.taskflow_id}: ${flow.title}`).join('\n'));
    card.appendChild(title);
    card.appendChild(summary);
    root.appendChild(card);
  }
}

function renderProtocolSecurity(report) {
  const root = document.getElementById('protocol-security');
  if (!root) return;
  root.innerHTML = '';
  if (!report) {
    root.innerHTML = '<div class="muted">No protocol security report yet.</div>';
    return;
  }
  const card = document.createElement('div');
  card.className = 'research-card';
  const title = document.createElement('div');
  title.className = 'research-title';
  setText(title, `Invariant report · pass ${report.pass_count ?? 0} · warn ${report.warning_count ?? 0} · fail ${report.fail_count ?? 0}`);
  const summary = document.createElement('div');
  summary.className = 'research-summary';
  const findings = (report.findings || []).slice(0, 8).map(finding =>
    `- [${finding.status}] ${finding.invariant_id}: ${finding.evidence}`
  ).join('\n');
  setText(summary, findings || 'No findings.');
  card.appendChild(title);
  card.appendChild(summary);
  root.appendChild(card);
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
      record.source_count != null ? 'sources: ' + record.source_count : '',
      record.source_bundle_path ? 'source bundle: ' + record.source_bundle_path : '',
      record.output_path ? 'report: ' + record.output_path : ''
    ].filter(Boolean);
    setText(meta, parts.join(' · ') || 'No report generated.');

    card.appendChild(head);
    card.appendChild(summary);
    card.appendChild(meta);
    root.appendChild(card);
  }
}

function renderPaperQa(lines) {
  const root = document.getElementById('paperqa');
  if (!root) return;
  root.innerHTML = '';
  const records = (lines || []).map(parseJsonLine).filter(Boolean).slice().reverse().slice(0, 3);
  if (!records.length) return;
  for (const record of records) {
    const card = document.createElement('div');
    const status = record.status || 'unknown';
    card.className = 'research-card ' + status;
    const head = document.createElement('div');
    head.className = 'research-head';
    const title = document.createElement('div');
    title.className = 'research-title';
    setText(title, `PaperQA · ${record.domain || 'Research'} · ${formatTime(record.timestamp_secs)}`);
    const badge = document.createElement('div');
    badge.className = 'badge ' + status;
    setText(badge, status);
    head.appendChild(title);
    head.appendChild(badge);
    const summary = document.createElement('div');
    summary.className = 'research-summary';
    setText(summary, [
      record.question ? 'question: ' + record.question : '',
      record.stdout_tail ? 'answer tail:\n' + record.stdout_tail : '',
      record.stderr_tail ? 'stderr:\n' + record.stderr_tail : ''
    ].filter(Boolean).join('\n\n') || 'No PaperQA output captured.');
    const meta = document.createElement('div');
    meta.className = 'research-meta';
    const parts = [
      record.report_path ? 'report: ' + record.report_path : '',
      record.paper_dir ? 'papers: ' + record.paper_dir : '',
      record.research_quality_score != null ? 'quality: ' + record.research_quality_score : '',
      record.research_claim_status ? 'claim: ' + record.research_claim_status : ''
    ].filter(Boolean);
    setText(meta, parts.join(' · ') || 'No report generated.');
    card.appendChild(head);
    card.appendChild(summary);
    card.appendChild(meta);
    root.appendChild(card);
  }
}

function renderPaperBrief(lines) {
  const root = document.getElementById('paper-brief');
  if (!root) return;
  root.innerHTML = '';
  const records = (lines || []).map(parseJsonLine).filter(Boolean).slice().reverse().slice(0, 3);
  if (!records.length) return;
  for (const record of records) {
    const card = document.createElement('div');
    const status = record.status || 'unknown';
    card.className = 'research-card ' + status;
    const head = document.createElement('div');
    head.className = 'research-head';
    const title = document.createElement('div');
    title.className = 'research-title';
    setText(title, `Paper Brief · ${record.domain || 'Research'} · ${formatTime(record.timestamp_secs)}`);
    const badge = document.createElement('div');
    badge.className = 'badge ' + status;
    setText(badge, status);
    head.appendChild(title);
    head.appendChild(badge);
    const papers = (record.selected_papers || []).slice(0, 3).map(paper =>
      `- ${paper.title || paper.source_id}\n  sha256: ${(paper.sha256 || '').slice(0, 16)} · text: ${paper.text_chars || 0} chars`
    ).join('\n');
    const summary = document.createElement('div');
    summary.className = 'research-summary';
    setText(summary, [
      record.question ? 'question: ' + record.question : '',
      record.summary ? 'brief:\n' + record.summary : '',
      papers ? 'papers:\n' + papers : '',
      record.error ? 'error: ' + record.error : ''
    ].filter(Boolean).join('\n\n') || 'No paper brief details.');
    const meta = document.createElement('div');
    meta.className = 'research-meta';
    const parts = [
      record.brief_path ? 'brief: ' + record.brief_path : '',
      record.model ? 'model: ' + record.model : '',
      record.provider ? 'provider: ' + record.provider : '',
      record.total_tokens ? 'tokens: ' + record.total_tokens : '',
      record.research_quality_score != null ? 'quality: ' + record.research_quality_score : '',
      record.research_claim_status ? 'claim: ' + record.research_claim_status : ''
    ].filter(Boolean);
    setText(meta, parts.join(' · ') || 'No brief generated.');
    card.appendChild(head);
    card.appendChild(summary);
    card.appendChild(meta);
    root.appendChild(card);
  }
}

function renderPaperAcquisition(lines) {
  const root = document.getElementById('paper-acquisition');
  if (!root) return;
  root.innerHTML = '';
  const records = (lines || []).map(parseJsonLine).filter(Boolean).slice().reverse().slice(0, 3);
  if (!records.length) return;
  for (const record of records) {
    const card = document.createElement('div');
    const status = record.status || 'unknown';
    card.className = 'research-card ' + status;
    const head = document.createElement('div');
    head.className = 'research-head';
    const title = document.createElement('div');
    title.className = 'research-title';
    setText(title, `Open Papers · ${record.domain || 'Research'} · ${formatTime(record.timestamp_secs)}`);
    const badge = document.createElement('div');
    badge.className = 'badge ' + status;
    setText(badge, status);
    head.appendChild(title);
    head.appendChild(badge);
    const downloaded = (record.downloaded || []).slice(0, 5).map(paper =>
      `- ${paper.title || paper.source_id} (${paper.bytes || 0} bytes)\n  ${paper.file_path || ''}`
    ).join('\n');
    const skipped = (record.skipped || []).slice(0, 5).map(item =>
      `- ${item.title || item.source_id}: ${item.reason || 'skipped'}`
    ).join('\n');
    const errors = (record.errors || []).slice(0, 3).map(item => '- ' + item).join('\n');
    const summary = document.createElement('div');
    summary.className = 'research-summary';
    setText(summary, [
      record.question ? 'question: ' + record.question : '',
      downloaded ? 'downloaded:\n' + downloaded : '',
      skipped ? 'skipped:\n' + skipped : '',
      errors ? 'errors:\n' + errors : ''
    ].filter(Boolean).join('\n\n') || 'No acquisition details.');
    const meta = document.createElement('div');
    meta.className = 'research-meta';
    setText(meta, [
      record.source_bundle_path ? 'source bundle: ' + record.source_bundle_path : '',
      record.manifest_path ? 'manifest: ' + record.manifest_path : ''
    ].filter(Boolean).join(' · '));
    card.appendChild(head);
    card.appendChild(summary);
    card.appendChild(meta);
    root.appendChild(card);
  }
}

function renderSourceFetch(lines) {
  const root = document.getElementById('source-fetch');
  if (!root) return;
  root.innerHTML = '';
  const records = (lines || []).map(parseJsonLine).filter(Boolean).slice().reverse().slice(0, 3);
  if (!records.length) return;
  for (const record of records) {
    const card = document.createElement('div');
    card.className = 'research-card ok';
    const title = document.createElement('div');
    title.className = 'research-title';
    setText(title, `Sources · ${record.domain || 'Research'} · ${(record.sources || []).length} fetched`);
    const summary = document.createElement('div');
    summary.className = 'research-summary';
    const sources = (record.sources || []).slice(0, 5).map(src => {
      const year = src.year ? ` (${src.year})` : '';
      const provider = src.provider || 'source';
      return `- [${provider}] ${src.title || 'Untitled'}${year}`;
    }).join('\n');
    const errors = (record.errors || []).map(err => `- ${err.provider}: ${err.message}`).join('\n');
    setText(summary, [
      record.query ? 'query: ' + record.query : '',
      sources ? 'fetched:\n' + sources : '',
      errors ? 'errors:\n' + errors : ''
    ].filter(Boolean).join('\n\n'));
    card.appendChild(title);
    card.appendChild(summary);
    root.appendChild(card);
  }
}

function renderResearchLedger(lines) {
  const root = document.getElementById('research-ledger');
  if (!root) return;
  root.innerHTML = '';
  const records = (lines || []).map(parseJsonLine).filter(Boolean).slice().reverse().slice(0, 3);
  if (!records.length) return;
  for (const record of records) {
    const card = document.createElement('div');
    card.className = 'research-card';
    const title = document.createElement('div');
    title.className = 'research-title';
    setText(title, `Ledger · ${record.domain || 'Research'} · ${record.task_id || 'ad-hoc'}`);
    const summary = document.createElement('div');
    summary.className = 'research-summary';
    const followUps = (record.follow_up_questions || []).map(item => '- ' + item).join('\n');
    const leads = (record.verification_leads || []).slice(0, 4).map(item => '- ' + item).join('\n');
    setText(summary, [
      record.question ? 'question: ' + record.question : '',
      followUps ? 'follow-up questions:\n' + followUps : '',
      leads ? 'verification leads:\n' + leads : ''
    ].filter(Boolean).join('\n\n'));
    card.appendChild(title);
    card.appendChild(summary);
    root.appendChild(card);
  }
}

function renderResearchQueue(queue) {
  const root = document.getElementById('research-queue');
  if (!root) return;
  root.innerHTML = '';
  const tasks = ((queue && queue.tasks) || [])
    .filter(task => task.status === 'Pending')
    .sort((a, b) => (b.priority || 0) - (a.priority || 0))
    .slice(0, 4);
  if (!tasks.length) {
    root.innerHTML = '<div class="muted">No pending research tasks.</div>';
    return;
  }
  for (const task of tasks) {
    const card = document.createElement('div');
    card.className = 'queue-card';
    const title = document.createElement('div');
    title.className = 'queue-title';
    setText(title, task.question || task.task_id || 'Research task');
    const meta = document.createElement('div');
    meta.className = 'queue-meta';
    setText(meta, `${task.domain || 'Unknown'} · priority ${task.priority ?? 'n/a'} · ${task.task_id || ''}`);
    card.appendChild(title);
    card.appendChild(meta);
    root.appendChild(card);
  }
}

function renderResearchDigest(digest) {
  const root = document.getElementById('research-digest');
  if (!root) return;
  root.innerHTML = '';
  if (!digest) {
    root.innerHTML = '<div class="muted">No research digest yet.</div>';
    return;
  }
  const grid = document.createElement('div');
  grid.className = 'digest-grid';
  const stats = [
    ['Reports', digest.report_count ?? 0],
    ['Completed', digest.completed_entries ?? 0],
    ['Sources', digest.fetched_source_count ?? 0],
    ['Pending', digest.pending_task_count ?? 0],
  ];
  for (const [label, value] of stats) {
    const stat = document.createElement('div');
    stat.className = 'digest-stat';
    stat.innerHTML = `<strong>${value}</strong><span>${label}</span>`;
    grid.appendChild(stat);
  }
  root.appendChild(grid);

  const domains = (digest.domain_summaries || []).slice(0, 6).map(domain =>
    `- ${domain.domain}: completed=${domain.completed_entries}, pending=${domain.pending_tasks}, sources=${domain.fetched_source_count}`
  ).join('\n');
  const recent = (digest.recent_reports || []).slice(0, 4).map(report =>
    `- [${report.status}] ${report.domain}: ${report.question}`
  ).join('\n');
  const questions = (digest.open_questions || []).slice(0, 5).map(q => '- ' + q).join('\n');

  const card = document.createElement('div');
  card.className = 'research-card';
  const title = document.createElement('div');
  title.className = 'research-title';
  setText(title, 'Digest');
  const summary = document.createElement('div');
  summary.className = 'research-summary';
  setText(summary, [
    domains ? 'domain progress:\n' + domains : '',
    recent ? 'recent reports:\n' + recent : '',
    questions ? 'open questions:\n' + questions : ''
  ].filter(Boolean).join('\n\n'));
  card.appendChild(title);
  card.appendChild(summary);
  root.appendChild(card);
}

function renderSkillReviewPanel(registry) {
  const root = document.getElementById('skill-review-panel');
  if (!root) return;
  for (const input of document.querySelectorAll('.skill-review-checkbox')) {
    if (input.checked) pendingSkillSelections.add(input.value);
    else pendingSkillSelections.delete(input.value);
  }
  root.innerHTML = '';
  const pending = ((registry && registry.entries) || [])
    .filter(entry => entry.scope === 'Installed' && entry.status === 'NeedsReview')
    .sort((a, b) => (b.governance_level || 0) - (a.governance_level || 0) || String(a.name).localeCompare(String(b.name)));
  const pendingNames = new Set(pending.map(entry => entry.name));
  for (const name of Array.from(pendingSkillSelections)) {
    if (!pendingNames.has(name)) pendingSkillSelections.delete(name);
  }
  if (!pending.length) {
    const empty = document.createElement('div');
    empty.className = 'muted side-note';
    setText(empty, 'No pending skill reviews.');
    root.appendChild(empty);
    return;
  }

  const intro = document.createElement('div');
  intro.className = 'muted side-note';
  setText(intro, `${pending.length} skill(s) waiting for explicit owner review. 勾选具体项目后再激活。`);
  root.appendChild(intro);

  const list = document.createElement('div');
  list.className = 'research-list';
  for (const entry of pending) {
    const card = document.createElement('div');
    card.className = 'research-card review-card' + ((entry.governance_level || 0) >= 4 ? ' high-risk' : '');
    const row = document.createElement('label');
    row.className = 'review-row';
    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.className = 'skill-review-checkbox';
    checkbox.value = entry.name || '';
    checkbox.checked = pendingSkillSelections.has(entry.name || '');
    checkbox.onchange = () => {
      if (checkbox.checked) pendingSkillSelections.add(checkbox.value);
      else pendingSkillSelections.delete(checkbox.value);
    };
    const content = document.createElement('div');

    const title = document.createElement('div');
    title.className = 'research-title';
    setText(title, `${entry.name || 'skill'} · ${entry.status || 'status'} · L${entry.governance_level ?? 'n/a'} ${entry.risk_level || ''}`);
    const summary = document.createElement('div');
    summary.className = 'research-summary';
    const triggers = (entry.triggers || []).slice(0, 2).map(item => '- ' + item).join('\n');
    setText(summary, [
      entry.description || '',
      triggers ? 'triggers:\n' + triggers : '',
    ].filter(Boolean).join('\n\n'));
    const meta = document.createElement('div');
    meta.className = 'research-meta review-source';
    setText(meta, `source: ${entry.source || 'unknown'}\npath: ${entry.path || 'unknown'}`);

    content.appendChild(title);
    content.appendChild(summary);
    content.appendChild(meta);
    row.appendChild(checkbox);
    row.appendChild(content);
    card.appendChild(row);
    list.appendChild(card);
  }
  root.appendChild(list);

  const bar = document.createElement('div');
  bar.className = 'review-bar';
  const approve = document.createElement('button');
  approve.id = 'approve-skills-button';
  approve.textContent = 'Activate Selected';
  approve.onclick = approveSelectedSkills;
  const selectAll = document.createElement('button');
  selectAll.className = 'secondary';
  selectAll.textContent = 'Select All';
  selectAll.onclick = () => {
    for (const input of document.querySelectorAll('.skill-review-checkbox')) {
      input.checked = true;
      pendingSkillSelections.add(input.value);
    }
  };
  const clear = document.createElement('button');
  clear.className = 'secondary';
  clear.textContent = 'Clear';
  clear.onclick = () => {
    for (const input of document.querySelectorAll('.skill-review-checkbox')) {
      input.checked = false;
      pendingSkillSelections.delete(input.value);
    }
  };
  const status = document.createElement('span');
  status.id = 'skill-review-status';
  status.className = 'review-status';
  bar.appendChild(approve);
  bar.appendChild(selectAll);
  bar.appendChild(clear);
  bar.appendChild(status);
  root.appendChild(bar);
}

async function approveSelectedSkills() {
  const button = document.getElementById('approve-skills-button');
  const status = document.getElementById('skill-review-status');
  for (const input of document.querySelectorAll('.skill-review-checkbox')) {
    if (input.checked) pendingSkillSelections.add(input.value);
    else pendingSkillSelections.delete(input.value);
  }
  const names = Array.from(pendingSkillSelections).filter(Boolean);
  if (!names.length) {
    if (status) status.textContent = 'No skill selected.';
    return;
  }
  if (button) button.disabled = true;
  if (status) status.textContent = `activating ${names.length} selected skill(s)...`;
  try {
    const res = await fetch('/api/skills/approve', {
      method: 'POST',
      headers: {'Content-Type': 'application/json', 'X-Buster-Console': '1', 'X-Buster-Owner-Token': OWNER_TOKEN},
      body: JSON.stringify({names})
    });
    const data = await res.json();
    if (status) {
      status.textContent = `approved ${data.approved?.length || 0}; failed ${data.failed?.length || 0}`;
    }
    for (const name of data.approved || []) pendingSkillSelections.delete(name);
    await refresh();
  } catch (_) {
    if (status) status.textContent = 'approval failed';
  } finally {
    if (button) button.disabled = false;
  }
}

function renderSkills(registry, events) {
  const root = document.getElementById('skills');
  const eventRoot = document.getElementById('skill-events');
  if (!root || !eventRoot) return;
  maybeAutoShowSkillReviews(registry);
  renderSkillReviewPanel(registry);
  root.innerHTML = '';
  const entries = ((registry && registry.entries) || [])
    .slice()
    .sort((a, b) => (b.use_count || 0) - (a.use_count || 0) || String(a.name).localeCompare(String(b.name)));
  if (!entries.length) {
    root.innerHTML = '<div class="muted">No skills registered yet.</div>';
  } else {
    for (const entry of entries.slice(0, 8)) {
      const card = document.createElement('div');
      card.className = 'research-card';
      const title = document.createElement('div');
      title.className = 'research-title';
      setText(title, `${entry.name || 'skill'} · ${entry.scope || 'scope'} · ${entry.status || 'status'}`);
      const summary = document.createElement('div');
      summary.className = 'research-summary';
      const triggers = (entry.triggers || []).slice(0, 3).map(item => '- ' + item).join('\n');
      const tools = (entry.tools || []).slice(0, 4).map(item => '- ' + item).join('\n');
      setText(summary, [
        entry.description || '',
        triggers ? 'triggers:\n' + triggers : '',
        tools ? 'tools:\n' + tools : '',
      ].filter(Boolean).join('\n\n'));
      const meta = document.createElement('div');
      meta.className = 'research-meta';
      setText(meta, `risk: ${entry.risk_level || 'unknown'} · level: ${entry.governance_level ?? 'n/a'} · used: ${entry.use_count || 0} · viewed: ${entry.view_count || 0}`);
      card.appendChild(title);
      card.appendChild(summary);
      card.appendChild(meta);
      root.appendChild(card);
    }
  }
  eventRoot.textContent = (events || []).slice(-8).join('\n') || 'No skill events yet.';
}

function renderTools(registry, events) {
  const root = document.getElementById('tools');
  const eventRoot = document.getElementById('tool-events');
  if (!root || !eventRoot) return;
  root.innerHTML = '';
  const entries = ((registry && registry.entries) || [])
    .slice()
    .sort((a, b) =>
      String(a.status || '').localeCompare(String(b.status || '')) ||
      String(a.name).localeCompare(String(b.name))
    );
  if (!entries.length) {
    root.innerHTML = '<div class="muted">No tools registered yet.</div>';
  } else {
    for (const entry of entries.slice(0, 10)) {
      const card = document.createElement('div');
      card.className = 'research-card';
      const title = document.createElement('div');
      title.className = 'research-title';
      setText(title, `${entry.name || 'tool'} · ${entry.status || 'status'} · ${entry.required_permission || 'permission'}`);
      const summary = document.createElement('div');
      summary.className = 'research-summary';
      const hosts = (entry.required_network_hosts || []).slice(0, 5).map(item => '- ' + item).join('\n');
      const secrets = (entry.required_secrets || []).slice(0, 5).map(item => '- ' + item).join('\n');
      const actions = (entry.action_rules || []).slice(0, 5).map(rule => `- ${rule.action} (${rule.required_permission})`).join('\n');
      setText(summary, [
        entry.description || '',
        hosts ? 'network:\n' + hosts : '',
        secrets ? 'secrets:\n' + secrets : '',
        actions ? 'actions:\n' + actions : '',
        entry.quarantined_reason ? 'quarantine: ' + entry.quarantined_reason : '',
      ].filter(Boolean).join('\n\n'));
      const meta = document.createElement('div');
      meta.className = 'research-meta';
      setText(meta, `runtime: ${entry.runtime_kind || 'n/a'} · capability: ${entry.capability || 'n/a'} · risk: ${entry.risk_line || 'n/a'} · used: ${entry.use_count || 0} · failures: ${entry.failure_count || 0}`);
      card.appendChild(title);
      card.appendChild(summary);
      card.appendChild(meta);
      root.appendChild(card);
    }
  }
  eventRoot.textContent = (events || []).slice(-8).join('\n') || 'No tool events yet.';
}

async function refresh() {
  try {
    const res = await fetch('/api/status');
    const data = await res.json();
    document.getElementById('state').textContent = data.state || 'No state yet.';
    renderTicks(data.runtime_tail || []);
    renderResearchDigest(data.research_digest);
    renderResearchQueue(data.research_queue);
    renderPaperAcquisition(data.paper_acquisition_tail || []);
    renderPaperBrief(data.paper_brief_tail || []);
    renderPaperQa(data.paperqa_tail || []);
    renderSourceFetch(data.source_fetch_tail || []);
    renderResearch(data.research_tail || []);
    renderResearchLedger(data.research_ledger_tail || []);
    renderHarness(data.harness_contract_tail || [], data.harness_outcome_tail || []);
    renderAgentSteps(data.chat_agent_steps_tail || []);
    renderSecurityResearch(data.security_research_tail || [], data.security_taskflows || []);
    renderProtocolSecurity(data.protocol_security);
    renderSkills(data.skill_registry, data.skill_events_tail || []);
    renderTools(data.tool_registry, data.tool_events_tail || []);
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
    const res = await fetch('/api/tick', { method: 'POST', headers: {'X-Buster-Console': '1', 'X-Buster-Owner-Token': OWNER_TOKEN} });
    const data = await res.json();
    const statusRes = await fetch('/api/status');
    const statusData = await statusRes.json();
    document.getElementById('state').textContent = statusData.state || 'No state yet.';
    renderTicks(statusData.runtime_tail || [], data);
    renderResearchDigest(statusData.research_digest);
    renderResearchQueue(statusData.research_queue);
    renderPaperAcquisition(statusData.paper_acquisition_tail || []);
    renderPaperBrief(statusData.paper_brief_tail || []);
    renderPaperQa(statusData.paperqa_tail || []);
    renderSourceFetch(statusData.source_fetch_tail || []);
    renderResearch(statusData.research_tail || []);
    renderResearchLedger(statusData.research_ledger_tail || []);
    renderHarness(statusData.harness_contract_tail || [], statusData.harness_outcome_tail || []);
    renderAgentSteps(statusData.chat_agent_steps_tail || []);
    renderSecurityResearch(statusData.security_research_tail || [], statusData.security_taskflows || []);
    renderProtocolSecurity(statusData.protocol_security);
    renderSkills(statusData.skill_registry, statusData.skill_events_tail || []);
    renderTools(statusData.tool_registry, statusData.tool_events_tail || []);
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
  shouldStickToChatBottom = true;
  forceNextChatScroll = true;
  message.value = '';
  sendButton.disabled = true;
  chat.innerHTML = '';
  addMessage(chat, 'user', text, '', 'ok');
  addMessage(chat, 'assistant', '正在思考...', '', 'ok');
  chat.scrollTop = chat.scrollHeight;
  try {
    const res = await fetch('/api/chat', { method: 'POST', headers: {'Content-Type': 'application/json', 'X-Buster-Console': '1', 'X-Buster-Owner-Token': OWNER_TOKEN}, body: JSON.stringify({message: text}) });
    const data = await res.json();
    renderChat([JSON.stringify(data)], { forceScroll: true });
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
document.getElementById('chat').addEventListener('scroll', () => {
  const chat = document.getElementById('chat');
  shouldStickToChatBottom = isNearBottom(chat);
});
refresh();
setInterval(refresh, 5000);
</script>
</body>
</html>"#;
