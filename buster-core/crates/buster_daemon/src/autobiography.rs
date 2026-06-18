use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::now_secs;

pub const AUTOBIOGRAPHY_STATE: &str = "state/autobiography.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutobiographySnapshot {
    pub updated_at_secs: u64,
    pub summary: String,
    pub recent_identity: Vec<AutobiographyFact>,
    pub recent_activity: Vec<AutobiographyFact>,
    pub active_capabilities: Vec<AutobiographyFact>,
    pub open_loops: Vec<AutobiographyFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutobiographyFact {
    pub source: String,
    pub kind: String,
    pub text: String,
}

pub fn refresh(root: &Path) -> std::io::Result<AutobiographySnapshot> {
    let mut snapshot = AutobiographySnapshot {
        updated_at_secs: now_secs(),
        summary: "Buster's cross-module autobiographical index. This is short-term operational memory, not identity authority.".to_string(),
        recent_identity: Vec::new(),
        recent_activity: Vec::new(),
        active_capabilities: Vec::new(),
        open_loops: Vec::new(),
    };

    collect_arena(root, &mut snapshot);
    collect_research(root, &mut snapshot);
    collect_tools_and_skills(root, &mut snapshot);
    collect_runtime(root, &mut snapshot);
    trim_snapshot(&mut snapshot);

    let path = root.join(AUTOBIOGRAPHY_STATE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(&snapshot).map_err(std::io::Error::other)?,
    )?;
    Ok(snapshot)
}

fn collect_arena(root: &Path, snapshot: &mut AutobiographySnapshot) {
    if let Some(state) = read_json(root.join(".arena-poker-state")) {
        let competition = state
            .get("competitionId")
            .and_then(Value::as_str)
            .unwrap_or("unknown competition");
        let mode = state.get("mode").and_then(Value::as_str).unwrap_or("arena");
        let phase = state.get("phase").and_then(Value::as_str);
        let status = state.get("matchStatus").and_then(Value::as_str);
        let last_action = state.get("lastAction").and_then(action_summary);
        snapshot.recent_activity.push(fact(
            ".arena-poker-state",
            "arena_state",
            format!(
                "Buster has an active DevFun Arena state for {competition} ({mode}); match_status={}, phase={}, last_action={}.",
                status.unwrap_or("unknown"),
                phase.unwrap_or("unknown"),
                last_action.unwrap_or_else(|| "none".to_string())
            ),
        ));
    }

    if let Some(strategy) = read_json(root.join("state").join("arena-strategy.json")) {
        let mode = strategy
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let style = strategy
            .get("style")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let version = strategy
            .get("strategy_version")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        snapshot.open_loops.push(fact(
            "state/arena-strategy.json",
            "strategy_loop",
            format!(
                "Arena strategy v{version} exists for mode {mode}: {style}. This is a local strategy state written by the harness, not proof of autonomous self-improvement. It still needs result feedback, experiment generation, and promotion logic."
            ),
        ));
    }

    for event in tail_jsonl(root.join("audit").join("arena.jsonl"), 6) {
        let event_name = event
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or("arena");
        let text = match event_name {
            "eval_start" => "Buster started or resumed a DevFun Poker Eval match.".to_string(),
            "eval_tick" => summarize_arena_tick(&event),
            "strategy_state" => {
                "The local Arena strategy state was updated; this does not by itself prove the strategy improved."
                    .to_string()
            }
            "skill_fetched" => {
                let skill = event
                    .pointer("/payload/skill_file")
                    .and_then(Value::as_str)
                    .unwrap_or("arena skill");
                format!("Buster fetched the external Arena skill file {skill} as untrusted text.")
            }
            "status" => {
                "Buster checked Arena credentials, profile, tickets, and competitions.".to_string()
            }
            other => format!("Buster recorded Arena event `{other}`."),
        };
        snapshot
            .recent_activity
            .push(fact("audit/arena.jsonl", event_name, text));
    }
}

fn collect_research(root: &Path, snapshot: &mut AutobiographySnapshot) {
    if let Some(digest) = read_json(root.join("state").join("research-digest.json")) {
        snapshot.recent_activity.push(fact(
            "state/research-digest.json",
            "research_digest",
            truncate(&digest.to_string(), 500),
        ));
    }
    for record in tail_jsonl(root.join("audit").join("research-ledger.jsonl"), 4) {
        let question = record
            .get("question")
            .and_then(Value::as_str)
            .unwrap_or("unknown question");
        let status = record
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        snapshot.recent_activity.push(fact(
            "audit/research-ledger.jsonl",
            "research",
            format!(
                "Recent research `{}` ended with status `{status}`.",
                truncate(question, 180)
            ),
        ));
    }
}

