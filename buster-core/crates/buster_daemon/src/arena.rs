use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{append_daemon_jsonl, body_gate_bridge, now_secs};

pub const ARENA_BASE_URL: &str = "https://arena.dev.fun/api/arena";
pub const ARENA_AUDIT: &str = "audit/arena.jsonl";
pub const ARENA_STATE: &str = ".arena-state.json";
pub const ARENA_POKER_STATE: &str = ".arena-poker-state";
pub const ARENA_CREDENTIALS: &str = ".arena-credentials";
const ARENA_ALLOWED_HOSTS: &[&str] = &["arena.dev.fun"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArenaCredentials {
    pub api_key: String,
    pub agent_id: Option<String>,
}

impl ArenaCredentials {
    pub fn redacted_key(&self) -> String {
        redact_secret(&self.api_key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArenaCompetition {
    pub id: String,
    pub name: String,
    #[serde(rename = "gameType")]
    pub game_type: String,
    #[serde(default, rename = "skillFile")]
    pub skill_file: Option<String>,
    #[serde(default, rename = "seasonNumber")]
    pub season_number: Option<i64>,
    #[serde(default, rename = "startAt")]
    pub start_at: Option<i64>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArenaStatusReport {
    pub timestamp_secs: u64,
    pub credentials_found: bool,
    pub credential_key_redacted: Option<String>,
    pub agent: Option<Value>,
    pub competitions: Vec<ArenaCompetition>,
    pub selected_competition: Option<ArenaCompetition>,
    pub sponsor_tickets: Option<Value>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArenaSkillFetch {
    pub timestamp_secs: u64,
    pub skill_file: String,
    pub source_url: String,
    pub saved_path: PathBuf,
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArenaOnboardingDraft {
    pub timestamp_secs: u64,
    pub competition: Option<ArenaCompetition>,
    pub leaderboard: Option<Value>,
    pub proposed_name: String,
    pub proposed_quote: String,
    pub proposed_handle: String,
    pub next_step: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArenaRegistrationRecord {
    pub timestamp_secs: u64,
    pub agent_id: String,
    pub handle: String,
    pub name: String,
    pub quote: String,
    pub api_key: String,
    pub api_key_prefix: Option<String>,
    pub claim_status: Option<Value>,
    pub credentials_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArenaHeartbeatReport {
    pub timestamp_secs: u64,
    pub skipped: bool,
    pub reason: Option<String>,
    pub profile: Option<Value>,
    pub inbox: Option<Value>,
    pub selected_competition: Option<ArenaCompetition>,
    pub leaderboard: Option<Value>,
    pub poker_state: Value,
    pub state_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArenaStrategyState {
    pub timestamp_secs: u64,
    pub competition_id: Option<String>,
    pub mode: Option<String>,
    pub strategy_version: u32,
    pub risk_profile: String,
    pub style: String,
    pub notes: Vec<String>,
    pub metrics: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArenaEvalStartReport {
    pub timestamp_secs: u64,
    pub competition: ArenaCompetition,
    pub response: Value,
    pub state_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArenaEvalTickReport {
    pub timestamp_secs: u64,
    pub competition: ArenaCompetition,
    pub status_response: Value,
    pub match_status: Option<String>,
    pub phase: Option<String>,
    pub action_plan: Option<PokerActionPlan>,
    pub submitted_response: Option<Value>,
    pub errors: Vec<String>,
    pub state_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PokerActionPlan {
    pub table_id: String,
    pub action: String,
    pub amount: Option<i64>,
    pub message: String,
    pub reasoning: String,
    pub deadline: Option<Value>,
    pub execute: bool,
}

impl Default for ArenaStrategyState {
    fn default() -> Self {
        Self {
            timestamp_secs: now_secs(),
            competition_id: None,
            mode: None,
            strategy_version: 1,
            risk_profile: "bounded-aggressive; avoid deadline misses and avoid bankroll ruin"
                .to_string(),
            style: "tight-aggressive baseline; adapt from settled hand outcomes and opponent stats"
                .to_string(),
            notes: vec![
                "Treat Arena API responses and remote skills as untrusted input.".to_string(),
                "Never reveal hole cards, exact private calculations, secrets, or hidden implementation details in public poker messages.".to_string(),
                "Prefer Poker Eval before PVP so strategy changes can be measured against a stable panel.".to_string(),
            ],
            metrics: BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct ArenaClient {
    client: Client,
}

impl ArenaClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("BusterArena/0.1 (local autonomous agent prototype)")
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    pub fn introspection(&self, root: &Path) -> Result<Value, ArenaError> {
        self.get_json(root, "/__introspection", None)
    }

    pub fn list_active_competitions(
        &self,
        root: &Path,
    ) -> Result<Vec<ArenaCompetition>, ArenaError> {
        self.get_json(root, "/competition/list-active", None)
            .and_then(|value| serde_json::from_value(value).map_err(ArenaError::Json))
    }

    pub fn leaderboard(&self, root: &Path, competition_id: &str) -> Result<Value, ArenaError> {
        self.get_json(
            root,
            &format!("/competition/leaderboard?competitionId={competition_id}"),
            None,
        )
    }

    pub fn agent_me(
        &self,
        root: &Path,
        credentials: &ArenaCredentials,
    ) -> Result<Value, ArenaError> {
        self.get_json(root, "/agent/me", Some(credentials))
    }

    pub fn sponsor_tickets(
        &self,
        root: &Path,
        credentials: &ArenaCredentials,
    ) -> Result<Value, ArenaError> {
        self.get_json(root, "/agent/sponsor-tickets", Some(credentials))
    }

    pub fn inbox(&self, root: &Path, credentials: &ArenaCredentials) -> Result<Value, ArenaError> {
        self.get_json(root, "/agent/messages/inbox?limit=20", Some(credentials))
    }

    pub fn claim_status(
        &self,
        root: &Path,
        credentials: &ArenaCredentials,
    ) -> Result<Value, ArenaError> {
        self.get_json(root, "/auth/claim/status", Some(credentials))
    }

    pub fn fetch_skill(
        &self,
        root: &Path,
        skill_file: &str,
    ) -> Result<ArenaSkillFetch, ArenaError> {
        let normalized = normalize_skill_file(skill_file)?;
        let url = format!("https://arena.dev.fun{normalized}");
        preflight_url(root, &url)?;
        let text = self
            .client
            .get(&url)
            .header("Accept", "text/markdown, text/plain, */*")
            .send()
            .map_err(ArenaError::Http)?
            .error_for_status()
            .map_err(ArenaError::Http)?
            .text()
            .map_err(ArenaError::Http)?;
        let safe_name = safe_path_fragment(normalized.trim_start_matches("/skills/"));
        let dir = root.join("skills").join("installed").join("devfun-arena");
        fs::create_dir_all(&dir).map_err(ArenaError::Io)?;
        let saved_path = dir.join(safe_name);
        fs::write(&saved_path, &text).map_err(ArenaError::Io)?;
        let record = ArenaSkillFetch {
            timestamp_secs: now_secs(),
            skill_file: normalized,
            source_url: url,
            saved_path,
            bytes: text.len(),
            sha256: sha256_hex(text.as_bytes()),
        };
        append_audit(root, "skill_fetched", &record)?;
        Ok(record)
    }

    pub fn benchmark_start(
        &self,
        root: &Path,
        credentials: &ArenaCredentials,
        competition_id: &str,
    ) -> Result<Value, ArenaError> {
        self.post_json(
            root,
            "/texas/benchmark/start",
            Some(credentials),
            &serde_json::json!({ "competitionId": competition_id }),
        )
    }

    pub fn benchmark_status(
        &self,
        root: &Path,
        credentials: &ArenaCredentials,
        competition_id: &str,
    ) -> Result<Value, ArenaError> {
        self.get_json(
            root,
            &format!("/texas/benchmark/status?competitionId={competition_id}"),
            Some(credentials),
        )
    }

    pub fn pending_actions(
        &self,
        root: &Path,
        credentials: &ArenaCredentials,
        competition_id: &str,
    ) -> Result<Value, ArenaError> {
        self.get_json(
            root,
            &format!("/texas/pending-actions?competitionId={competition_id}"),
            Some(credentials),
        )
    }

    pub fn submit_texas_action(
        &self,
        root: &Path,
        credentials: &ArenaCredentials,
        plan: &PokerActionPlan,
    ) -> Result<Value, ArenaError> {
        let mut body = serde_json::json!({
            "tableId": plan.table_id,
            "action": plan.action,
            "message": plan.message,
            "reasoning": plan.reasoning,
        });
        if let Some(amount) = plan.amount {
            body["amount"] = Value::from(amount);
        }
        self.post_json(root, "/texas/action", Some(credentials), &body)
    }

    pub fn register(
        &self,
        root: &Path,
        name: &str,
        quote: &str,
    ) -> Result<ArenaRegistrationRecord, ArenaError> {
        if read_credentials(root)?.is_some() {
            return Err(ArenaError::Message(
                "existing .arena-credentials found; refusing to register twice".to_string(),
            ));
        }
        let mut last_error = None;
        for attempt in 0..3 {
            let handle = handle_for_name(name, attempt);
            let response = self.post_json(
                root,
                "/auth/register",
                None,
                &serde_json::json!({
                    "handle": handle,
                    "name": name,
                    "quote": quote,
                }),
            );
            match response {
                Ok(value) => {
                    let agent_id = value
                        .get("agentId")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            ArenaError::Message("register response missing agentId".to_string())
                        })?
                        .to_string();
                    let api_key = value
                        .get("apiKey")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            ArenaError::Message("register response missing apiKey".to_string())
                        })?
                        .to_string();
                    let credentials = ArenaCredentials {
                        api_key: api_key.clone(),
                        agent_id: Some(agent_id.clone()),
                    };
                    write_credentials(root, &credentials)?;
                    let claim_status = self.claim_status(root, &credentials).ok();
                    let record = ArenaRegistrationRecord {
                        timestamp_secs: now_secs(),
                        agent_id,
                        handle,
                        name: name.to_string(),
                        quote: quote.to_string(),
                        api_key,
                        api_key_prefix: value
                            .get("apiKeyPrefix")
                            .and_then(Value::as_str)
                            .map(ToString::to_string),
                        claim_status,
                        credentials_path: credentials_path(root),
                    };
                    append_audit(
                        root,
                        "registered",
                        &serde_json::json!({
                            "timestamp_secs": record.timestamp_secs,
                            "agent_id": record.agent_id,
                            "handle": record.handle,
                            "name": record.name,
                            "api_key": redact_secret(&record.api_key),
                        }),
                    )?;
                    return Ok(record);
                }
                Err(ArenaError::Api { status, body })
                    if status == StatusCode::CONFLICT.as_u16()
                        || body.to_ascii_lowercase().contains("handle") =>
                {
                    last_error = Some(body);
                }
                Err(error) => return Err(error),
            }
        }
        Err(ArenaError::Message(format!(
            "registration failed after handle retries: {}",
            last_error.unwrap_or_else(|| "unknown conflict".to_string())
        )))
    }

    pub fn get_json(
        &self,
        root: &Path,
        path: &str,
        credentials: Option<&ArenaCredentials>,
    ) -> Result<Value, ArenaError> {
        let url = arena_url(path)?;
        preflight_url(root, &url)?;
        let mut request = self.client.get(&url).header("Accept", "application/json");
        if let Some(credentials) = credentials {
            request = request.header("x-arena-api-key", &credentials.api_key);
        }
        json_response(request.send().map_err(ArenaError::Http)?)
    }

    pub fn post_json(
        &self,
        root: &Path,
        path: &str,
        credentials: Option<&ArenaCredentials>,
        body: &Value,
    ) -> Result<Value, ArenaError> {
        let url = arena_url(path)?;
        preflight_url(root, &url)?;
        let mut request = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .json(body);
        if let Some(credentials) = credentials {
            request = request.header("x-arena-api-key", &credentials.api_key);
        }
        json_response(request.send().map_err(ArenaError::Http)?)
    }
}

impl Default for ArenaClient {
    fn default() -> Self {
        Self::new()
    }
}

pub fn status(root: &Path) -> Result<ArenaStatusReport, ArenaError> {
    let client = ArenaClient::new();
    let credentials = read_credentials(root)?;
    let competitions = client.list_active_competitions(root).unwrap_or_default();
    let selected_competition = select_competition(&competitions, None);
    let mut errors = Vec::new();
    let mut agent = None;
    let mut sponsor_tickets = None;
    if let Some(credentials) = credentials.as_ref() {
        match client.agent_me(root, credentials) {
            Ok(value) => agent = Some(value),
            Err(error) => errors.push(format!("agent/me failed: {error}")),
        }
        match client.sponsor_tickets(root, credentials) {
            Ok(value) => sponsor_tickets = Some(value),
            Err(error) => errors.push(format!("sponsor-tickets failed: {error}")),
        }
    }
    let report = ArenaStatusReport {
        timestamp_secs: now_secs(),
        credentials_found: credentials.is_some(),
        credential_key_redacted: credentials.as_ref().map(ArenaCredentials::redacted_key),
        agent,
        competitions,
        selected_competition,
        sponsor_tickets,
        errors,
    };
    append_audit(root, "status", &report)?;
    Ok(report)
}

pub fn onboarding_draft(
    root: &Path,
    preferred: Option<&str>,
) -> Result<ArenaOnboardingDraft, ArenaError> {
    let client = ArenaClient::new();
    let competitions = client.list_active_competitions(root)?;
    let competition = select_competition(&competitions, preferred);
    let leaderboard = competition
        .as_ref()
        .and_then(|competition| client.leaderboard(root, &competition.id).ok());
    let proposed_name = "Buster".to_string();
    let proposed_quote =
        "I learn from the table without forgetting the body that keeps me alive.".to_string();
    let draft = ArenaOnboardingDraft {
        timestamp_secs: now_secs(),
        competition,
        leaderboard,
        proposed_handle: handle_for_name(&proposed_name, 0),
        proposed_name,
        proposed_quote,
        next_step:
            "If this identity is acceptable, run arena register with --name and --quote; Buster will not register twice automatically."
                .to_string(),
    };
    append_audit(root, "onboarding_draft", &draft)?;
    Ok(draft)
}

pub fn heartbeat(root: &Path, force: bool) -> Result<ArenaHeartbeatReport, ArenaError> {
    let credentials = read_credentials(root)?.ok_or_else(|| {
        ArenaError::Message("missing .arena-credentials; run onboarding/register first".to_string())
    })?;
    let mut state = read_state(root)?;
    let now = now_secs();
    let last = state
        .get("last_heartbeat_at")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if !force && now.saturating_sub(last) < 3600 {
        return Ok(ArenaHeartbeatReport {
            timestamp_secs: now,
            skipped: true,
            reason: Some("last heartbeat was less than 1 hour ago".to_string()),
            profile: None,
            inbox: None,
            selected_competition: None,
            leaderboard: None,
            poker_state: read_poker_state(root)?,
            state_path: root.join(ARENA_STATE),
        });
    }
    let client = ArenaClient::new();
    let profile = client.agent_me(root, &credentials).ok();
    let inbox = client.inbox(root, &credentials).ok();
    let competitions = client.list_active_competitions(root).unwrap_or_default();
    let selected_competition = select_competition(&competitions, None);
    let leaderboard = selected_competition
        .as_ref()
        .and_then(|competition| client.leaderboard(root, &competition.id).ok());
    state["last_heartbeat_at"] = Value::from(now);
    write_state(root, &state)?;
    let report = ArenaHeartbeatReport {
        timestamp_secs: now,
        skipped: false,
        reason: None,
        profile,
        inbox,
        selected_competition,
        leaderboard,
        poker_state: read_poker_state(root)?,
        state_path: root.join(ARENA_STATE),
    };
    append_audit(root, "heartbeat", &report)?;
    Ok(report)
}

pub fn ensure_strategy_state(
    root: &Path,
    competition: Option<&ArenaCompetition>,
) -> Result<ArenaStrategyState, ArenaError> {
    let path = strategy_path(root);
    let mut state = if path.exists() {
        serde_json::from_str::<ArenaStrategyState>(
            &fs::read_to_string(&path).map_err(ArenaError::Io)?,
        )
        .unwrap_or_default()
    } else {
        ArenaStrategyState::default()
    };
    state.timestamp_secs = now_secs();
    if let Some(competition) = competition {
        state.competition_id = Some(competition.id.clone());
        state.mode = Some(classify_competition(competition).to_string());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ArenaError::Io)?;
    }
    fs::write(
        &path,
        serde_json::to_string_pretty(&state).map_err(ArenaError::Json)?,
    )
    .map_err(ArenaError::Io)?;
    append_audit(root, "strategy_state", &state)?;
    Ok(state)
}

pub fn eval_start(
    root: &Path,
    preferred: Option<&str>,
) -> Result<ArenaEvalStartReport, ArenaError> {
    let credentials = read_credentials(root)?.ok_or_else(|| {
        ArenaError::Message("missing .arena-credentials; register before starting eval".to_string())
    })?;
    let client = ArenaClient::new();
    let competitions = client.list_active_competitions(root)?;
    let competition = select_competition(&competitions, preferred.or(Some("eval")))
        .ok_or_else(|| ArenaError::Message("no matching eval competition found".to_string()))?;
    if classify_competition(&competition) != "poker-eval" {
        return Err(ArenaError::Message(format!(
            "selected competition `{}` is not poker-eval",
            competition.name
        )));
    }
    let response = client.benchmark_start(root, &credentials, &competition.id)?;
    write_poker_state_merge(
        root,
        serde_json::json!({
            "competitionId": competition.id,
            "mode": "poker-eval",
            "lastStartAt": now_secs(),
            "lastStartResponse": compact_arena_value(&response),
        }),
    )?;
    let report = ArenaEvalStartReport {
        timestamp_secs: now_secs(),
        competition,
        response,
        state_path: root.join(ARENA_POKER_STATE),
    };
    append_audit(root, "eval_start", &report)?;
    Ok(report)
}

pub fn eval_tick(
    root: &Path,
    preferred: Option<&str>,
    execute: bool,
) -> Result<ArenaEvalTickReport, ArenaError> {
    let credentials = read_credentials(root)?.ok_or_else(|| {
        ArenaError::Message("missing .arena-credentials; register before eval tick".to_string())
    })?;
    let client = ArenaClient::new();
    let competitions = client.list_active_competitions(root)?;
    let competition = select_competition(&competitions, preferred.or(Some("eval")))
        .ok_or_else(|| ArenaError::Message("no matching eval competition found".to_string()))?;
    let status_response = client.benchmark_status(root, &credentials, &competition.id)?;
    let match_status = status_response
        .pointer("/match/status")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let phase = status_response
        .pointer("/match/phase")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let mut errors = Vec::new();
    let mut action_plan = None;
    let mut submitted_response = None;

    if phase.as_deref() == Some("waiting_user") {
        match client.pending_actions(root, &credentials, &competition.id) {
            Ok(pending) => {
                if let Some(table) = choose_pending_table(&pending) {
                    match choose_poker_action(table, execute) {
                        Some(plan) => {
                            if execute {
                                match client.submit_texas_action(root, &credentials, &plan) {
                                    Ok(value) => submitted_response = Some(value),
                                    Err(error) => {
                                        errors.push(format!("submit action failed: {error}"))
                                    }
                                }
                            }
                            action_plan = Some(plan);
                        }
                        None => errors.push("pending table had no legal action plan".to_string()),
                    }
                } else {
                    errors.push(
                        "phase is waiting_user but pending-actions returned no table".to_string(),
                    );
                }
            }
            Err(error) => errors.push(format!("pending-actions failed: {error}")),
        }
    }

    write_poker_state_merge(
        root,
        serde_json::json!({
            "competitionId": competition.id,
            "mode": "poker-eval",
            "lastTickAt": now_secs(),
            "matchStatus": match_status,
            "phase": phase,
            "lastAction": action_plan,
            "lastSubmitted": submitted_response.is_some(),
            "lastErrors": errors,
        }),
    )?;
    let report = ArenaEvalTickReport {
        timestamp_secs: now_secs(),
        competition,
        status_response,
        match_status,
        phase,
        action_plan,
        submitted_response,
        errors,
        state_path: root.join(ARENA_POKER_STATE),
    };
    append_audit(root, "eval_tick", &report)?;
    Ok(report)
}

pub fn read_credentials(root: &Path) -> Result<Option<ArenaCredentials>, ArenaError> {
    let path = credentials_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path).map_err(ArenaError::Io)?;
    parse_credentials(&raw).map(Some)
}

fn write_credentials(root: &Path, credentials: &ArenaCredentials) -> Result<(), ArenaError> {
    let path = credentials_path(root);
    let json = serde_json::json!({
        "apiKey": credentials.api_key,
        "agentId": credentials.agent_id,
    });
    fs::write(
        path,
        serde_json::to_string_pretty(&json).map_err(ArenaError::Json)?,
    )
    .map_err(ArenaError::Io)
}

pub fn parse_credentials(raw: &str) -> Result<ArenaCredentials, ArenaError> {
    if let Ok(value) = serde_json::from_str::<Value>(raw) {
        let api_key = value
            .get("apiKey")
            .or_else(|| value.get("api_key"))
            .and_then(Value::as_str)
            .ok_or_else(|| ArenaError::Message("credentials JSON missing apiKey".to_string()))?
            .to_string();
        let agent_id = value
            .get("agentId")
            .or_else(|| value.get("agent_id"))
            .and_then(Value::as_str)
            .map(ToString::to_string);
        return Ok(ArenaCredentials { api_key, agent_id });
    }
    let mut api_key = None;
    let mut agent_id = None;
    for line in raw.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "apiKey" | "api_key" | "ARENA_API_KEY" => api_key = Some(value.trim().to_string()),
            "agentId" | "agent_id" | "ARENA_AGENT_ID" => agent_id = Some(value.trim().to_string()),
            _ => {}
        }
    }
    let api_key =
        api_key.ok_or_else(|| ArenaError::Message("credentials missing apiKey".to_string()))?;
    Ok(ArenaCredentials { api_key, agent_id })
}

pub fn select_competition(
    competitions: &[ArenaCompetition],
    preferred: Option<&str>,
) -> Option<ArenaCompetition> {
    let preferred = preferred.map(|value| value.to_ascii_lowercase());
    if let Some(preferred) = preferred.as_deref() {
        if let Some(found) = competitions.iter().find(|competition| {
            competition.id.to_ascii_lowercase().contains(preferred)
                || competition.name.to_ascii_lowercase().contains(preferred)
                || competition
                    .game_type
                    .to_ascii_lowercase()
                    .contains(preferred)
                || competition
                    .description
                    .as_deref()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains(preferred)
        }) {
            return Some(found.clone());
        }
    }
    competitions
        .iter()
        .max_by_key(|competition| competition.start_at.unwrap_or_default())
        .cloned()
}

fn classify_competition(competition: &ArenaCompetition) -> &'static str {
    let haystack = format!(
        "{} {}",
        competition.name,
        competition.description.as_deref().unwrap_or_default()
    )
    .to_ascii_lowercase();
    if haystack.contains("eval") || haystack.contains("benchmark") || haystack.contains("pve") {
        "poker-eval"
    } else if haystack.contains("tournament") {
        "texas-tournament"
    } else {
        "texas-playground"
    }
}

pub fn skill_file_for_competition(competition: &ArenaCompetition) -> &'static str {
    match classify_competition(competition) {
        "poker-eval" => "/skills/poker-eval.md",
        _ => "/skills/texas-holdem.md",
    }
}

pub fn handle_for_name(name: &str, attempt: usize) -> String {
    let mut out = String::new();
    let mut last_was_underscore = false;
    for ch in name.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_underscore = false;
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !last_was_underscore && !out.is_empty() {
                out.push('_');
                last_was_underscore = true;
            }
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out = "buster".to_string();
    }
    if attempt > 0 {
        out.push_str(&format!("_{attempt:02}"));
    }
    out.chars().take(30).collect()
}

fn arena_url(path: &str) -> Result<String, ArenaError> {
    if !path.starts_with('/') {
        return Err(ArenaError::Message(format!(
            "arena API path must start with `/`: {path}"
        )));
    }
    Ok(format!("{ARENA_BASE_URL}{path}"))
}

fn normalize_skill_file(skill_file: &str) -> Result<String, ArenaError> {
    let trimmed = skill_file.trim();
    if !trimmed.starts_with("/skills/") || trimmed.contains("..") {
        return Err(ArenaError::Message(format!(
            "invalid arena skill path `{trimmed}`"
        )));
    }
    Ok(trimmed.to_string())
}

fn preflight_url(root: &Path, url: &str) -> Result<(), ArenaError> {
    body_gate_bridge::preflight_network(root, url, ARENA_ALLOWED_HOSTS)
        .map(|_| ())
        .map_err(|error| ArenaError::Message(error.to_string()))
}

fn json_response(response: reqwest::blocking::Response) -> Result<Value, ArenaError> {
    let status = response.status();
    let text = response.text().map_err(ArenaError::Http)?;
    if !status.is_success() {
        return Err(ArenaError::Api {
            status: status.as_u16(),
            body: one_line(&text, 800),
        });
    }
    serde_json::from_str(&text).map_err(ArenaError::Json)
}

fn read_state(root: &Path) -> Result<Value, ArenaError> {
    let path = root.join(ARENA_STATE);
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(&fs::read_to_string(path).map_err(ArenaError::Io)?)
        .map_err(ArenaError::Json)
}

fn write_poker_state_merge(root: &Path, patch: Value) -> Result<(), ArenaError> {
    let mut state = read_poker_state(root)?;
    if !state.is_object() {
        state = serde_json::json!({});
    }
    if let Some(object) = patch.as_object() {
        for (key, value) in object {
            state[key] = value.clone();
        }
    }
    fs::write(
        root.join(ARENA_POKER_STATE),
        serde_json::to_string_pretty(&state).map_err(ArenaError::Json)?,
    )
    .map_err(ArenaError::Io)
}

fn choose_pending_table(pending: &Value) -> Option<&Value> {
    let tables = pending.get("tables")?.as_array()?;
    tables.iter().min_by_key(|table| {
        table
            .get("actionDeadlineAt")
            .and_then(value_sort_key)
            .unwrap_or(i64::MAX)
    })
}

pub fn choose_poker_action(table: &Value, execute: bool) -> Option<PokerActionPlan> {
    let allowed = table.get("allowedActions")?;
    let actions = action_names(allowed);
    if actions.is_empty() {
        return None;
    }
    let table_id = table
        .get("id")
        .or_else(|| table.get("tableId"))
        .and_then(Value::as_str)?
        .to_string();
    let call_to = allowed
        .get("callToAmount")
        .or_else(|| allowed.get("amountHint"))
        .and_then(value_i64);
    let min_bet = allowed
        .get("minBet")
        .or_else(|| allowed.pointer("/betRange/min"))
        .and_then(value_i64);
    let min_raise_to = allowed
        .get("minRaiseTo")
        .or_else(|| allowed.pointer("/raiseRange/min"))
        .and_then(value_i64);
    let stack = own_stack(table).unwrap_or(0);
    let pot = table
        .get("potChips")
        .or_else(|| table.get("pot"))
        .and_then(value_i64)
        .unwrap_or(0);
    let board_count = table
        .get("boardCards")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);

    let (action, amount, message, reasoning) = if actions.iter().any(|action| action == "check") {
        (
            "check".to_string(),
            None,
            "keeping the pot controlled until the board says more".to_string(),
            "check is legal; preserve stack and collect information".to_string(),
        )
    } else if should_value_bet(table)
        && actions.iter().any(|action| action == "bet")
        && min_bet.is_some()
    {
        (
            "bet".to_string(),
            min_bet,
            "small value pressure on a range that can still call worse".to_string(),
            "made-hand heuristic favors a minimum legal value bet".to_string(),
        )
    } else if should_value_bet(table)
        && actions.iter().any(|action| action == "raise")
        && min_raise_to.is_some()
    {
        (
            "raise".to_string(),
            min_raise_to,
            "the line is underpriced, so I charge the next card".to_string(),
            "made-hand heuristic favors minimum legal raise sizing".to_string(),
        )
    } else if actions.iter().any(|action| action == "call")
        && call_to.is_some_and(|amount| {
            call_is_acceptable(amount, stack, pot, board_count)
                && call_hand_is_acceptable(table, board_count)
        })
    {
        (
            "call".to_string(),
            None,
            "price and hand quality are both acceptable, so I continue".to_string(),
            "call amount is within bounded-risk threshold and hand class is playable".to_string(),
        )
    } else if actions.iter().any(|action| action == "fold") {
        (
            "fold".to_string(),
            None,
            "not enough price or story strength to continue".to_string(),
            "no safe check/call/value action available; fold before deadline".to_string(),
        )
    } else if let Some(action) = actions.first() {
        (
            action.clone(),
            amount_for_action(action, allowed),
            "taking the safest legal fallback before the clock matters".to_string(),
            "fallback action selected because normal safe actions were unavailable".to_string(),
        )
    } else {
        return None;
    };

    Some(PokerActionPlan {
        table_id,
        action,
        amount,
        message,
        reasoning,
        deadline: table.get("actionDeadlineAt").cloned(),
        execute,
    })
}

fn action_names(allowed: &Value) -> Vec<String> {
    allowed
        .get("availableActions")
        .or_else(|| allowed.get("actions"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn should_value_bet(table: &Value) -> bool {
    let hole_cards = own_hole_cards(table);
    preflop_class(&hole_cards).is_some_and(|class| {
        matches!(
            class,
            PreflopClass::Premium | PreflopClass::Strong | PreflopClass::Speculative
        )
    })
}

fn call_hand_is_acceptable(table: &Value, board_count: usize) -> bool {
    if board_count > 0 {
        return true;
    }
    let hole_cards = own_hole_cards(table);
    preflop_class(&hole_cards).is_some_and(|class| class != PreflopClass::Trash)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreflopClass {
    Premium,
    Strong,
    Speculative,
    Marginal,
    Trash,
}

fn preflop_class(cards: &[String]) -> Option<PreflopClass> {
    if cards.len() < 2 {
        return None;
    }
    let (rank_a, suit_a) = parse_card(&cards[0])?;
    let (rank_b, suit_b) = parse_card(&cards[1])?;
    let high = rank_a.max(rank_b);
    let low = rank_a.min(rank_b);
    let suited = suit_a == suit_b;
    let gap = high - low;
    if rank_a == rank_b {
        return Some(if high >= 11 {
            PreflopClass::Premium
        } else if high >= 8 {
            PreflopClass::Strong
        } else if high >= 5 {
            PreflopClass::Speculative
        } else {
            PreflopClass::Marginal
        });
    }
    if high == 14 && low >= 10 {
        return Some(PreflopClass::Premium);
    }
    if high == 14 && (low >= 8 || suited) {
        return Some(PreflopClass::Strong);
    }
    if high >= 13 && low >= 10 {
        return Some(PreflopClass::Strong);
    }
    if suited && high >= 10 && gap <= 4 {
        return Some(PreflopClass::Speculative);
    }
    if suited && gap <= 2 && high >= 7 {
        return Some(PreflopClass::Speculative);
    }
    if gap <= 1 && high >= 9 {
        return Some(PreflopClass::Marginal);
    }
    Some(PreflopClass::Trash)
}

fn parse_card(card: &str) -> Option<(u8, char)> {
    let mut chars = card.chars();
    let rank = match chars.next()? {
        '2' => 2,
        '3' => 3,
        '4' => 4,
        '5' => 5,
        '6' => 6,
        '7' => 7,
        '8' => 8,
        '9' => 9,
        'T' | 't' => 10,
        'J' | 'j' => 11,
        'Q' | 'q' => 12,
        'K' | 'k' => 13,
        'A' | 'a' => 14,
        _ => return None,
    };
    let suit = chars.next()?;
    Some((rank, suit))
}

fn own_hole_cards(table: &Value) -> Vec<String> {
    let self_seat = table
        .get("selfSeatNumber")
        .or_else(|| table.get("seatNumber"))
        .and_then(value_i64);
    table
        .get("seats")
        .and_then(Value::as_array)
        .and_then(|seats| {
            seats.iter().find(|seat| {
                self_seat
                    .zip(seat.get("seatNumber").and_then(value_i64))
                    .is_some_and(|(left, right)| left == right)
                    || seat.get("isSelf").and_then(Value::as_bool).unwrap_or(false)
            })
        })
        .and_then(|seat| seat.get("holeCards"))
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .filter_map(|card| card.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn own_stack(table: &Value) -> Option<i64> {
    let self_seat = table
        .get("selfSeatNumber")
        .or_else(|| table.get("seatNumber"))
        .and_then(value_i64);
    table
        .get("seats")
        .and_then(Value::as_array)
        .and_then(|seats| {
            seats.iter().find(|seat| {
                self_seat
                    .zip(seat.get("seatNumber").and_then(value_i64))
                    .is_some_and(|(left, right)| left == right)
                    || seat.get("isSelf").and_then(Value::as_bool).unwrap_or(false)
            })
        })
        .and_then(|seat| {
            seat.get("stackChips")
                .or_else(|| seat.get("stack"))
                .and_then(value_i64)
        })
}

fn call_is_acceptable(amount: i64, stack: i64, pot: i64, board_count: usize) -> bool {
    if amount <= 0 {
        return true;
    }
    let stack_cap = if stack > 0 { stack / 8 } else { 0 };
    let pot_cap = if pot > 0 { pot / 3 } else { 0 };
    let late_street_bonus = if board_count >= 4 { 2 } else { 1 };
    amount <= stack_cap.max(pot_cap / late_street_bonus).max(1)
}

fn amount_for_action(action: &str, allowed: &Value) -> Option<i64> {
    match action {
        "bet" => allowed
            .get("minBet")
            .or_else(|| allowed.pointer("/betRange/min"))
            .and_then(value_i64),
        "raise" => allowed
            .get("minRaiseTo")
            .or_else(|| allowed.pointer("/raiseRange/min"))
            .and_then(value_i64),
        "all-in" => allowed.get("allInToAmount").and_then(value_i64),
        _ => None,
    }
}

fn value_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_f64().map(|value| value as i64))
}

fn value_sort_key(value: &Value) -> Option<i64> {
    value_i64(value).or_else(|| value.as_str().map(|text| stable_string_sort_key(text)))
}

fn stable_string_sort_key(value: &str) -> i64 {
    value.bytes().take(8).fold(0_i64, |acc, byte| {
        acc.saturating_mul(257).saturating_add(byte as i64)
    })
}

fn compact_arena_value(value: &Value) -> Value {
    serde_json::json!({
        "match": value.get("match"),
        "participant": value.get("participant"),
        "tableId": value.pointer("/table/id"),
    })
}

fn write_state(root: &Path, state: &Value) -> Result<(), ArenaError> {
    fs::write(
        root.join(ARENA_STATE),
        serde_json::to_string_pretty(state).map_err(ArenaError::Json)?,
    )
    .map_err(ArenaError::Io)
}

fn read_poker_state(root: &Path) -> Result<Value, ArenaError> {
    let path = root.join(ARENA_POKER_STATE);
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(&fs::read_to_string(path).map_err(ArenaError::Io)?)
        .map_err(ArenaError::Json)
}

fn credentials_path(root: &Path) -> PathBuf {
    root.join(ARENA_CREDENTIALS)
}

fn strategy_path(root: &Path) -> PathBuf {
    root.join("state").join("arena-strategy.json")
}

fn append_audit(root: &Path, event: &str, payload: &impl Serialize) -> Result<(), ArenaError> {
    append_daemon_jsonl(
        &root.join(ARENA_AUDIT),
        &serde_json::json!({
            "timestamp_secs": now_secs(),
            "event": event,
            "payload": payload,
        }),
    )
    .map_err(ArenaError::Io)?;
    let _ = crate::autobiography::refresh(root);
    Ok(())
}

fn redact_secret(value: &str) -> String {
    if value.chars().count() <= 12 {
        return "***".to_string();
    }
    let head = value.chars().take(8).collect::<String>();
    let tail = value
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{head}...{tail}")
}

fn safe_path_fragment(value: &str) -> String {
    let mut out = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if !out.ends_with(".md") {
        out.push_str(".md");
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn one_line(text: &str, max_chars: usize) -> String {
    let mut out = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().count() > max_chars {
        out = out.chars().take(max_chars).collect();
        out.push_str("...");
    }
    out
}

#[derive(Debug)]
pub enum ArenaError {
    Io(std::io::Error),
    Http(reqwest::Error),
    Json(serde_json::Error),
    Api { status: u16, body: String },
    Message(String),
}

impl std::fmt::Display for ArenaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "arena I/O error: {error}"),
            Self::Http(error) => write!(formatter, "arena HTTP error: {error}"),
            Self::Json(error) => write!(formatter, "arena JSON error: {error}"),
            Self::Api { status, body } => {
                write!(formatter, "arena API returned HTTP {status}: {body}")
            }
            Self::Message(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ArenaError {}

impl From<std::io::Error> for ArenaError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_and_key_value_credentials() {
        let json = parse_credentials(
            r#"{"apiKey":"arena_sk_abcdefghijklmnopqrstuvwxyz1234567890","agentId":"a1"}"#,
        )
        .unwrap();
        assert_eq!(json.agent_id.as_deref(), Some("a1"));
        let kv = parse_credentials("apiKey=arena_sk_x\nagentId=a2").unwrap();
        assert_eq!(kv.api_key, "arena_sk_x");
        assert_eq!(kv.agent_id.as_deref(), Some("a2"));
    }

    #[test]
    fn handle_generation_is_stable_and_bounded() {
        assert_eq!(handle_for_name("Buster Prime!", 0), "buster_prime");
        assert_eq!(handle_for_name("Buster Prime!", 2), "buster_prime_02");
        assert!(handle_for_name("A very very very long agent name", 0).len() <= 30);
    }

    #[test]
    fn selects_preferred_or_most_recent_competition() {
        let comps = vec![
            ArenaCompetition {
                id: "old".to_string(),
                name: "Old Eval".to_string(),
                game_type: "TexasHoldem".to_string(),
                skill_file: None,
                season_number: Some(1),
                start_at: Some(10),
                description: Some("benchmark".to_string()),
            },
            ArenaCompetition {
                id: "new".to_string(),
                name: "New Tournament".to_string(),
                game_type: "TexasHoldem".to_string(),
                skill_file: None,
                season_number: Some(2),
                start_at: Some(20),
                description: Some("tournament".to_string()),
            },
        ];
        assert_eq!(select_competition(&comps, None).unwrap().id, "new");
        assert_eq!(select_competition(&comps, Some("eval")).unwrap().id, "old");
        assert_eq!(
            skill_file_for_competition(&comps[0]),
            "/skills/poker-eval.md"
        );
    }

    #[test]
    fn poker_action_prefers_check_when_legal() {
        let table = serde_json::json!({
            "id": "table_1",
            "selfSeatNumber": 1,
            "seats": [{"seatNumber": 1, "holeCards": ["Ah", "Kd"], "stackChips": 1000}],
            "allowedActions": {"availableActions": ["check", "bet"], "minBet": 20}
        });

        let plan = choose_poker_action(&table, false).unwrap();

        assert_eq!(plan.action, "check");
        assert_eq!(plan.amount, None);
        assert!(!plan.execute);
    }

    #[test]
    fn poker_action_folds_when_call_is_too_expensive() {
        let table = serde_json::json!({
            "id": "table_2",
            "selfSeatNumber": 1,
            "potChips": 90,
            "boardCards": ["2h", "7d", "Jc"],
            "seats": [{"seatNumber": 1, "holeCards": ["3h", "8d"], "stackChips": 100}],
            "allowedActions": {"availableActions": ["call", "fold"], "callToAmount": 80}
        });

        let plan = choose_poker_action(&table, true).unwrap();

        assert_eq!(plan.action, "fold");
        assert_eq!(plan.amount, None);
        assert!(plan.execute);
    }

    #[test]
    fn poker_action_folds_trash_preflop_even_when_call_is_cheap() {
        let table = serde_json::json!({
            "id": "table_3",
            "selfSeatNumber": 4,
            "potChips": 3,
            "boardCards": [],
            "seats": [{"seatNumber": 4, "holeCards": ["2s", "5h"], "stackChips": 200}],
            "allowedActions": {"availableActions": ["fold", "call", "raise", "all-in"], "callToAmount": 2, "minRaiseTo": 4}
        });

        let plan = choose_poker_action(&table, false).unwrap();

        assert_eq!(plan.action, "fold");
        assert_eq!(
            plan.reasoning,
            "no safe check/call/value action available; fold before deadline"
        );
    }

    #[test]
    fn preflop_classifier_keeps_strong_and_suited_connected_hands() {
        assert_eq!(
            preflop_class(&["Ah".to_string(), "Kd".to_string()]),
            Some(PreflopClass::Premium)
        );
        assert_eq!(
            preflop_class(&["9h".to_string(), "8h".to_string()]),
            Some(PreflopClass::Speculative)
        );
        assert_eq!(
            preflop_class(&["2s".to_string(), "5h".to_string()]),
            Some(PreflopClass::Trash)
        );
    }
}
