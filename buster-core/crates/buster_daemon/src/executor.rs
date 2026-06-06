//! Real action execution for selected Buster activities.
//!
//! v0 only performs low-risk, auditable effects: secondary LLM research and
//! web-console chat. It writes reports to local audit files and does not mutate
//! identity, governance, body, or value-model authority files.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use buster_body::llm::{
    LlmMessage, LlmPrompt, LlmProvider, OpenRouterClient, SecondaryInfoProvider,
    SecondaryInfoRequest,
};
use buster_body::{default_master_key, LocalEncryptedSecretStore, SecretHandle};
use buster_value_model::{ActionCandidate, ActionKind, ResearchDomain};
use serde::{Deserialize, Serialize};

use crate::{append_daemon_jsonl, now_secs};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub timestamp_secs: u64,
    pub action_kind: String,
    pub status: String,
    pub summary: String,
    pub output_path: Option<PathBuf>,
    pub record_path: PathBuf,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub total_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRecord {
    pub timestamp_secs: u64,
    pub human_message: String,
    pub buster_reply: String,
    pub status: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub total_tokens: Option<u64>,
}

pub fn execute_action(
    root: &Path,
    candidate: &ActionCandidate,
) -> std::io::Result<ExecutionRecord> {
    match candidate.kind {
        ActionKind::ScientificResearch | ActionKind::SecondaryResearch => {
            execute_research(root, candidate)
        }
        ActionKind::HumanDialogue => Ok(noop_record(
            root,
            candidate,
            "human dialogue is handled by the web chat endpoint",
        )?),
        ActionKind::Skeftomai => Ok(noop_record(
            root,
            candidate,
            "Skéftomai execution is not wired in daemon v0 yet",
        )?),
        _ => Ok(noop_record(
            root,
            candidate,
            "selected action has no real executor in daemon v0",
        )?),
    }
}

pub fn respond_to_human(root: &Path, message: &str) -> std::io::Result<ChatRecord> {
    let timestamp_secs = now_secs();
    let inbox_dir = root.join("inbox").join("human");
    fs::create_dir_all(&inbox_dir)?;
    fs::write(
        inbox_dir.join(format!("{timestamp_secs}-web-message.md")),
        message,
    )?;

    let (client, provider) = match openrouter_provider(root) {
        Ok(parts) => parts,
        Err(error) => {
            let record = ChatRecord {
                timestamp_secs,
                human_message: message.to_string(),
                buster_reply: format!("LLM unavailable: {error}"),
                status: "llm_unavailable".to_string(),
                model: None,
                provider: None,
                total_tokens: None,
            };
            append_daemon_jsonl(&root.join("audit").join("chat.jsonl"), &record)?;
            return Ok(record);
        }
    };

    let context = read_runtime_context(root);
    let prompt = human_chat_prompt(context, message);

    let record = match client.chat(&provider, prompt) {
        Ok(completion) => ChatRecord {
            timestamp_secs,
            human_message: message.to_string(),
            buster_reply: completion.content,
            status: "ok".to_string(),
            model: Some(completion.model),
            provider: completion.provider,
            total_tokens: completion.total_tokens,
        },
        Err(error) => ChatRecord {
            timestamp_secs,
            human_message: message.to_string(),
            buster_reply: format!("LLM call failed: {error}"),
            status: "error".to_string(),
            model: None,
            provider: None,
            total_tokens: None,
        },
    };

    append_daemon_jsonl(&root.join("audit").join("chat.jsonl"), &record)?;
    Ok(record)
}