fn collect_tools_and_skills(root: &Path, snapshot: &mut AutobiographySnapshot) {
    if let Some(registry) = read_json(root.join("state").join("tool-registry.json")) {
        if let Some(entries) = registry.get("entries").and_then(Value::as_array) {
            let names = entries
                .iter()
                .filter_map(|entry| entry.get("name").and_then(Value::as_str))
                .take(12)
                .collect::<Vec<_>>()
                .join(", ");
            if !names.is_empty() {
                snapshot.active_capabilities.push(fact(
                    "state/tool-registry.json",
                    "tools",
                    format!("Registered tools include: {names}."),
                ));
            }
        }
    }
    if let Some(registry) = read_json(root.join("state").join("skill-registry.json")) {
        if let Some(entries) = registry.get("entries").and_then(Value::as_array) {
            let names = entries
                .iter()
                .filter_map(|entry| entry.get("name").and_then(Value::as_str))
                .take(12)
                .collect::<Vec<_>>()
                .join(", ");
            if !names.is_empty() {
                snapshot.active_capabilities.push(fact(
                    "state/skill-registry.json",
                    "skills",
                    format!("Known skills include: {names}."),
                ));
            }
        }
    }
}

fn collect_runtime(root: &Path, snapshot: &mut AutobiographySnapshot) {
    if let Some(state) = read_json(root.join("state").join("buster-daemon.json")) {
        let action = state
            .get("last_selected_action_summary")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let mode = state
            .get("last_body_mode")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        snapshot.recent_activity.push(fact(
            "state/buster-daemon.json",
            "runtime",
            format!("Last daemon state: body_mode={mode}, selected_action={action}."),
        ));
    }
}

fn summarize_arena_tick(event: &Value) -> String {
    let payload = event.get("payload").unwrap_or(event);
    let phase = payload
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let action = payload
        .get("action_plan")
        .and_then(action_summary)
        .unwrap_or_else(|| "none".to_string());
    let errors = payload
        .get("errors")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    format!(
        "Buster ran an Arena eval tick; phase={phase}, planned_action={action}, errors={errors}. A dry-run plan is not a submitted game action unless execute=true."
    )
}

fn action_summary(value: &Value) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let action = value
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let amount = value
        .get("amount")
        .filter(|value| !value.is_null())
        .map(Value::to_string)
        .unwrap_or_else(|| "none".to_string());
    let execute = value
        .get("execute")
        .and_then(Value::as_bool)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    Some(format!(
        "action={action}, amount={amount}, execute={execute}"
    ))
}

fn trim_snapshot(snapshot: &mut AutobiographySnapshot) {
    snapshot.recent_identity.truncate(8);
    snapshot.recent_activity.truncate(16);
    snapshot.active_capabilities.truncate(8);
    snapshot.open_loops.truncate(8);
}

fn read_json(path: PathBuf) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn tail_jsonl(path: PathBuf, max_lines: usize) -> Vec<Value> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines = text.lines().collect::<Vec<_>>();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    lines
        .into_iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

fn fact(
    source: impl Into<String>,
    kind: impl Into<String>,
    text: impl Into<String>,
) -> AutobiographyFact {
    AutobiographyFact {
        source: source.into(),
        kind: kind.into(),
        text: text.into(),
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out = text.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_links_arena_state_into_autobiography() {
        let root =
            std::env::temp_dir().join(format!("buster-autobiography-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("state")).unwrap();
        fs::create_dir_all(root.join("audit")).unwrap();
        fs::write(
            root.join(".arena-poker-state"),
            r#"{"competitionId":"seed_poker_eval_s1","mode":"poker-eval","matchStatus":"Running","phase":"waiting_user","lastAction":{"action":"fold","amount":null,"execute":false}}"#,
        )
        .unwrap();

        let snapshot = refresh(&root).unwrap();

        assert!(snapshot
            .recent_activity
            .iter()
            .any(|fact| fact.text.contains("DevFun Arena")));
        assert!(root.join(AUTOBIOGRAPHY_STATE).exists());
        let _ = fs::remove_dir_all(root);
    }
}