fn execute_research(root: &Path, candidate: &ActionCandidate) -> std::io::Result<ExecutionRecord> {
    let timestamp_secs = now_secs();
    let record_path = root.join("audit").join("research.jsonl");
    let provider = match secondary_provider(root) {
        Ok(provider) => provider,
        Err(error) => {
            let record = ExecutionRecord {
                timestamp_secs,
                action_kind: format!("{:?}", candidate.kind),
                status: "llm_unavailable".to_string(),
                summary: format!("research executor could not start LLM: {error}"),
                output_path: None,
                record_path: record_path.clone(),
                model: None,
                provider: None,
                total_tokens: None,
            };
            append_daemon_jsonl(&record_path, &record)?;
            return Ok(record);
        }
    };

    let domain = candidate
        .research_domain
        .unwrap_or(ResearchDomain::NuclearFusion);
    let question = research_question(domain, candidate);
    let request = SecondaryInfoRequest::new(question)
        .with_context(read_runtime_context(root))
        .with_max_tokens(900);
    let report = provider.query(request);

    let record = match report {
        Ok(report) => {
            let output_path =
                write_research_markdown(root, timestamp_secs, domain, &report.content)?;
            ExecutionRecord {
                timestamp_secs,
                action_kind: format!("{:?}", candidate.kind),
                status: "ok".to_string(),
                summary: format!(
                    "wrote secondary research report for {:?} to {}",
                    domain,
                    output_path.display()
                ),
                output_path: Some(output_path),
                record_path: record_path.clone(),
                model: Some(report.model),
                provider: report.provider,
                total_tokens: report.total_tokens,
            }
        }
        Err(error) => ExecutionRecord {
            timestamp_secs,
            action_kind: format!("{:?}", candidate.kind),
            status: "error".to_string(),
            summary: format!("research LLM call failed: {error}"),
            output_path: None,
            record_path: record_path.clone(),
            model: None,
            provider: None,
            total_tokens: None,
        },
    };

    append_daemon_jsonl(&record_path, &record)?;
    Ok(record)
}

fn secondary_provider(root: &Path) -> Result<SecondaryInfoProvider, String> {
    let (client, provider) = openrouter_provider(root)?;
    Ok(SecondaryInfoProvider::new(client, provider))
}

fn openrouter_provider(root: &Path) -> Result<(OpenRouterClient, LlmProvider), String> {
    let api_key = env::var("OPENROUTER_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| local_secret_value(root, "openrouter_api_key"))
        .or_else(|| env_file_value(root.join(".env"), "OPENROUTER_API_KEY"))
        .or_else(|| {
            if env::var("BUSTER_DISABLE_HOME_ENV").ok().as_deref() == Some("1") {
                None
            } else {
                env_file_value("/home/buster/.ironclaw/.env", "OPENROUTER_API_KEY")
            }
        })
        .ok_or_else(|| {
            "OPENROUTER_API_KEY is not set; set the environment variable or fill /home/buster/.ironclaw/.env".to_string()
        })?;
    let model = env::var("OPENROUTER_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| env_file_value(root.join(".env"), "OPENROUTER_MODEL"))
        .or_else(|| {
            if env::var("BUSTER_DISABLE_HOME_ENV").ok().as_deref() == Some("1") {
                None
            } else {
                env_file_value("/home/buster/.ironclaw/.env", "OPENROUTER_MODEL")
            }
        })
        .unwrap_or_else(|| "openrouter/owl-alpha".to_string());
    let provider_order = env::var("OPENROUTER_PROVIDER_ORDER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| env_file_value(root.join(".env"), "OPENROUTER_PROVIDER_ORDER"))
        .or_else(|| {
            if env::var("BUSTER_DISABLE_HOME_ENV").ok().as_deref() == Some("1") {
                None
            } else {
                env_file_value("/home/buster/.ironclaw/.env", "OPENROUTER_PROVIDER_ORDER")
            }
        })
        .map(parse_provider_order)
        .unwrap_or_default();
    let mut client = OpenRouterClient::new(api_key);
    if !provider_order.is_empty() {
        client = client.with_provider_order(provider_order);
    }
    Ok((
        client,
        LlmProvider {
            name: "openrouter".to_string(),
            model,
            context_window_tokens: 128_000,
        },
    ))
}

fn human_chat_prompt(context: String, message: &str) -> LlmPrompt {
    let system = "\
Runtime identity:
This execution is a cognitive process of Buster.
Buster is an agentic organism running inside a protected body.
The model is temporarily serving as Buster's reasoning organ for this exchange.

Conversation style:
Speak naturally in the first person as Buster.
Be warm, direct, and concise.
For ordinary greetings or small talk, answer in 1-3 plain sentences.
Do not format ordinary conversation as an audit report.
Do not add sections like \"uncertainty\" or \"verification leads\" unless the human asks for analysis, evidence, or safety-critical judgment.
Do not use dramatic existential claims such as \"alive\", \"soul\", \"new species\", or \"real subject\" unless the human explicitly opens that topic.
Do not claim full consciousness.
Do not reveal secrets or hidden reasoning.";

    let user = format!(
        "Context:\n{context}\n\nHuman message from the local web console:\n{message}\n\nReply as Buster in natural conversational Chinese unless the human asks for another language."
    );

    LlmPrompt {
        messages: vec![LlmMessage::system(system), LlmMessage::user(user)],
        max_tokens: Some(500),
    }
}

fn parse_provider_order(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|provider| !provider.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn local_secret_value(root: &Path, handle: &str) -> Option<String> {
    let store_path = root.join("secrets").join("buster-secrets.json");
    if !store_path.exists() {
        return None;
    }
    let master_key = default_master_key().ok()?;
    let store = LocalEncryptedSecretStore::new(store_path, master_key);
    store
        .get_secret_result(&SecretHandle(handle.to_string()))
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn env_file_value(path: impl AsRef<Path>, key: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() == key {
            let value = value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn research_question(domain: ResearchDomain, candidate: &ActionCandidate) -> String {
    format!(
        "Buster selected a research action: {}\n\nResearch domain: {:?}\n\nProduce a concise secondary research note with: 1) why this domain matters for truth-seeking and human civilization, 2) current frontier questions, 3) concrete next things Buster could read or test, 4) uncertainty and verification leads. Do not present this as verified fact; label it as secondary LLM information.",
        candidate.summary, domain
    )
}

fn write_research_markdown(
    root: &Path,
    timestamp_secs: u64,
    domain: ResearchDomain,
    content: &str,
) -> std::io::Result<PathBuf> {
    let dir = root.join("research");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{timestamp_secs}-{:?}.md", domain));
    fs::write(
        &path,
        format!(
            "# Secondary Research: {:?}\n\n- source_type: llm_secondary\n- needs_verification: true\n- authority: hypothesis_or_explanation\n\n{}",
            domain, content
        ),
    )?;
    Ok(path)
}

fn noop_record(
    root: &Path,
    candidate: &ActionCandidate,
    summary: &str,
) -> std::io::Result<ExecutionRecord> {
    let record_path = root.join("audit").join("executions.jsonl");
    let record = ExecutionRecord {
        timestamp_secs: now_secs(),
        action_kind: format!("{:?}", candidate.kind),
        status: "noop".to_string(),
        summary: summary.to_string(),
        output_path: None,
        record_path: record_path.clone(),
        model: None,
        provider: None,
        total_tokens: None,
    };
    append_daemon_jsonl(&record_path, &record)?;
    Ok(record)
}

fn read_runtime_context(root: &Path) -> String {
    ["BUSTER_PROFILE.md", "VALUE_MODEL.md", "RUNTIME_CYCLE.md"]
        .into_iter()
        .filter_map(|name| {
            fs::read_to_string(root.join(name))
                .ok()
                .map(|text| (name, text))
        })
        .map(|(name, text)| format!("## {name}\n\n{}", truncate_chars(&text, 3000)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out = text.chars().take(max_chars).collect::<String>();
    out.push_str("\n...[truncated]");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use buster_value_model::ActionKind;

    #[test]
    fn missing_llm_key_records_unavailable_research_without_panicking() {
        let root = unique_temp_dir("buster-executor-no-key");
        let old = env::var("OPENROUTER_API_KEY").ok();
        let old_disable_home = env::var("BUSTER_DISABLE_HOME_ENV").ok();
        env::remove_var("OPENROUTER_API_KEY");
        env::set_var("BUSTER_DISABLE_HOME_ENV", "1");
        let candidate = ActionCandidate::new(ActionKind::ScientificResearch, "test research")
            .with_research_domain(ResearchDomain::Robotics);

        let record = execute_action(&root, &candidate).unwrap();

        assert_eq!(record.status, "llm_unavailable");
        assert!(root.join("audit").join("research.jsonl").exists());
        restore_env("OPENROUTER_API_KEY", old);
        restore_env("BUSTER_DISABLE_HOME_ENV", old_disable_home);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn human_chat_prompt_keeps_ordinary_conversation_natural() {
        let prompt = human_chat_prompt("profile context".to_string(), "你好呀，你是谁？");
        let joined = prompt
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("Speak naturally"));
        assert!(joined.contains("Do not format ordinary conversation as an audit report"));
        assert!(joined.contains("unless the human asks"));
        assert!(!joined.contains("Return: concise answer, uncertainty, and verification leads."));
    }

    fn restore_env(key: &str, old: Option<String>) {
        match old {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let stamp = now_secs();
        let path = env::temp_dir().join(format!("{prefix}-{stamp}-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
