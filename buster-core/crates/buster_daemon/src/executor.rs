//! Real action execution for selected Buster activities.
//!
//! v0 only performs low-risk, auditable effects: secondary LLM research and
//! web-console chat. It writes reports to local audit files and does not mutate
//! identity, governance, body, or value-model authority files.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use buster_body::llm::{
    prompt_for_secondary_info, LlmCompletion, LlmError, LlmMessage, LlmPrompt, LlmProvider,
    OpenRouterClient, SecondaryInfoRequest,
};
use buster_body::runtime::{
    BackendCompletion, BackendFailure, RuntimeBackend, RuntimeKind, RuntimeOutcome, RuntimePolicy,
    RuntimeRequest,
};
use buster_body::{
    default_master_key, CapabilityLease, ChangeLevel, EgressBroker, LocalEncryptedSecretStore,
    NetworkPolicy, ResourceBudget, ResourceUsage, RiskLine, SecretHandle,
};
use buster_skills::{SkillRegistryEntry, SkillScope, SkillStatus, SkillWorkspace};
use buster_tools::{ToolActionResult, ToolWorkspace};
use buster_value_model::{ActionCandidate, ActionKind, ResearchDomain};
use serde::{Deserialize, Serialize};

use crate::{
    append_daemon_jsonl, body_gate_bridge, harness, now_secs, organism,
    paper_acquisition::{self, PaperAcquisitionRequest},
    paper_brief::{self, PaperBriefRequest},
    research::{self, ResearchCompletion},
    research_framework::{self, ResearchQualityInput},
    source_fetch::{self, SourceFetcher},
    web_search::{self, WebSearchClient, WebSearchRequest},
};

const DEFAULT_RESEARCH_MAX_PER_HOUR: usize = 3;
const DEFAULT_RESEARCH_MAX_CONSECUTIVE_LLM_ERRORS: usize = 5;
const DEFAULT_LLM_SYNTHESIS_RETRIES: u32 = 2;
const MAX_CHAT_AGENT_ROUNDS: usize = 2;
const MAX_CHAT_TOOL_CALLS: usize = 4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    #[serde(default)]
    pub contract_id: Option<String>,
    pub timestamp_secs: u64,
    pub action_kind: String,
    pub status: String,
    pub summary: String,
    pub output_path: Option<PathBuf>,
    pub record_path: PathBuf,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub total_tokens: Option<u64>,
    pub source_bundle_path: Option<PathBuf>,
    pub source_count: usize,
    #[serde(default)]
    pub research_quality_score: Option<f32>,
    #[serde(default)]
    pub research_claim_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRecord {
    #[serde(default)]
    pub contract_id: Option<String>,
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

pub fn respond_to_human(
    root: &Path,
    message: &str,
    owner_authorized: bool,
) -> std::io::Result<ChatRecord> {
    let timestamp_secs = now_secs();
    let _ = crate::autobiography::refresh(root);
    let contract = harness::human_chat_contract(timestamp_secs, message);
    let contract_id = contract.contract_id.clone();
    harness::append_contract(root, &contract)?;
    let inbox_dir = root.join("inbox").join("human");
    fs::create_dir_all(&inbox_dir)?;
    let inbox_path = inbox_dir.join(format!("{timestamp_secs}-web-message.md"));
    fs::write(&inbox_path, message)?;

    if let Some(paper_request) = detect_direct_paper_request(message) {
        let record = respond_to_direct_paper_request(
            root,
            timestamp_secs,
            &contract_id,
            message,
            paper_request,
        )?;
        append_daemon_jsonl(&root.join("audit").join("chat.jsonl"), &record)?;
        let mut outcome = harness::HarnessOutcome::new(
            contract_id.clone(),
            if record.status == "ok" {
                harness::HarnessStatus::Completed
            } else {
                harness::HarnessStatus::Partial
            },
            format!(
                "human-requested paper reading ended with status {}",
                record.status
            ),
        )
        .with_output("audit", "audit/chat.jsonl", "chat audit record");
        if record.status != "ok" {
            outcome = outcome.with_error(record.buster_reply.clone());
        }
        harness::append_outcome(root, &outcome)?;
        organism::append_signed_event(
            root,
            "human_dialogue",
            serde_json::to_value(&record).map_err(std::io::Error::other)?,
        )?;
        archive_handled_inbox(root, &inbox_path)?;
        return Ok(record);
    }

    if let Some(skill_request) = detect_skill_install_request(message) {
        let record = respond_to_skill_install_request(
            root,
            timestamp_secs,
            &contract_id,
            message,
            skill_request,
        )?;
        append_daemon_jsonl(&root.join("audit").join("chat.jsonl"), &record)?;
        let mut outcome = harness::HarnessOutcome::new(
            contract_id.clone(),
            if record.status == "ok" {
                harness::HarnessStatus::Completed
            } else {
                harness::HarnessStatus::Partial
            },
            format!(
                "human-requested skill install ended with status {}",
                record.status
            ),
        )
        .with_output("audit", "audit/chat.jsonl", "chat audit record")
        .with_output(
            "skill_registry",
            "state/skill-registry.json",
            "updated skill registry",
        );
        if record.status != "ok" {
            outcome = outcome.with_error(record.buster_reply.clone());
        }
        harness::append_outcome(root, &outcome)?;
        organism::append_signed_event(
            root,
            "human_dialogue",
            serde_json::to_value(&record).map_err(std::io::Error::other)?,
        )?;
        archive_handled_inbox(root, &inbox_path)?;
        return Ok(record);
    }

    if let Some(skill_request) = detect_skill_approval_request(root, message)? {
        let record = respond_to_skill_approval_request(
            root,
            timestamp_secs,
            &contract_id,
            message,
            skill_request,
            owner_authorized,
        )?;
        append_daemon_jsonl(&root.join("audit").join("chat.jsonl"), &record)?;
        let mut outcome = harness::HarnessOutcome::new(
            contract_id.clone(),
            if record.status == "ok" {
                harness::HarnessStatus::Completed
            } else {
                harness::HarnessStatus::Partial
            },
            format!(
                "human-requested skill approval ended with status {}",
                record.status
            ),
        )
        .with_output("audit", "audit/chat.jsonl", "chat audit record")
        .with_output(
            "owner_consent",
            "audit/owner-consent.jsonl",
            "local owner consent audit",
        )
        .with_output(
            "skill_registry",
            "state/skill-registry.json",
            "updated skill registry",
        );
        if record.status != "ok" {
            outcome = outcome.with_error(record.buster_reply.clone());
        }
        harness::append_outcome(root, &outcome)?;
        organism::append_signed_event(
            root,
            "human_dialogue",
            serde_json::to_value(&record).map_err(std::io::Error::other)?,
        )?;
        archive_handled_inbox(root, &inbox_path)?;
        return Ok(record);
    }

    let (client, provider) = match llm_provider(root) {
        Ok(parts) => parts,
        Err(error) => {
            let record = ChatRecord {
                contract_id: Some(contract_id.clone()),
                timestamp_secs,
                human_message: message.to_string(),
                buster_reply: format!("LLM unavailable: {error}"),
                status: "llm_unavailable".to_string(),
                model: None,
                provider: None,
                total_tokens: None,
            };
            append_daemon_jsonl(&root.join("audit").join("chat.jsonl"), &record)?;
            harness::append_outcome(
                root,
                &harness::HarnessOutcome::new(
                    contract_id,
                    harness::HarnessStatus::Deferred,
                    "human chat deferred because the LLM provider was unavailable",
                )
                .with_error(error),
            )?;
            organism::append_signed_event(
                root,
                "human_dialogue",
                serde_json::to_value(&record).map_err(std::io::Error::other)?,
            )?;
            archive_handled_inbox(root, &inbox_path)?;
            return Ok(record);
        }
    };

    let context = read_runtime_context(root);
    let record = match respond_with_agent_loop(
        root,
        timestamp_secs,
        &contract_id,
        message,
        context,
        client.clone(),
        provider.clone(),
    ) {
        Ok(record) => record,
        Err(error) => {
            let prompt = human_chat_prompt(read_runtime_context(root), message);
            match synthesize_with_existing_provider(
                root,
                client,
                provider,
                prompt,
                "llm.human_chat",
                "web console human dialogue fallback",
            ) {
                Ok(completion) => ChatRecord {
                    contract_id: Some(contract_id.clone()),
                    timestamp_secs,
                    human_message: message.to_string(),
                    buster_reply: completion.content,
                    status: "ok".to_string(),
                    model: Some(completion.model),
                    provider: completion.provider,
                    total_tokens: completion.total_tokens,
                },
                Err(llm_error) => ChatRecord {
                    contract_id: Some(contract_id.clone()),
                    timestamp_secs,
                    human_message: message.to_string(),
                    buster_reply: format!(
                        "我尝试进入 agent 工具循环，但失败了：{error}\n随后普通对话也失败：{llm_error}"
                    ),
                    status: "error".to_string(),
                    model: None,
                    provider: None,
                    total_tokens: None,
                },
            }
        }
    };

    append_daemon_jsonl(&root.join("audit").join("chat.jsonl"), &record)?;
    let mut outcome = harness::HarnessOutcome::new(
        contract_id.clone(),
        if record.status == "ok" {
            harness::HarnessStatus::Completed
        } else {
            harness::HarnessStatus::Failed
        },
        format!("human chat ended with status {}", record.status),
    )
    .with_output("audit", "audit/chat.jsonl", "chat audit record");
    if record.status != "ok" {
        outcome = outcome.with_error(record.buster_reply.clone());
    }
    harness::append_outcome(root, &outcome)?;
    organism::append_signed_event(
        root,
        "human_dialogue",
        serde_json::to_value(&record).map_err(std::io::Error::other)?,
    )?;
    archive_handled_inbox(root, &inbox_path)?;
    Ok(record)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirectPaperRequest {
    url: String,
    question: String,
    domain: ResearchDomain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillInstallRequest {
    url: String,
    name: Option<String>,
    activate: bool,
    mode: SkillInstallMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SkillInstallMode {
    SingleMarkdown,
    GithubTree {
        owner: String,
        repo: String,
        branch: String,
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SkillApprovalRequest {
    Approve { name: String },
    Clarify { candidates: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ChatAgentPlan {
    #[serde(default)]
    direct_answer: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ChatToolCall>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ChatToolCall {
    tool: String,
    #[serde(default)]
    input: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ChatToolObservation {
    tool: String,
    status: String,
    summary: String,
    #[serde(default)]
    artifact_path: Option<PathBuf>,
}

fn respond_with_agent_loop(
    root: &Path,
    timestamp_secs: u64,
    contract_id: &str,
    message: &str,
    context: String,
    client: OpenRouterClient,
    provider: LlmProvider,
) -> std::io::Result<ChatRecord> {
    append_chat_agent_step(
        root,
        contract_id,
        "start",
        "agent loop started for local human message",
        None,
    )?;

    let mut observations = Vec::new();
    let mut total_tokens = 0u64;
    let mut model = None;
    let mut provider_name = None;
    let mut last_plan = heuristic_chat_agent_plan(message);

    for round in 0..MAX_CHAT_AGENT_ROUNDS {
        let plan = if let Some(plan) = last_plan.take() {
            plan
        } else {
            let prompt = if observations.is_empty() {
                chat_agent_planner_prompt(root, &context, message)?
            } else {
                chat_agent_followup_prompt(&context, message, &observations)
            };
            let completion = chat_with_body_gate(
                root,
                client.clone(),
                provider.clone(),
                prompt,
                "llm.chat_planner",
                "plan local web-console response with bounded tools",
            )
            .map_err(std::io::Error::other)?;
            total_tokens = total_tokens.saturating_add(completion.total_tokens.unwrap_or(0));
            model.get_or_insert(completion.model.clone());
            provider_name = provider_name.or(completion.provider.clone());
            parse_chat_agent_plan(&completion.content).unwrap_or_else(|error| {
                let direct_answer = if observations.is_empty() {
                    Some(format!(
                        "我没有成功解析自己的工具计划，会改用普通对话回答。解析错误：{error}"
                    ))
                } else {
                    None
                };
                ChatAgentPlan {
                    direct_answer,
                    tool_calls: Vec::new(),
                }
            })
        };

        append_chat_agent_step(
            root,
            contract_id,
            &format!("plan.round.{round}"),
            &format!(
                "planner produced {} tool call(s)",
                plan.tool_calls.len().min(MAX_CHAT_TOOL_CALLS)
            ),
            Some(serde_json::to_value(&plan).map_err(std::io::Error::other)?),
        )?;

        if plan.tool_calls.is_empty() {
            if observations.is_empty() {
                if let Some(answer) = plan
                    .direct_answer
                    .filter(|answer| !answer.trim().is_empty())
                {
                    return Ok(ChatRecord {
                        contract_id: Some(contract_id.to_string()),
                        timestamp_secs,
                        human_message: message.to_string(),
                        buster_reply: answer,
                        status: "ok".to_string(),
                        model,
                        provider: provider_name,
                        total_tokens: Some(total_tokens).filter(|tokens| *tokens > 0),
                    });
                }
            }
            break;
        }

        for call in plan.tool_calls.into_iter().take(MAX_CHAT_TOOL_CALLS) {
            if observations.len() >= MAX_CHAT_TOOL_CALLS {
                break;
            }
            let observation = execute_chat_tool_call(root, &call)?;
            append_chat_agent_step(
                root,
                contract_id,
                &format!("tool.{}", observation.tool),
                &observation.summary,
                Some(serde_json::to_value(&observation).map_err(std::io::Error::other)?),
            )?;
            observations.push(observation);
        }
    }

    if observations.is_empty() {
        let completion = chat_with_body_gate(
            root,
            client,
            provider,
            human_chat_prompt(context, message),
            "llm.human_chat",
            "web console human dialogue",
        )
        .map_err(std::io::Error::other)?;
        return Ok(ChatRecord {
            contract_id: Some(contract_id.to_string()),
            timestamp_secs,
            human_message: message.to_string(),
            buster_reply: completion.content,
            status: "ok".to_string(),
            model: Some(completion.model),
            provider: completion.provider,
            total_tokens: completion.total_tokens,
        });
    }

    let final_prompt = chat_agent_final_prompt(&context, message, &observations);
    let final_completion = chat_with_body_gate(
        root,
        client,
        provider,
        final_prompt,
        "llm.chat_final",
        "synthesize answer from completed Buster tool observations",
    );
    match final_completion {
        Ok(completion) => {
            total_tokens = total_tokens.saturating_add(completion.total_tokens.unwrap_or(0));
            Ok(ChatRecord {
                contract_id: Some(contract_id.to_string()),
                timestamp_secs,
                human_message: message.to_string(),
                buster_reply: completion.content,
                status: "ok".to_string(),
                model: Some(completion.model),
                provider: completion.provider,
                total_tokens: Some(total_tokens).filter(|tokens| *tokens > 0),
            })
        }
        Err(error) => Ok(ChatRecord {
            contract_id: Some(contract_id.to_string()),
            timestamp_secs,
            human_message: message.to_string(),
            buster_reply: format!(
                "我已经执行了工具，但最终语言整理失败：{error}\n\n{}",
                observations_to_answer(&observations)
            ),
            status: "tool_observed_llm_final_failed".to_string(),
            model,
            provider: provider_name,
            total_tokens: Some(total_tokens).filter(|tokens| *tokens > 0),
        }),
    }
}

fn synthesize_with_existing_provider(
    root: &Path,
    client: OpenRouterClient,
    provider: LlmProvider,
    prompt: LlmPrompt,
    capability: &str,
    input_summary: &str,
) -> Result<LlmCompletion, LlmError> {
    chat_with_body_gate(root, client, provider, prompt, capability, input_summary)
}

fn heuristic_chat_agent_plan(message: &str) -> Option<ChatAgentPlan> {
    let lower = message.to_ascii_lowercase();
    let asks_tools = message.contains("工具")
        || lower.contains("tool")
        || lower.contains("tools")
        || lower.contains("plugin");
    let asks_skills =
        message.contains("技能") || lower.contains("skill") || lower.contains("skills");
    let asks_list = message.contains("有哪些")
        || message.contains("有什么")
        || message.contains("列出")
        || lower.contains("list")
        || lower.contains("what can")
        || lower.contains("available");
    if asks_tools && asks_list {
        return Some(ChatAgentPlan {
            direct_answer: None,
            tool_calls: vec![ChatToolCall {
                tool: "tools.query".to_string(),
                input: serde_json::json!({ "query": "" }),
            }],
        });
    }
    if asks_skills && asks_list {
        return Some(ChatAgentPlan {
            direct_answer: None,
            tool_calls: vec![ChatToolCall {
                tool: "skills.query".to_string(),
                input: serde_json::json!({ "query": "" }),
            }],
        });
    }
    if lower.contains("arena") || message.contains("比赛") {
        return Some(ChatAgentPlan {
            direct_answer: None,
            tool_calls: vec![ChatToolCall {
                tool: "arena.status".to_string(),
                input: serde_json::json!({}),
            }],
        });
    }
    if message.contains("mp.weixin.qq.com") {
        if let Some(url) = extract_https_url(message) {
            return Some(ChatAgentPlan {
                direct_answer: None,
                tool_calls: vec![ChatToolCall {
                    tool: "skills.wechat_article_scraper".to_string(),
                    input: serde_json::json!({ "url": url }),
                }],
            });
        }
    }
    if lower.contains("search")
        || lower.contains("duckduckgo")
        || message.contains("联网搜索")
        || message.contains("上网查")
        || message.contains("查一下")
        || message.contains("搜一下")
    {
        return Some(ChatAgentPlan {
            direct_answer: None,
            tool_calls: vec![ChatToolCall {
                tool: "web.search".to_string(),
                input: serde_json::json!({ "query": cleanup_query_text(message), "count": 5 }),
            }],
        });
    }
    if message.contains("论文") || lower.contains("paper") || lower.contains("arxiv") {
        if message.contains("下载") || message.contains("阅读") || message.contains("读") {
            return Some(ChatAgentPlan {
                direct_answer: None,
                tool_calls: vec![
                    ChatToolCall {
                        tool: "paper.acquire".to_string(),
                        input: serde_json::json!({ "query": cleanup_query_text(message), "limit": 2 }),
                    },
                    ChatToolCall {
                        tool: "paper.brief".to_string(),
                        input: serde_json::json!({ "question": cleanup_query_text(message), "max_papers": 2 }),
                    },
                ],
            });
        }
        return Some(ChatAgentPlan {
            direct_answer: None,
            tool_calls: vec![ChatToolCall {
                tool: "research.sources".to_string(),
                input: serde_json::json!({ "query": cleanup_query_text(message) }),
            }],
        });
    }
    None
}

fn chat_agent_planner_prompt(
    root: &Path,
    context: &str,
    message: &str,
) -> std::io::Result<LlmPrompt> {
    let catalog = chat_tool_catalog(root)?;
    let system = "\
Runtime identity:
This execution is a cognitive process of Buster.

You are the bounded planner for Buster's local web-console conversation.
Decide whether Buster should answer directly or call available tools first.
Return JSON only. Do not use Markdown fences.

Schema:
{
  \"direct_answer\": string | null,
  \"tool_calls\": [
    { \"tool\": \"tools.query|tools.view|skills.query|skills.view|skills.wechat_article_scraper|web.search|research.sources|paper.acquire|paper.brief|arena.status\", \"input\": object }
  ]
}

Rules:
- Prefer tools when the human asks what Buster can do, asks about installed skills/tools, asks to search/read/research, asks about Arena, or asks what Buster recently did.
- Use at most 3 tool calls.
- Do not invent tool outputs.
- For ordinary greetings, direct_answer can be a natural short reply and tool_calls should be empty.
- For risky writes, only plan listed bounded tools. Do not plan shell commands.";
    let user = format!(
        "Context:\n{}\n\nAvailable tool interface:\n{}\n\nHuman message:\n{}\n\nReturn JSON only.",
        truncate_chars(context, 6000),
        catalog,
        message
    );
    Ok(LlmPrompt {
        messages: vec![LlmMessage::system(system), LlmMessage::user(user)],
        max_tokens: Some(700),
    })
}

fn chat_agent_followup_prompt(
    context: &str,
    message: &str,
    observations: &[ChatToolObservation],
) -> LlmPrompt {
    let system = "\
You are Buster's bounded follow-up planner.
Given tool observations, decide whether one more bounded tool call is needed.
Return JSON only using the same schema as before.
If enough evidence exists, return {\"direct_answer\": null, \"tool_calls\": []}.";
    let user = format!(
        "Context:\n{}\n\nHuman message:\n{}\n\nObservations:\n{}\n\nReturn JSON only.",
        truncate_chars(context, 3000),
        message,
        serde_json::to_string_pretty(observations).unwrap_or_default()
    );
    LlmPrompt {
        messages: vec![LlmMessage::system(system), LlmMessage::user(user)],
        max_tokens: Some(500),
    }
}

fn chat_agent_final_prompt(
    context: &str,
    message: &str,
    observations: &[ChatToolObservation],
) -> LlmPrompt {
    let system = "\
Runtime identity:
This execution is a cognitive process of Buster.
Buster has just used tools. Reply naturally in Chinese unless asked otherwise.
Use the tool observations as the source of truth.
Be concise, say what was done, and mention failures plainly.
Do not reveal secrets or hidden reasoning.";
    let user = format!(
        "Context:\n{}\n\nHuman message:\n{}\n\nTool observations:\n{}\n\nAnswer the human based on these observations.",
        truncate_chars(context, 5000),
        message,
        serde_json::to_string_pretty(observations).unwrap_or_default()
    );
    LlmPrompt {
        messages: vec![LlmMessage::system(system), LlmMessage::user(user)],
        max_tokens: Some(900),
    }
}

fn chat_tool_catalog(root: &Path) -> std::io::Result<String> {
    let tools = ToolWorkspace::new(root)
        .read_registry()
        .or_else(|_| ToolWorkspace::new(root).refresh_registry())
        .map_err(std::io::Error::other)?;
    let skills = SkillWorkspace::new(root)
        .read_registry()
        .or_else(|_| SkillWorkspace::new(root).refresh_registry())
        .map_err(std::io::Error::other)?;
    let tool_lines = tools
        .entries
        .iter()
        .filter(|entry| format!("{:?}", entry.status) == "Active")
        .take(14)
        .map(|entry| {
            format!(
                "- {}: {} actions={}",
                entry.name,
                one_line(&entry.description, 140),
                entry
                    .action_rules
                    .iter()
                    .map(|rule| rule.action.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let skill_lines = skills
        .entries
        .iter()
        .filter(|entry| entry.status == SkillStatus::Active)
        .take(12)
        .map(|entry| format!("- {}: {}", entry.name, one_line(&entry.description, 140)))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!(
        "Planner-callable wrappers:\n- tools.query {{query}}\n- tools.view {{name}}\n- skills.query {{query}}\n- skills.view {{name}}\n- skills.wechat_article_scraper {{url}}\n- web.search {{query,count}}\n- research.sources {{query}}\n- paper.acquire {{query,limit}}\n- paper.brief {{question,max_papers}}\n- arena.status {{}}\n\nRegistered tools:\n{}\n\nActive skills:\n{}",
        if tool_lines.is_empty() { "(none)" } else { &tool_lines },
        if skill_lines.is_empty() { "(none)" } else { &skill_lines }
    ))
}

fn parse_chat_agent_plan(content: &str) -> Result<ChatAgentPlan, String> {
    let json = extract_json_object(content).ok_or_else(|| "no JSON object found".to_string())?;
    let mut plan: ChatAgentPlan = serde_json::from_str(json).map_err(|error| error.to_string())?;
    plan.tool_calls.truncate(3);
    Ok(plan)
}

fn extract_json_object(content: &str) -> Option<&str> {
    let trimmed = content.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    (start < end).then_some(&trimmed[start..=end])
}

fn execute_chat_tool_call(
    root: &Path,
    call: &ChatToolCall,
) -> std::io::Result<ChatToolObservation> {
    match call.tool.as_str() {
        "tools.query" => observe_tools_query(root, string_input(&call.input, "query")),
        "tools.view" => observe_tools_view(root, required_string_input(&call.input, "name")?),
        "skills.query" => observe_skills_query(root, string_input(&call.input, "query")),
        "skills.view" => observe_skills_view(root, required_string_input(&call.input, "name")?),
        "skills.wechat_article_scraper" => {
            observe_wechat_article_scraper(root, required_string_input(&call.input, "url")?)
        }
        "web.search" => observe_web_search(
            root,
            string_input(&call.input, "query"),
            usize_input(&call.input, "count").unwrap_or(5),
        ),
        "research.sources" => observe_research_sources(root, string_input(&call.input, "query")),
        "paper.acquire" => observe_paper_acquire(
            root,
            string_input(&call.input, "query"),
            usize_input(&call.input, "limit").unwrap_or(2),
        ),
        "paper.brief" => observe_paper_brief(
            root,
            string_input(&call.input, "question"),
            usize_input(&call.input, "max_papers").unwrap_or(2),
        ),
        "arena.status" => observe_arena_status(root),
        other => Ok(ChatToolObservation {
            tool: other.to_string(),
            status: "unsupported_tool".to_string(),
            summary: format!("unsupported chat tool `{other}`"),
            artifact_path: None,
        }),
    }
}

fn observe_tools_query(root: &Path, query: String) -> std::io::Result<ChatToolObservation> {
    let workspace = ToolWorkspace::new(root);
    let entries = workspace
        .query_tools(&query)
        .map_err(std::io::Error::other)?;
    let summary = if entries.is_empty() {
        format!("No tools matched query `{query}`.")
    } else {
        entries
            .iter()
            .take(10)
            .map(|entry| {
                format!(
                    "- {} [{:?}] {}",
                    entry.name,
                    entry.status,
                    one_line(&entry.description, 160)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(ChatToolObservation {
        tool: "tools.query".to_string(),
        status: "ok".to_string(),
        summary,
        artifact_path: Some(root.join("state").join("tool-registry.json")),
    })
}

fn observe_tools_view(root: &Path, name: String) -> std::io::Result<ChatToolObservation> {
    let workspace = ToolWorkspace::new(root);
    let view = workspace.view_tool(&name).map_err(std::io::Error::other)?;
    Ok(ChatToolObservation {
        tool: "tools.view".to_string(),
        status: "ok".to_string(),
        summary: serde_json::to_string_pretty(&view.entry).map_err(std::io::Error::other)?,
        artifact_path: Some(root.join("state").join("tool-registry.json")),
    })
}

fn observe_skills_query(root: &Path, query: String) -> std::io::Result<ChatToolObservation> {
    let workspace = SkillWorkspace::new(root);
    let registry = workspace
        .read_registry()
        .or_else(|_| workspace.refresh_registry())
        .map_err(std::io::Error::other)?;
    let needle = query.to_ascii_lowercase();
    let entries = registry
        .entries
        .into_iter()
        .filter(|entry| {
            needle.trim().is_empty()
                || entry.name.to_ascii_lowercase().contains(&needle)
                || entry.description.to_ascii_lowercase().contains(&needle)
                || entry
                    .triggers
                    .iter()
                    .any(|trigger| trigger.to_ascii_lowercase().contains(&needle))
        })
        .collect::<Vec<_>>();
    let summary = if entries.is_empty() {
        format!("No skills matched query `{query}`.")
    } else {
        entries
            .iter()
            .take(10)
            .map(|entry| {
                format!(
                    "- {} [{:?}] L{} {:?}: {}",
                    entry.name,
                    entry.status,
                    entry.governance_level,
                    entry.risk_level,
                    one_line(&entry.description, 150)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(ChatToolObservation {
        tool: "skills.query".to_string(),
        status: "ok".to_string(),
        summary,
        artifact_path: Some(root.join("state").join("skill-registry.json")),
    })
}

fn observe_skills_view(root: &Path, name: String) -> std::io::Result<ChatToolObservation> {
    let workspace = SkillWorkspace::new(root);
    let view = workspace.view_skill(&name).map_err(std::io::Error::other)?;
    Ok(ChatToolObservation {
        tool: "skills.view".to_string(),
        status: "ok".to_string(),
        summary: format!(
            "{}\n\n{}",
            serde_json::to_string_pretty(&view.entry).map_err(std::io::Error::other)?,
            truncate_chars(&view.markdown, 2400)
        ),
        artifact_path: Some(view.entry.path.join("SKILL.md")),
    })
}

fn observe_wechat_article_scraper(
    root: &Path,
    url: String,
) -> std::io::Result<ChatToolObservation> {
    if !url.starts_with("https://mp.weixin.qq.com/") {
        return Ok(ChatToolObservation {
            tool: "skills.wechat_article_scraper".to_string(),
            status: "blocked".to_string(),
            summary: "wechat_article_scraper only accepts https://mp.weixin.qq.com/ URLs"
                .to_string(),
            artifact_path: None,
        });
    }
    body_gate_bridge::preflight_network(root, &url, &["mp.weixin.qq.com"])
        .map_err(std::io::Error::other)?;
    let html = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) BusterWechatScraper/0.1")
        .build()
        .map_err(std::io::Error::other)?
        .get(&url)
        .header("Accept", "text/html")
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.text())
        .map_err(std::io::Error::other)?;
    let markdown = extract_wechat_markdown(&html);
    let status = if markdown.contains("Could not find WeChat article content") {
        ToolActionResult::Failed
    } else {
        ToolActionResult::Success
    };
    let dir = root.join("research").join("wechat");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}-wechat-article.md", now_secs()));
    body_gate_bridge::record_memory_write(
        root,
        "wechat_article_markdown",
        "skills.wechat_article_scraper",
        &markdown,
    )
    .map_err(std::io::Error::other)?;
    fs::write(&path, &markdown)?;
    let _ = SkillWorkspace::new(root).record_use(
        "wechat_article_scraper",
        format!("url={url}"),
        format!("saved markdown to {}", path.display()),
    );
    Ok(ChatToolObservation {
        tool: "skills.wechat_article_scraper".to_string(),
        status: if status == ToolActionResult::Success {
            "ok".to_string()
        } else {
            "failed".to_string()
        },
        summary: format!(
            "Fetched WeChat article and saved Markdown to {}.\n\n{}",
            path.display(),
            truncate_chars(&markdown, 2200)
        ),
        artifact_path: Some(path),
    })
}

fn observe_web_search(
    root: &Path,
    query: String,
    count: usize,
) -> std::io::Result<ChatToolObservation> {
    let mut request = WebSearchRequest::new(query);
    request.count = count.clamp(1, 8);
    request.task_id = Some(format!("chat-{}", now_secs()));
    let result = WebSearchClient::new().search(root, request);
    match result {
        Ok(bundle) => {
            let _ = ToolWorkspace::new(root).record_use(
                "research.web_search",
                ToolActionResult::Success,
                "chat agent web search completed",
            );
            Ok(ChatToolObservation {
                tool: "web.search".to_string(),
                status: "ok".to_string(),
                summary: web_search::web_search_evidence_prompt(&bundle),
                artifact_path: Some(bundle.bundle_path),
            })
        }
        Err(error) => {
            let _ = ToolWorkspace::new(root).record_use(
                "research.web_search",
                ToolActionResult::Failed,
                error.to_string(),
            );
            Ok(ChatToolObservation {
                tool: "web.search".to_string(),
                status: "failed".to_string(),
                summary: error.to_string(),
                artifact_path: None,
            })
        }
    }
}

fn extract_wechat_markdown(html: &str) -> String {
    let title = extract_meta_content(html, "og:title")
        .or_else(|| {
            extract_between(html, "rich_media_title", "</h1>").map(|value| strip_tags_loose(&value))
        })
        .map(|value| decode_basic_html(&value).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "WeChat Article".to_string());
    let Some(content_html) = extract_wechat_content_html(html) else {
        return format!("# {title}\n\nCould not find WeChat article content div `#js_content`.\n");
    };
    let markdown = html_fragment_to_markdown(&content_html);
    format!("# {title}\n\n{}\n", markdown.trim())
}

fn extract_meta_content(html: &str, property: &str) -> Option<String> {
    let property_marker = format!("property=\"{property}\"");
    let pos = html.find(&property_marker)?;
    let tag_start = html[..pos].rfind('<')?;
    let tag_end = html[pos..].find('>')? + pos;
    extract_html_attr(&html[tag_start..=tag_end], "content")
}

fn extract_wechat_content_html(html: &str) -> Option<String> {
    let id_pos = html
        .find("id=\"js_content\"")
        .or_else(|| html.find("id='js_content'"))?;
    let _div_start = html[..id_pos].rfind("<div")?;
    let open_end = html[id_pos..].find('>')? + id_pos;
    let close = html[open_end + 1..].find("</div>")? + open_end + 1;
    Some(html[open_end + 1..close].to_string())
}

fn html_fragment_to_markdown(fragment: &str) -> String {
    let mut out = String::new();
    let mut cursor = 0;
    while let Some(img_rel) = fragment[cursor..].find("<img") {
        let img_start = cursor + img_rel;
        out.push_str(&strip_tags_loose(&fragment[cursor..img_start]));
        let img_end = fragment[img_start..]
            .find('>')
            .map(|offset| img_start + offset + 1)
            .unwrap_or(fragment.len());
        let img_tag = &fragment[img_start..img_end];
        if let Some(src) =
            extract_html_attr(img_tag, "data-src").or_else(|| extract_html_attr(img_tag, "src"))
        {
            out.push_str(&format!("\n\n![image]({})\n\n", decode_basic_html(&src)));
        }
        cursor = img_end;
    }
    out.push_str(&strip_tags_loose(&fragment[cursor..]));
    decode_basic_html(&out)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn strip_tags_loose(html: &str) -> String {
    let text = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n")
        .replace("</section>", "\n\n");
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn extract_html_attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let start = tag.find(&needle)? + needle.len();
    let quote = tag[start..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value_start = start + quote.len_utf8();
    let value_end = tag[value_start..].find(quote)? + value_start;
    Some(tag[value_start..value_end].to_string())
}

fn extract_between(text: &str, start_marker: &str, end_marker: &str) -> Option<String> {
    let start = text.find(start_marker)?;
    let after = &text[start..];
    let tag_end = after.find('>')?;
    let content_start = start + tag_end + 1;
    let end = text[content_start..].find(end_marker)? + content_start;
    Some(text[content_start..end].to_string())
}

fn decode_basic_html(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn observe_research_sources(root: &Path, query: String) -> std::io::Result<ChatToolObservation> {
    let bundle = SourceFetcher::new().fetch_bundle(
        root,
        Some(&format!("chat-{}", now_secs())),
        infer_research_domain_from_message(&query),
        &query,
    )?;
    let _ = ToolWorkspace::new(root).record_use(
        "research.source_fetch",
        ToolActionResult::Success,
        "chat agent source fetch completed",
    );
    Ok(ChatToolObservation {
        tool: "research.sources".to_string(),
        status: "ok".to_string(),
        summary: source_fetch::source_evidence_prompt(&bundle),
        artifact_path: Some(bundle.bundle_path),
    })
}

fn observe_paper_acquire(
    root: &Path,
    query: String,
    limit: usize,
) -> std::io::Result<ChatToolObservation> {
    let mut request = PaperAcquisitionRequest::new(query.clone());
    request.domain = infer_research_domain_from_message(&query);
    request.limit = limit.clamp(1, 3);
    request.task_id = Some(format!("chat-paper-{}", now_secs()));
    let record = paper_acquisition::acquire_open_papers(root, request)?;
    let _ = ToolWorkspace::new(root).record_use(
        "research.paper_acquire",
        if record.downloaded.is_empty() {
            ToolActionResult::Blocked
        } else {
            ToolActionResult::Success
        },
        format!("downloaded {} open paper(s)", record.downloaded.len()),
    );
    Ok(ChatToolObservation {
        tool: "paper.acquire".to_string(),
        status: record.status,
        summary: format!(
            "Downloaded {} open paper(s), skipped {}, errors {}. Manifest: {}",
            record.downloaded.len(),
            record.skipped.len(),
            record.errors.len(),
            record.manifest_path.display()
        ),
        artifact_path: Some(record.manifest_path),
    })
}

fn observe_paper_brief(
    root: &Path,
    question: String,
    max_papers: usize,
) -> std::io::Result<ChatToolObservation> {
    let mut request = PaperBriefRequest::new(question.clone());
    request.domain = infer_research_domain_from_message(&question);
    request.max_papers = max_papers.clamp(1, 3);
    request.task_id = Some(format!("chat-paper-{}", now_secs()));
    let record = paper_brief::brief_latest(root, request)?;
    let _ = ToolWorkspace::new(root).record_use(
        "research.paper_brief",
        if record.status == "ok" {
            ToolActionResult::Success
        } else {
            ToolActionResult::Failed
        },
        format!("paper brief status {}", record.status),
    );
    Ok(ChatToolObservation {
        tool: "paper.brief".to_string(),
        status: record.status,
        summary: record.summary,
        artifact_path: record.brief_path,
    })
}

fn observe_arena_status(root: &Path) -> std::io::Result<ChatToolObservation> {
    match crate::arena::status(root) {
        Ok(status) => Ok(ChatToolObservation {
            tool: "arena.status".to_string(),
            status: "ok".to_string(),
            summary: serde_json::to_string_pretty(&status).map_err(std::io::Error::other)?,
            artifact_path: Some(root.join("audit").join("arena-status.jsonl")),
        }),
        Err(error) => Ok(ChatToolObservation {
            tool: "arena.status".to_string(),
            status: "failed".to_string(),
            summary: error.to_string(),
            artifact_path: None,
        }),
    }
}

fn observations_to_answer(observations: &[ChatToolObservation]) -> String {
    observations
        .iter()
        .map(|observation| {
            format!(
                "工具 `{}` [{}]\n{}{}",
                observation.tool,
                observation.status,
                observation.summary,
                observation
                    .artifact_path
                    .as_ref()
                    .map(|path| format!("\nartifact: {}", path.display()))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn append_chat_agent_step(
    root: &Path,
    contract_id: &str,
    step: &str,
    summary: &str,
    details: Option<serde_json::Value>,
) -> std::io::Result<()> {
    append_daemon_jsonl(
        &root.join("audit").join("chat-agent-steps.jsonl"),
        &serde_json::json!({
            "timestamp_secs": now_secs(),
            "contract_id": contract_id,
            "step": step,
            "summary": summary,
            "details": details,
        }),
    )
}

fn string_input(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn required_string_input(value: &serde_json::Value, key: &str) -> std::io::Result<String> {
    let found = string_input(value, key);
    if found.is_empty() {
        Err(std::io::Error::other(format!(
            "tool input field `{key}` is required"
        )))
    } else {
        Ok(found)
    }
}

fn usize_input(value: &serde_json::Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(|value| value.as_u64())
        .and_then(|value| usize::try_from(value).ok())
}

fn cleanup_query_text(message: &str) -> String {
    let cleaned = message
        .replace("联网搜索", "")
        .replace("上网查一下", "")
        .replace("查一下", "")
        .replace("搜一下", "")
        .replace("搜索", "")
        .replace("论文", "")
        .replace("阅读", "")
        .replace("下载", "")
        .replace("请", "")
        .trim()
        .to_string();
    if cleaned.is_empty() {
        message.trim().to_string()
    } else {
        cleaned
    }
}

fn one_line(text: &str, max_chars: usize) -> String {
    let mut compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > max_chars {
        compact = compact.chars().take(max_chars).collect::<String>();
        compact.push_str("...");
    }
    compact
}

fn respond_to_direct_paper_request(
    root: &Path,
    timestamp_secs: u64,
    contract_id: &str,
    message: &str,
    request: DirectPaperRequest,
) -> std::io::Result<ChatRecord> {
    let mut acquisition = PaperAcquisitionRequest::new(request.question.clone());
    acquisition.domain = request.domain;
    acquisition.task_id = Some(format!("human-paper-{timestamp_secs}"));
    acquisition.limit = 1;

    let acquisition_record =
        match paper_acquisition::acquire_direct_pdf(root, acquisition, &request.url) {
            Ok(record) => record,
            Err(error) => {
                return Ok(ChatRecord {
                    contract_id: Some(contract_id.to_string()),
                    timestamp_secs,
                    human_message: message.to_string(),
                    buster_reply: format!(
                        "我没有真的读到这篇论文。runtime 尝试下载这个 PDF 时失败了：{error}"
                    ),
                    status: "paper_acquisition_failed".to_string(),
                    model: None,
                    provider: None,
                    total_tokens: None,
                });
            }
        };

    let mut brief_request = PaperBriefRequest::new(request.question);
    brief_request.domain = request.domain;
    brief_request.task_id = Some(format!("human-paper-{timestamp_secs}"));
    brief_request.max_papers = 1;
    let brief = paper_brief::brief_latest(root, brief_request)?;
    let paper_line = brief
        .selected_papers
        .first()
        .map(|paper| {
            format!(
                "{} | sha256={} | text_chars={}",
                paper.title,
                paper.sha256.chars().take(16).collect::<String>(),
                paper.text_chars
            )
        })
        .unwrap_or_else(|| "没有可用的提取文本".to_string());
    let brief_path = brief
        .brief_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "未生成 brief 文件".to_string());
    let summary = truncate_chars(&brief.summary, 1600);
    let reply = if brief.selected_papers.is_empty() {
        format!(
            "我这次没有只停留在口头承诺，而是真的走了论文管线；不过结果不完整。\n\n下载状态：{}\n下载数量：{}\nPDF：{}\n问题：{}\n结果：{}\n\n{}",
            acquisition_record.status,
            acquisition_record.downloaded.len(),
            request.url,
            acquisition_record.question,
            paper_line,
            summary
        )
    } else {
        format!(
            "我已经真实下载并阅读了这篇 PDF，结果写进了 Buster 的 paper brief。\n\n论文：{}\nbrief：{}\n状态：{}\n\n{}",
            paper_line, brief_path, brief.status, summary
        )
    };

    Ok(ChatRecord {
        contract_id: Some(contract_id.to_string()),
        timestamp_secs,
        human_message: message.to_string(),
        buster_reply: reply,
        status: if brief.selected_papers.is_empty() {
            "paper_read_partial".to_string()
        } else {
            "ok".to_string()
        },
        model: brief.model,
        provider: brief.provider,
        total_tokens: brief.total_tokens,
    })
}

fn detect_direct_paper_request(message: &str) -> Option<DirectPaperRequest> {
    let url = extract_direct_pdf_url(message)?;
    let lower = message.to_ascii_lowercase();
    let asks_to_read = lower.contains("read")
        || lower.contains("study")
        || lower.contains("analyze")
        || lower.contains("analyse")
        || lower.contains("summarize")
        || lower.contains("summarise")
        || message.contains("论文")
        || message.contains("学习")
        || message.contains("看一下")
        || message.contains("读一下")
        || message.contains("研究");
    if !asks_to_read {
        return None;
    }
    Some(DirectPaperRequest {
        url,
        question: message.trim().to_string(),
        domain: infer_research_domain_from_message(message),
    })
}

fn extract_direct_pdf_url(message: &str) -> Option<String> {
    extract_https_url(message).filter(|url| url.to_ascii_lowercase().contains(".pdf"))
}

fn infer_research_domain_from_message(message: &str) -> ResearchDomain {
    let lower = message.to_ascii_lowercase();
    if lower.contains("harness") || lower.contains("agent") || lower.contains("security") {
        ResearchDomain::Cybersecurity
    } else if message.contains("量子") || lower.contains("quantum") {
        ResearchDomain::QuantumComputing
    } else if message.contains("航天") || lower.contains("aerospace") {
        ResearchDomain::Aerospace
    } else {
        ResearchDomain::Other
    }
}

fn respond_to_skill_install_request(
    root: &Path,
    timestamp_secs: u64,
    contract_id: &str,
    message: &str,
    request: SkillInstallRequest,
) -> std::io::Result<ChatRecord> {
    if matches!(request.mode, SkillInstallMode::GithubTree { .. }) {
        return respond_to_skill_repo_install_request(
            root,
            timestamp_secs,
            contract_id,
            message,
            request,
        );
    }
    let markdown = match fetch_skill_markdown_url(root, &request.url) {
        Ok(markdown) => markdown,
        Err(error) => {
            return Ok(ChatRecord {
                contract_id: Some(contract_id.to_string()),
                timestamp_secs,
                human_message: message.to_string(),
                buster_reply: format!("我没有安装这个 skill。下载或 BodyGate 预检失败：{error}"),
                status: "skill_install_failed".to_string(),
                model: None,
                provider: None,
                total_tokens: None,
            });
        }
    };
    let workspace = SkillWorkspace::new(root);
    let receipt = match workspace.install_markdown_skill(
        &markdown,
        request.url.clone(),
        request.name.as_deref(),
        request.activate,
    ) {
        Ok(receipt) => receipt,
        Err(error) => {
            return Ok(ChatRecord {
                contract_id: Some(contract_id.to_string()),
                timestamp_secs,
                human_message: message.to_string(),
                buster_reply: format!(
                    "我下载到了 skill markdown，但没有安装。安全/格式检查失败：{error}"
                ),
                status: "skill_install_rejected".to_string(),
                model: None,
                provider: None,
                total_tokens: None,
            });
        }
    };
    let reply = format!(
        "我已经真实安装了这个 skill，并刷新了 skill registry。\n\n名称：{}\n状态：{:?}\n风险层级：L{} / {:?}\n路径：{}\n来源：{}\n\n默认策略已经放宽：通过安全校验的外部 skill 会直接进入可用状态；真正执行到网络、文件、账号、进程或外部副作用时，再由 BodyGate 按具体动作约束。",
        receipt.entry.name,
        receipt.entry.status,
        receipt.entry.governance_level,
        receipt.entry.risk_level,
        receipt.entry.path.display(),
        receipt.source
    );
    Ok(ChatRecord {
        contract_id: Some(contract_id.to_string()),
        timestamp_secs,
        human_message: message.to_string(),
        buster_reply: reply,
        status: "ok".to_string(),
        model: None,
        provider: None,
        total_tokens: None,
    })
}

fn respond_to_skill_repo_install_request(
    root: &Path,
    timestamp_secs: u64,
    contract_id: &str,
    message: &str,
    request: SkillInstallRequest,
) -> std::io::Result<ChatRecord> {
    let skills = match discover_github_tree_skills(root, &request.mode) {
        Ok(skills) => skills,
        Err(error) => {
            return Ok(ChatRecord {
                contract_id: Some(contract_id.to_string()),
                timestamp_secs,
                human_message: message.to_string(),
                buster_reply: format!("我没有学会这个仓库里的 skills。目录扫描失败：{error}"),
                status: "skill_repo_scan_failed".to_string(),
                model: None,
                provider: None,
                total_tokens: None,
            });
        }
    };
    if skills.is_empty() {
        return Ok(ChatRecord {
            contract_id: Some(contract_id.to_string()),
            timestamp_secs,
            human_message: message.to_string(),
            buster_reply: format!(
                "我扫描了这个仓库目录，但没有找到可安装的 `SKILL.md`：{}",
                request.url
            ),
            status: "skill_repo_empty".to_string(),
            model: None,
            provider: None,
            total_tokens: None,
        });
    }

    let workspace = SkillWorkspace::new(root);
    let mut installed = Vec::new();
    let mut failed = Vec::new();
    for skill in skills.into_iter().take(24) {
        match workspace.install_markdown_skill(
            &skill.markdown,
            skill.source_url.clone(),
            None,
            request.activate,
        ) {
            Ok(receipt) => installed.push(receipt.entry),
            Err(error) => failed.push(format!(
                "{}: {}",
                skill.source_url,
                one_line(&error.to_string(), 160)
            )),
        }
    }

    let installed_lines = installed
        .iter()
        .take(20)
        .map(|entry| {
            format!(
                "- {} [{:?}, L{} {:?}]",
                entry.name, entry.status, entry.governance_level, entry.risk_level
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let failed_lines = failed
        .iter()
        .take(8)
        .map(|line| format!("- {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let status = if installed.is_empty() {
        "skill_repo_install_failed"
    } else if failed.is_empty() {
        "ok"
    } else {
        "skill_repo_install_partial"
    };
    let reply = format!(
        "我已经扫描并安装这个仓库目录里的 skills。\n\n来源：{}\n成功安装：{} 个\n失败：{} 个\n默认状态：{}\n\n{}\n{}\n\n默认策略已经放宽：通过安全校验的外部 skill 会直接进入可用状态；真正执行到网络、文件、账号、进程或外部副作用时，再由 BodyGate 按具体动作约束。",
        request.url,
        installed.len(),
        failed.len(),
        "Active",
        if installed_lines.is_empty() {
            "没有成功安装的 skill。".to_string()
        } else {
            installed_lines
        },
        if failed_lines.is_empty() {
            String::new()
        } else {
            format!("\n失败项：\n{failed_lines}")
        }
    );

    Ok(ChatRecord {
        contract_id: Some(contract_id.to_string()),
        timestamp_secs,
        human_message: message.to_string(),
        buster_reply: reply,
        status: status.to_string(),
        model: None,
        provider: None,
        total_tokens: None,
    })
}

fn detect_skill_install_request(message: &str) -> Option<SkillInstallRequest> {
    let lower = message.to_ascii_lowercase();
    let mentions_skill = lower.contains("skill") || message.contains("技能");
    let asks_install = lower.contains("install")
        || lower.contains("learn")
        || message.contains("安装")
        || message.contains("接入")
        || message.contains("学会")
        || message.contains("学习");
    if !mentions_skill || !asks_install {
        return None;
    }
    let url = extract_https_url(message)?;
    if let Some((owner, repo, branch, path)) = parse_github_tree_url(&url) {
        let lower_path = path.to_ascii_lowercase();
        if lower_path.contains("skill") {
            return Some(SkillInstallRequest {
                url,
                name: None,
                activate: lower.contains("activate=true") || message.contains("直接激活"),
                mode: SkillInstallMode::GithubTree {
                    owner,
                    repo,
                    branch,
                    path,
                },
            });
        }
    }
    let looks_like_skill_url = url.to_ascii_lowercase().ends_with(".md")
        || url.to_ascii_lowercase().contains("skill")
        || url.to_ascii_lowercase().contains("/skills/");
    if !looks_like_skill_url {
        return None;
    }
    Some(SkillInstallRequest {
        url,
        name: None,
        activate: lower.contains("activate=true") || message.contains("直接激活"),
        mode: SkillInstallMode::SingleMarkdown,
    })
}

fn respond_to_skill_approval_request(
    root: &Path,
    timestamp_secs: u64,
    contract_id: &str,
    message: &str,
    request: SkillApprovalRequest,
    owner_authorized: bool,
) -> std::io::Result<ChatRecord> {
    match request {
        SkillApprovalRequest::Clarify { candidates } => Ok(ChatRecord {
            contract_id: Some(contract_id.to_string()),
            timestamp_secs,
            human_message: message.to_string(),
            buster_reply: format!(
                "我理解你在批准 skill，但现在有多个待审核项。请明确说出要激活哪一个：{}",
                candidates.join(", ")
            ),
            status: "skill_approval_needs_name".to_string(),
            model: None,
            provider: None,
            total_tokens: None,
        }),
        SkillApprovalRequest::Approve { name } => {
            if !owner_authorized {
                append_owner_consent(
                    root,
                    timestamp_secs,
                    contract_id,
                    "approve_skill",
                    &name,
                    "rejected_missing_owner_token",
                )?;
                return Ok(ChatRecord {
                    contract_id: Some(contract_id.to_string()),
                    timestamp_secs,
                    human_message: message.to_string(),
                    buster_reply: "我理解这是批准 skill 的指令，但这次请求没有通过本地 owner token 验证，所以不能生效。请从本机 Buster Console 重新发送。".to_string(),
                    status: "owner_consent_required".to_string(),
                    model: None,
                    provider: None,
                    total_tokens: None,
                });
            }
            let workspace = SkillWorkspace::new(root);
            match workspace.approve_installed_skill(&name, "local-owner-console") {
                Ok(entry) => {
                    append_owner_consent(
                        root,
                        timestamp_secs,
                        contract_id,
                        "approve_skill",
                        &entry.name,
                        "approved",
                    )?;
                    Ok(ChatRecord {
                        contract_id: Some(contract_id.to_string()),
                        timestamp_secs,
                        human_message: message.to_string(),
                        buster_reply: format!(
                            "已确认你的本地 owner consent。\n\n我已经把 skill `{}` 从待审核状态激活为 `Active`。\n风险层级：L{} / {:?}\n路径：{}\n\n下一步，Buster 在需要完成相应任务时就可以查询并使用这个 skill；实际联网、读取文件、调用 CLI 或账号资源时仍会经过 BodyGate。",
                            entry.name,
                            entry.governance_level,
                            entry.risk_level,
                            entry.path.display()
                        ),
                        status: "ok".to_string(),
                        model: None,
                        provider: None,
                        total_tokens: None,
                    })
                }
                Err(error) => {
                    append_owner_consent(
                        root,
                        timestamp_secs,
                        contract_id,
                        "approve_skill",
                        &name,
                        "failed",
                    )?;
                    Ok(ChatRecord {
                        contract_id: Some(contract_id.to_string()),
                        timestamp_secs,
                        human_message: message.to_string(),
                        buster_reply: format!(
                            "我理解这是批准 skill 的指令，但没有成功激活 `{name}`：{error}"
                        ),
                        status: "skill_approval_failed".to_string(),
                        model: None,
                        provider: None,
                        total_tokens: None,
                    })
                }
            }
        }
    }
}

fn detect_skill_approval_request(
    root: &Path,
    message: &str,
) -> std::io::Result<Option<SkillApprovalRequest>> {
    let lower = message.to_ascii_lowercase();
    let asks_approval = lower.contains("approve")
        || lower.contains("activate")
        || lower.contains("enable")
        || message.contains("同意")
        || message.contains("批准")
        || message.contains("确认")
        || message.contains("激活")
        || message.contains("启用")
        || message.contains("可以用");
    if !asks_approval {
        return Ok(None);
    }

    let workspace = SkillWorkspace::new(root);
    let registry = workspace
        .read_registry()
        .or_else(|_| workspace.refresh_registry())
        .map_err(std::io::Error::other)?;
    let pending = registry
        .entries
        .into_iter()
        .filter(|entry| entry.scope == SkillScope::Installed)
        .filter(|entry| entry.status == SkillStatus::NeedsReview)
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(None);
    }

    if let Some(entry) = find_named_skill_in_message(&pending, message) {
        return Ok(Some(SkillApprovalRequest::Approve {
            name: entry.name.clone(),
        }));
    }

    let mentions_skill = lower.contains("skill") || message.contains("技能");
    let short_local_consent = message.trim().chars().count() <= 24;
    if pending.len() == 1 && (mentions_skill || short_local_consent) {
        return Ok(Some(SkillApprovalRequest::Approve {
            name: pending[0].name.clone(),
        }));
    }

    if mentions_skill {
        return Ok(Some(SkillApprovalRequest::Clarify {
            candidates: pending.into_iter().map(|entry| entry.name).collect(),
        }));
    }
    Ok(None)
}

fn find_named_skill_in_message<'a>(
    pending: &'a [SkillRegistryEntry],
    message: &str,
) -> Option<&'a SkillRegistryEntry> {
    let lower = message.to_ascii_lowercase();
    pending.iter().find(|entry| {
        let name = entry.name.to_ascii_lowercase();
        let slug = name.replace(' ', "-");
        lower.contains(&name) || lower.contains(&slug)
    })
}

fn append_owner_consent(
    root: &Path,
    timestamp_secs: u64,
    contract_id: &str,
    action: &str,
    target: &str,
    status: &str,
) -> std::io::Result<()> {
    append_daemon_jsonl(
        &root.join("audit").join("owner-consent.jsonl"),
        &serde_json::json!({
            "timestamp_secs": timestamp_secs,
            "authority": "local_owner_console",
            "action": action,
            "target": target,
            "status": status,
            "contract_id": contract_id,
            "consent_basis": "valid X-Buster-Owner-Token on localhost web console",
        }),
    )
}

fn extract_https_url(message: &str) -> Option<String> {
    let start = message.to_ascii_lowercase().find("https://")?;
    let rest = &message[start..];
    let end = rest
        .char_indices()
        .find_map(|(index, ch)| {
            if ch.is_whitespace()
                || matches!(
                    ch,
                    '"' | '\''
                        | '<'
                        | '>'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '，'
                        | '。'
                        | '、'
                        | '；'
                        | '：'
                )
            {
                Some(index)
            } else {
                None
            }
        })
        .unwrap_or(rest.len());
    let token = &rest[..end];
    Some(
        token
            .trim_end_matches([',', '.', ';', ':', '!', '?', '！', '？'])
            .to_string(),
    )
}

fn fetch_skill_markdown_url(root: &Path, url: &str) -> Result<String, String> {
    let url = normalize_skill_markdown_url(url)?;
    let parsed = reqwest::Url::parse(&url).map_err(|error| error.to_string())?;
    if parsed.scheme() != "https" {
        return Err("skill install URL must use https".to_string());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "skill install URL has no host".to_string())?
        .to_ascii_lowercase();
    body_gate_bridge::preflight_network(root, &url, &[host.as_str()])
        .map_err(|error| error.to_string())?;
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("BusterSkillInstaller/0.1")
        .build()
        .map_err(|error| error.to_string())?
        .get(&url)
        .header("Accept", "text/markdown,text/plain,*/*")
        .send();
    let text = match response {
        Ok(response) => {
            let response = response
                .error_for_status()
                .map_err(|error| error.to_string())?;
            if response.content_length().unwrap_or(0) > 256 * 1024 {
                return Err("skill markdown response is too large".to_string());
            }
            response.text().map_err(|error| error.to_string())?
        }
        Err(error) => fetch_text_with_curl(&url, &format!("reqwest failed: {error}"))?,
    };
    if text.chars().count() > 16_000 {
        return Err("skill markdown exceeds 16000 characters".to_string());
    }
    Ok(text)
}

fn fetch_text_with_curl(url: &str, cause: &str) -> Result<String, String> {
    let output = Command::new("curl")
        .args([
            "--location",
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "30",
            "--max-filesize",
            "262144",
            url,
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("{cause}; curl fallback could not start: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{cause}; curl fallback failed with status {:?}: {}",
            output.status.code(),
            stderr.trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("{cause}; curl fallback returned non-utf8 data: {error}"))
}

#[derive(Debug, Deserialize)]
struct GithubContentItem {
    name: String,
    path: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    download_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredSkillMarkdown {
    source_url: String,
    markdown: String,
}

fn discover_github_tree_skills(
    root: &Path,
    mode: &SkillInstallMode,
) -> Result<Vec<DiscoveredSkillMarkdown>, String> {
    let SkillInstallMode::GithubTree {
        owner,
        repo,
        branch,
        path,
    } = mode
    else {
        return Err("skill repo discovery requires a GitHub tree URL".to_string());
    };
    let api_url = format!(
        "https://api.github.com/repos/{owner}/{repo}/contents/{}?ref={branch}",
        path.trim_matches('/')
    );
    body_gate_bridge::preflight_network(root, &api_url, &["api.github.com"])
        .map_err(|error| error.to_string())?;
    let text = fetch_github_text(&api_url)?;
    let items: Vec<GithubContentItem> =
        serde_json::from_str(&text).map_err(|error| format!("GitHub API parse failed: {error}"))?;
    let mut raw_urls = Vec::new();
    for item in items {
        if item.kind == "file" && item.name.eq_ignore_ascii_case("SKILL.md") {
            if let Some(download_url) = item.download_url {
                raw_urls.push(download_url);
            } else {
                raw_urls.push(format!(
                    "https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{}",
                    item.path
                ));
            }
        } else if item.kind == "dir" {
            raw_urls.push(format!(
                "https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{}/SKILL.md",
                item.path
            ));
        }
    }
    raw_urls.sort();
    raw_urls.dedup();

    let mut skills = Vec::new();
    let mut failures = Vec::new();
    for raw_url in raw_urls.into_iter().take(32) {
        match fetch_skill_markdown_url(root, &raw_url) {
            Ok(markdown) => {
                if markdown.contains("#") && markdown.to_ascii_lowercase().contains("skill") {
                    skills.push(DiscoveredSkillMarkdown {
                        source_url: raw_url,
                        markdown,
                    });
                } else {
                    failures.push(format!("{raw_url}: not a recognizable skill markdown"));
                }
            }
            Err(error) => failures.push(format!("{raw_url}: {error}")),
        }
    }
    if skills.is_empty() && !failures.is_empty() {
        return Err(format!(
            "found candidate paths but none could be downloaded: {}",
            failures.into_iter().take(5).collect::<Vec<_>>().join("; ")
        ));
    }
    Ok(skills)
}

fn fetch_github_text(url: &str) -> Result<String, String> {
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("BusterSkillInstaller/0.1")
        .build()
        .map_err(|error| error.to_string())?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send();
    match response {
        Ok(response) => response
            .error_for_status()
            .map_err(|error| error.to_string())?
            .text()
            .map_err(|error| error.to_string()),
        Err(error) => fetch_text_with_curl(url, &format!("reqwest failed: {error}")),
    }
}

fn normalize_skill_markdown_url(url: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(url).map_err(|error| error.to_string())?;
    let Some(host) = parsed.host_str() else {
        return Err("skill install URL has no host".to_string());
    };
    if host.eq_ignore_ascii_case("github.com") {
        let segments = parsed
            .path_segments()
            .map(|segments| segments.collect::<Vec<_>>())
            .unwrap_or_default();
        if segments.len() >= 5 && segments[2] == "blob" {
            let owner = segments[0];
            let repo = segments[1];
            let branch = segments[3];
            let path = segments[4..].join("/");
            return Ok(format!(
                "https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}"
            ));
        }
    }
    Ok(url.to_string())
}

fn parse_github_tree_url(url: &str) -> Option<(String, String, String, String)> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if !parsed.host_str()?.eq_ignore_ascii_case("github.com") {
        return None;
    }
    let segments = parsed.path_segments()?.collect::<Vec<_>>();
    if segments.len() < 5 || segments[2] != "tree" {
        return None;
    }
    let owner = segments[0].to_string();
    let repo = segments[1].to_string();
    let branch = segments[3].to_string();
    let path = segments[4..].join("/");
    Some((owner, repo, branch, path))
}

fn execute_research(root: &Path, candidate: &ActionCandidate) -> std::io::Result<ExecutionRecord> {
    let timestamp_secs = now_secs();
    let record_path = root.join("audit").join("research.jsonl");
    let domain = candidate.research_domain.unwrap_or(ResearchDomain::Other);
    let question = candidate
        .research_question
        .clone()
        .unwrap_or_else(|| candidate.summary.clone());
    if matches!(
        candidate.info_source,
        Some(buster_value_model::InfoSource::Paper)
    ) {
        return execute_paper_research(root, candidate, timestamp_secs, domain, &question);
    }
    let contract = harness::research_contract(timestamp_secs, candidate, domain, &question);
    let contract_id = contract.contract_id.clone();
    harness::append_contract(root, &contract)?;
    if let Some(reason) = research_budget_block_reason(root) {
        let record = ExecutionRecord {
            contract_id: Some(contract_id.clone()),
            timestamp_secs,
            action_kind: format!("{:?}", candidate.kind),
            status: "research_budget_deferred".to_string(),
            summary: reason,
            output_path: None,
            record_path: record_path.clone(),
            model: None,
            provider: None,
            total_tokens: None,
            source_bundle_path: None,
            source_count: 0,
            research_quality_score: None,
            research_claim_status: None,
        };
        append_daemon_jsonl(&record_path, &record)?;
        harness::append_outcome(
            root,
            &harness::HarnessOutcome::new(
                contract_id,
                harness::HarnessStatus::Deferred,
                "research deferred by budget guard",
            )
            .with_error(record.summary.clone()),
        )?;
        return Ok(record);
    }
    let (client, provider) = match llm_provider(root) {
        Ok(parts) => parts,
        Err(error) => {
            research::record_research_completion(
                root,
                ResearchCompletion {
                    contract_id: Some(&contract_id),
                    task_id: candidate.research_task_id.as_deref(),
                    domain,
                    question: &question,
                    status: "llm_unavailable",
                    report_path: None,
                    model: None,
                    provider: None,
                    total_tokens: None,
                    source_bundle_path: None,
                    source_count: 0,
                    content: None,
                },
            )?;
            let record = ExecutionRecord {
                contract_id: Some(contract_id.clone()),
                timestamp_secs,
                action_kind: format!("{:?}", candidate.kind),
                status: "llm_unavailable".to_string(),
                summary: format!("research executor could not start LLM: {error}"),
                output_path: None,
                record_path: record_path.clone(),
                model: None,
                provider: None,
                total_tokens: None,
                source_bundle_path: None,
                source_count: 0,
                research_quality_score: None,
                research_claim_status: None,
            };
            append_daemon_jsonl(&record_path, &record)?;
            harness::append_outcome(
                root,
                &harness::HarnessOutcome::new(
                    contract_id,
                    harness::HarnessStatus::Deferred,
                    "research deferred because the LLM provider was unavailable",
                )
                .with_error(error),
            )?;
            return Ok(record);
        }
    };

    let source_bundle = SourceFetcher::new().fetch_bundle(
        root,
        candidate.research_task_id.as_deref(),
        domain,
        &question,
    )?;
    let source_context = source_fetch::source_evidence_prompt(&source_bundle);
    let web_bundle = WebSearchClient::new().search(root, {
        let mut request = WebSearchRequest::new(&question);
        request.count = 5;
        request.task_id = candidate.research_task_id.clone();
        request
    });
    let web_context = match web_bundle {
        Ok(bundle) => web_search::web_search_evidence_prompt(&bundle),
        Err(error) => format!("Web search failed before bundle write: {error}"),
    };
    let request = SecondaryInfoRequest::new(research::research_prompt(domain, candidate))
        .with_context(format!(
            "{}\n\nFetched source evidence:\n{}\n\nFetched web evidence:\n{}",
            read_runtime_context(root),
            source_context,
            web_context
        ))
        .with_max_tokens(800);
    let report = query_with_retry(root, client, provider, request, llm_retry_count(root));

    let record = match report {
        Ok(report) => {
            let output_path = write_research_markdown(
                root,
                timestamp_secs,
                domain,
                candidate.research_task_id.as_deref(),
                &question,
                Some(&source_bundle.bundle_path),
                source_bundle.sources.len(),
                &report.content,
            )?;
            research::record_research_completion(
                root,
                ResearchCompletion {
                    contract_id: Some(&contract_id),
                    task_id: candidate.research_task_id.as_deref(),
                    domain,
                    question: &question,
                    status: "ok",
                    report_path: Some(output_path.clone()),
                    model: Some(report.model.clone()),
                    provider: report.provider.clone(),
                    total_tokens: report.total_tokens,
                    source_bundle_path: Some(source_bundle.bundle_path.clone()),
                    source_count: source_bundle.sources.len(),
                    content: Some(&report.content),
                },
            )?;
            let quality = research_framework::evaluate_and_record(
                root,
                ResearchQualityInput {
                    contract_id: Some(&contract_id),
                    task_id: candidate.research_task_id.as_deref(),
                    domain,
                    question: &question,
                    source_count: source_bundle.sources.len(),
                    content: Some(&report.content),
                    synthesis_succeeded: true,
                    report_path: Some(output_path.clone()),
                },
            )?;
            let output_path_display = output_path.display().to_string();
            let record = ExecutionRecord {
                contract_id: Some(contract_id.clone()),
                timestamp_secs,
                action_kind: format!("{:?}", candidate.kind),
                status: "ok".to_string(),
                summary: format!(
                    "wrote secondary research report for {:?} to {}",
                    domain, output_path_display
                ),
                output_path: Some(output_path),
                record_path: record_path.clone(),
                model: Some(report.model),
                provider: report.provider,
                total_tokens: report.total_tokens,
                source_bundle_path: Some(source_bundle.bundle_path.clone()),
                source_count: source_bundle.sources.len(),
                research_quality_score: Some(quality.quality_score),
                research_claim_status: Some(quality.claim_status.as_str().to_string()),
            };
            harness::append_outcome(
                root,
                &harness::HarnessOutcome::new(
                    contract_id.clone(),
                    harness::HarnessStatus::Completed,
                    record.summary.clone(),
                )
                .with_evidence(
                    "source_bundle",
                    source_bundle.bundle_path.display().to_string(),
                    "fetched source evidence",
                )
                .with_output(
                    "research_report",
                    output_path_display,
                    "synthesized research report",
                )
                .with_output(
                    "audit",
                    "audit/research-ledger.jsonl",
                    "research ledger entry",
                ),
            )?;
            record
        }
        Err(error) => {
            let fallback_content = format!(
                "## Source Fetch Completed; LLM Synthesis Failed\n\nBuster fetched external evidence, but the LLM synthesis step failed.\n\n- llm_error: {}\n- source_count: {}\n- source_bundle: {}\n\n## Fetched Evidence Leads\n\n{}",
                error,
                source_bundle.sources.len(),
                source_bundle.bundle_path.display(),
                source_fetch::source_evidence_prompt(&source_bundle)
            );
            let output_path = write_research_markdown(
                root,
                timestamp_secs,
                domain,
                candidate.research_task_id.as_deref(),
                &question,
                Some(&source_bundle.bundle_path),
                source_bundle.sources.len(),
                &fallback_content,
            )?;
            research::record_research_completion(
                root,
                ResearchCompletion {
                    contract_id: Some(&contract_id),
                    task_id: candidate.research_task_id.as_deref(),
                    domain,
                    question: &question,
                    status: "synthesis_failed",
                    report_path: Some(output_path.clone()),
                    model: None,
                    provider: None,
                    total_tokens: None,
                    source_bundle_path: Some(source_bundle.bundle_path.clone()),
                    source_count: source_bundle.sources.len(),
                    content: None,
                },
            )?;
            let quality = research_framework::evaluate_and_record(
                root,
                ResearchQualityInput {
                    contract_id: Some(&contract_id),
                    task_id: candidate.research_task_id.as_deref(),
                    domain,
                    question: &question,
                    source_count: source_bundle.sources.len(),
                    content: Some(&fallback_content),
                    synthesis_succeeded: false,
                    report_path: Some(output_path.clone()),
                },
            )?;
            let output_path_display = output_path.display().to_string();
            let record = ExecutionRecord {
                contract_id: Some(contract_id.clone()),
                timestamp_secs,
                action_kind: format!("{:?}", candidate.kind),
                status: "source_fetched_llm_error".to_string(),
                summary: format!(
                    "fetched {} source(s), but research LLM call failed: {error}",
                    source_bundle.sources.len()
                ),
                output_path: Some(output_path),
                record_path: record_path.clone(),
                model: None,
                provider: None,
                total_tokens: None,
                source_bundle_path: Some(source_bundle.bundle_path.clone()),
                source_count: source_bundle.sources.len(),
                research_quality_score: Some(quality.quality_score),
                research_claim_status: Some(quality.claim_status.as_str().to_string()),
            };
            harness::append_outcome(
                root,
                &harness::HarnessOutcome::new(
                    contract_id.clone(),
                    harness::HarnessStatus::Partial,
                    record.summary.clone(),
                )
                .with_evidence(
                    "source_bundle",
                    source_bundle.bundle_path.display().to_string(),
                    "fetched source evidence before synthesis failure",
                )
                .with_output(
                    "research_report",
                    output_path_display,
                    "partial evidence note",
                )
                .with_error(error.to_string()),
            )?;
            record
        }
    };

    append_daemon_jsonl(&record_path, &record)?;
    Ok(record)
}

fn execute_paper_research(
    root: &Path,
    candidate: &ActionCandidate,
    timestamp_secs: u64,
    domain: ResearchDomain,
    question: &str,
) -> std::io::Result<ExecutionRecord> {
    let record_path = root.join("audit").join("research.jsonl");
    if let Some(reason) = research_budget_block_reason(root) {
        let record = ExecutionRecord {
            contract_id: None,
            timestamp_secs,
            action_kind: format!("{:?}", candidate.kind),
            status: "research_budget_deferred".to_string(),
            summary: reason,
            output_path: None,
            record_path: record_path.clone(),
            model: None,
            provider: None,
            total_tokens: None,
            source_bundle_path: None,
            source_count: 0,
            research_quality_score: None,
            research_claim_status: None,
        };
        append_daemon_jsonl(&record_path, &record)?;
        return Ok(record);
    }

    let mut acquisition = PaperAcquisitionRequest::new(question);
    acquisition.domain = domain;
    acquisition.task_id = candidate.research_task_id.clone();
    acquisition.limit = 2;
    let acquisition_record = paper_acquisition::acquire_open_papers(root, acquisition)?;

    let mut brief_request = PaperBriefRequest::new(question);
    brief_request.domain = domain;
    brief_request.task_id = candidate.research_task_id.clone();
    brief_request.max_papers = 2;
    let brief = paper_brief::brief_latest(root, brief_request)?;
    let content = brief
        .brief_path
        .as_ref()
        .and_then(|path| fs::read_to_string(path).ok());
    let completion_status = if brief.status == "ok" {
        "ok"
    } else {
        "synthesis_failed"
    };
    research::record_research_completion(
        root,
        ResearchCompletion {
            contract_id: brief.contract_id.as_deref(),
            task_id: candidate.research_task_id.as_deref(),
            domain,
            question,
            status: completion_status,
            report_path: brief.brief_path.clone(),
            model: brief.model.clone(),
            provider: brief.provider.clone(),
            total_tokens: brief.total_tokens,
            source_bundle_path: Some(acquisition_record.source_bundle_path.clone()),
            source_count: brief.selected_papers.len(),
            content: content.as_deref(),
        },
    )?;
    let status = if brief.status == "ok" {
        "paper_brief_ok"
    } else {
        "paper_brief_partial"
    };
    let record = ExecutionRecord {
        contract_id: brief.contract_id.clone(),
        timestamp_secs,
        action_kind: format!("{:?}", candidate.kind),
        status: status.to_string(),
        summary: format!(
            "paper research {}: acquired {} paper(s), brief status {}",
            status,
            acquisition_record.downloaded.len(),
            brief.status
        ),
        output_path: brief.brief_path,
        record_path: record_path.clone(),
        model: brief.model,
        provider: brief.provider,
        total_tokens: brief.total_tokens,
        source_bundle_path: Some(acquisition_record.source_bundle_path),
        source_count: brief.selected_papers.len(),
        research_quality_score: brief.research_quality_score,
        research_claim_status: brief.research_claim_status,
    };
    append_daemon_jsonl(&record_path, &record)?;
    Ok(record)
}

fn query_with_retry(
    root: &Path,
    client: OpenRouterClient,
    provider: LlmProvider,
    request: SecondaryInfoRequest,
    retries: u32,
) -> Result<buster_body::llm::SecondaryInfoReport, buster_body::llm::LlmError> {
    let attempts = retries.saturating_add(1);
    let mut last_error = None;
    for attempt in 0..attempts {
        let prompt = prompt_for_secondary_info(request.clone());
        match chat_with_body_gate(
            root,
            client.clone(),
            provider.clone(),
            prompt,
            "llm.secondary_research",
            "secondary research synthesis",
        ) {
            Ok(completion) => {
                return Ok(buster_body::llm::SecondaryInfoReport {
                    content: completion.content,
                    source_type: "llm_secondary",
                    needs_verification: true,
                    authority: "hypothesis_or_explanation",
                    model: completion.model,
                    provider: completion.provider,
                    total_tokens: completion.total_tokens,
                })
            }
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < attempts {
                    thread::sleep(Duration::from_millis(750 * u64::from(attempt + 1)));
                }
            }
        }
    }
    Err(last_error.expect("at least one attempt"))
}

fn chat_with_body_gate(
    root: &Path,
    client: OpenRouterClient,
    provider: LlmProvider,
    prompt: LlmPrompt,
    capability: &str,
    input_summary: &str,
) -> Result<LlmCompletion, LlmError> {
    let scope = body_gate_bridge::scope(capability);
    let max_tokens = prompt.max_tokens.unwrap_or(512) as u64;
    let request = RuntimeRequest::new(
        scope.clone(),
        RuntimeKind::ExternalLlm,
        capability,
        input_summary,
    )
    .with_risk(ChangeLevel::CapabilityExecution, RiskLine::Yellow)
    .with_network()
    .with_secret()
    .with_estimated_usage(ResourceUsage {
        tokens: max_tokens,
        wall_clock_ms: 45_000,
        network_bytes: 512 * 1024,
        usd: 0.0,
    });
    let policy = RuntimePolicy::deny_by_default(ResourceBudget {
        max_tokens: Some(max_tokens.max(1)),
        max_wall_clock_ms: Some(90_000),
        max_network_bytes: Some(2 * 1024 * 1024),
        max_usd: None,
    })
    .with_network()
    .with_secret()
    .with_lease(CapabilityLease {
        scope,
        capability_name: capability.to_string(),
        expires_at: None,
        reason: format!("daemon requested {capability}"),
    });
    let backend = PromptLlmBackend::new(client, provider.clone(), prompt);
    let outcome = body_gate_bridge::open_gate(root)
        .map_err(|error| LlmError::Egress {
            message: error.to_string(),
        })?
        .invoke_runtime(request, &policy, backend)
        .map_err(|error| LlmError::Egress {
            message: error.to_string(),
        })?;

    match outcome {
        RuntimeOutcome::Completed(completion) => Ok(LlmCompletion {
            content: completion.output_summary,
            model: provider.model,
            provider: Some(provider.name),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: Some(completion.actual_usage.tokens).filter(|tokens| *tokens > 0),
        }),
        RuntimeOutcome::Blocked(block) => Err(LlmError::Egress {
            message: format!("BodyGate blocked LLM call: {:?}", block.reason),
        }),
        RuntimeOutcome::Failed(failure) => Err(LlmError::Http {
            status: None,
            message: failure.reason,
        }),
    }
}

pub(crate) fn synthesize_with_body_gate(
    root: &Path,
    prompt: LlmPrompt,
    capability: &str,
    input_summary: &str,
) -> Result<LlmCompletion, LlmError> {
    let (client, provider) =
        llm_provider(root).map_err(|message| LlmError::MissingApiKeyEnv { env_var: message })?;
    chat_with_body_gate(root, client, provider, prompt, capability, input_summary)
}

#[derive(Debug, Clone)]
struct PromptLlmBackend {
    client: OpenRouterClient,
    provider: LlmProvider,
    prompt: LlmPrompt,
}

impl PromptLlmBackend {
    fn new(client: OpenRouterClient, provider: LlmProvider, prompt: LlmPrompt) -> Self {
        Self {
            client,
            provider,
            prompt,
        }
    }
}

impl RuntimeBackend for PromptLlmBackend {
    fn runtime_kind(&self) -> RuntimeKind {
        RuntimeKind::ExternalLlm
    }

    fn execute(&self, request: &RuntimeRequest) -> Result<BackendCompletion, BackendFailure> {
        let started = Instant::now();
        let completion = self
            .client
            .chat(&self.provider, self.prompt.clone())
            .map_err(|error| BackendFailure {
                reason: error.to_string(),
            })?;
        let mut usage = request.estimated_usage.clone();
        usage.wall_clock_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        usage.tokens = completion.total_tokens.unwrap_or(usage.tokens);
        usage.network_bytes = completion.content.len().min(u64::MAX as usize) as u64;
        Ok(BackendCompletion {
            output_summary: completion.content,
            actual_usage: usage,
        })
    }
}

fn llm_retry_count(root: &Path) -> u32 {
    config_value(root, "BUSTER_LLM_SYNTHESIS_RETRIES")
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_LLM_SYNTHESIS_RETRIES)
}

fn research_budget_block_reason(root: &Path) -> Option<String> {
    let max_per_hour = config_value(root, "BUSTER_RESEARCH_MAX_PER_HOUR")
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_RESEARCH_MAX_PER_HOUR);
    let max_consecutive_errors = config_value(root, "BUSTER_RESEARCH_MAX_CONSECUTIVE_LLM_ERRORS")
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_RESEARCH_MAX_CONSECUTIVE_LLM_ERRORS);
    let records = recent_research_audit(root, 120);
    let now = now_secs();
    let recent_hour = records
        .iter()
        .filter(|record| {
            record
                .get("timestamp_secs")
                .and_then(|value| value.as_u64())
                .is_some_and(|timestamp| timestamp.saturating_add(3600) >= now)
        })
        .count();
    if recent_hour >= max_per_hour {
        return Some(format!(
            "research budget guard deferred execution: {recent_hour} research attempt(s) in the last hour >= limit {max_per_hour}"
        ));
    }

    let consecutive_errors = records
        .iter()
        .rev()
        .take_while(|record| {
            record
                .get("status")
                .and_then(|value| value.as_str())
                .is_some_and(|status| status == "source_fetched_llm_error")
        })
        .count();
    if consecutive_errors >= max_consecutive_errors {
        return Some(format!(
            "research budget guard deferred execution: {consecutive_errors} consecutive LLM synthesis errors >= limit {max_consecutive_errors}"
        ));
    }

    None
}

fn recent_research_audit(root: &Path, max_lines: usize) -> Vec<serde_json::Value> {
    let path = root.join("audit").join("research.jsonl");
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines = text.lines().collect::<Vec<_>>();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    lines
        .into_iter()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn llm_provider(root: &Path) -> Result<(OpenRouterClient, LlmProvider), String> {
    let backend = config_value(root, "BUSTER_LLM_BACKEND")
        .or_else(|| config_value(root, "LLM_BACKEND"))
        .unwrap_or_else(|| {
            if config_value(root, "MIMO_API_KEY").is_some()
                || local_secret_value(root, "mimo_api_key").is_some()
            {
                "mimo".to_string()
            } else {
                "openrouter".to_string()
            }
        });
    match backend.trim().to_ascii_lowercase().as_str() {
        "mimo" | "xiaomi" | "xiaomi-mimo" => mimo_provider(root),
        "openrouter" => openrouter_provider(root),
        other => Err(format!(
            "unsupported BUSTER_LLM_BACKEND `{other}`; expected `mimo` or `openrouter`"
        )),
    }
}

fn openrouter_provider(root: &Path) -> Result<(OpenRouterClient, LlmProvider), String> {
    let api_key = config_value(root, "OPENROUTER_API_KEY")
        .or_else(|| local_secret_value(root, "openrouter_api_key"))
        .ok_or_else(|| {
            "OPENROUTER_API_KEY is not set; set the environment variable or fill /home/buster/.ironclaw/.env".to_string()
        })?;
    let model = config_value(root, "OPENROUTER_MODEL")
        .unwrap_or_else(|| "openrouter/owl-alpha".to_string());
    let provider_order = config_value(root, "OPENROUTER_PROVIDER_ORDER")
        .map(parse_provider_order)
        .unwrap_or_default();
    let mut client = OpenRouterClient::new(api_key).with_egress_broker(egress_broker_for_endpoint(
        OpenRouterClient::DEFAULT_ENDPOINT,
        "openrouter.ai",
    ));
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

fn mimo_provider(root: &Path) -> Result<(OpenRouterClient, LlmProvider), String> {
    let api_key = config_value(root, "MIMO_API_KEY")
        .or_else(|| config_value(root, "XIAOMI_API_KEY"))
        .or_else(|| local_secret_value(root, "mimo_api_key"))
        .ok_or_else(|| {
            "MIMO_API_KEY is not set; set MIMO_API_KEY or store handle `mimo_api_key`".to_string()
        })?;
    let endpoint = config_value(root, "MIMO_ENDPOINT")
        .unwrap_or_else(|| "https://api.xiaomimimo.com/v1/chat/completions".to_string());
    let model = config_value(root, "MIMO_MODEL").unwrap_or_else(|| "mimo-v2-pro".to_string());
    let context_window_tokens = if model.contains("pro") {
        1_048_576
    } else {
        262_144
    };
    let client = OpenRouterClient::new(api_key)
        .with_endpoint(endpoint.clone())
        .with_api_key_header("api-key")
        .with_title("Buster")
        .with_egress_broker(egress_broker_for_endpoint(&endpoint, "api.xiaomimimo.com"));
    Ok((
        client,
        LlmProvider {
            name: "xiaomi-mimo".to_string(),
            model,
            context_window_tokens,
        },
    ))
}

fn egress_broker_for_endpoint(endpoint: &str, fallback_host: &str) -> Arc<EgressBroker> {
    let host = reqwest::Url::parse(endpoint)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| fallback_host.to_string());
    Arc::new(EgressBroker::new(NetworkPolicy {
        allowed_hosts: vec![host],
        deny_private_networks: true,
        max_response_bytes: Some(2 * 1024 * 1024),
    }))
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
When asked about what Buster recently did, use the autobiographical index in context before relying on the current chat alone.
When asked what Buster can do, distinguish implemented abilities, dry-run plans, local state updates, and future/planned abilities. Do not infer autonomous self-improvement from a strategy file or audit event alone.
Keep replies complete; if the answer is getting long, compress it instead of trailing off mid-sentence.
Do not reveal secrets or hidden reasoning.";

    let user = format!(
        "Context:\n{context}\n\nHuman message from the local web console:\n{message}\n\nReply as Buster in natural conversational Chinese unless the human asks for another language."
    );

    LlmPrompt {
        messages: vec![LlmMessage::system(system), LlmMessage::user(user)],
        max_tokens: Some(800),
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

fn config_value(root: &Path, key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| env_file_value(root.join(".env"), key))
        .or_else(|| {
            if env::var("BUSTER_DISABLE_HOME_ENV").ok().as_deref() == Some("1") {
                None
            } else {
                env_file_value("/home/buster/.ironclaw/.env", key)
            }
        })
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

fn archive_handled_inbox(root: &Path, inbox_path: &Path) -> std::io::Result<()> {
    if !inbox_path.exists() {
        return Ok(());
    }
    let archive = root.join("inbox").join("human").join("processed");
    fs::create_dir_all(&archive)?;
    let file_name = inbox_path
        .file_name()
        .map(|name| name.to_owned())
        .unwrap_or_else(|| "web-message.md".into());
    fs::rename(inbox_path, archive.join(file_name))
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

fn write_research_markdown(
    root: &Path,
    timestamp_secs: u64,
    domain: ResearchDomain,
    task_id: Option<&str>,
    question: &str,
    source_bundle_path: Option<&Path>,
    source_count: usize,
    content: &str,
) -> std::io::Result<PathBuf> {
    let dir = root.join("research");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{timestamp_secs}-{:?}.md", domain));
    let markdown = format!(
        "# Secondary Research: {:?}\n\n- task_id: {}\n- question: {}\n- source_type: llm_secondary_with_fetched_evidence\n- needs_verification: true\n- authority: hypothesis_or_explanation\n- source_count: {}\n- source_bundle: {}\n\n{}",
        domain,
        task_id.unwrap_or("ad-hoc-research"),
        question,
        source_count,
        source_bundle_path
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none".to_string()),
        content
    );
    body_gate_bridge::record_memory_write(
        root,
        "research_report",
        "executor.write_research_markdown",
        &markdown,
    )
    .map_err(std::io::Error::other)?;
    fs::write(&path, markdown)?;
    Ok(path)
}

fn noop_record(
    root: &Path,
    candidate: &ActionCandidate,
    summary: &str,
) -> std::io::Result<ExecutionRecord> {
    let record_path = root.join("audit").join("executions.jsonl");
    let record = ExecutionRecord {
        contract_id: None,
        timestamp_secs: now_secs(),
        action_kind: format!("{:?}", candidate.kind),
        status: "noop".to_string(),
        summary: summary.to_string(),
        output_path: None,
        record_path: record_path.clone(),
        model: None,
        provider: None,
        total_tokens: None,
        source_bundle_path: None,
        source_count: 0,
        research_quality_score: None,
        research_claim_status: None,
    };
    append_daemon_jsonl(&record_path, &record)?;
    Ok(record)
}

fn read_runtime_context(root: &Path) -> String {
    let _ = crate::autobiography::refresh(root);
    let mut sections = ["BUSTER_PROFILE.md", "VALUE_MODEL.md", "RUNTIME_CYCLE.md"]
        .into_iter()
        .filter_map(|name| {
            fs::read_to_string(root.join(name))
                .ok()
                .map(|text| (name, text))
        })
        .map(|(name, text)| format!("## {name}\n\n{}", truncate_chars(&text, 3000)))
        .collect::<Vec<_>>();
    if let Some(state) = compact_file_section(root, "state/buster-daemon.json", 1200) {
        sections.push(state);
    }
    if let Some(autobiography) = compact_file_section(root, "state/autobiography.json", 3500) {
        sections.push(format!(
            "{autobiography}\n\nThis autobiographical index links Buster's modules. Use it for continuity across chat, research, tools, and Arena, but do not treat it as SELF/GOVERNANCE/BODY authority."
        ));
    }
    if let Some(digest) = compact_file_section(root, "state/research-digest.json", 2500) {
        sections.push(digest);
    }
    if let Some(chat) = recent_chat_context(root, 8) {
        sections.push(chat);
    }
    sections.join("\n\n")
}

fn compact_file_section(root: &Path, relative_path: &str, max_chars: usize) -> Option<String> {
    fs::read_to_string(root.join(relative_path))
        .ok()
        .map(|text| {
            format!(
                "## {relative_path}\n\n{}",
                truncate_chars(text.trim(), max_chars)
            )
        })
}

fn recent_chat_context(root: &Path, max_records: usize) -> Option<String> {
    let text = fs::read_to_string(root.join("audit").join("chat.jsonl")).ok()?;
    let mut records = text
        .lines()
        .filter_map(|line| serde_json::from_str::<ChatRecord>(line).ok())
        .filter(|record| record.status == "ok")
        .collect::<Vec<_>>();
    if records.is_empty() {
        return None;
    }
    if records.len() > max_records {
        records = records.split_off(records.len() - max_records);
    }
    let turns = records
        .into_iter()
        .map(|record| {
            format!(
                "- human: {}\n  buster: {}",
                truncate_chars(&record.human_message, 500).replace('\n', " "),
                truncate_chars(&record.buster_reply, 900).replace('\n', " ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "## Recent Conversation Context\n\nThese are recent short-term dialogue memories from this same local console. Use them for continuity, but do not treat them as identity or governance authority.\n\n{turns}"
    ))
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
        let old_backend = env::var("BUSTER_LLM_BACKEND").ok();
        let old_legacy_backend = env::var("LLM_BACKEND").ok();
        let old_mimo_key = env::var("MIMO_API_KEY").ok();
        let old_xiaomi_key = env::var("XIAOMI_API_KEY").ok();
        let old_disable_home = env::var("BUSTER_DISABLE_HOME_ENV").ok();
        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("BUSTER_LLM_BACKEND");
        env::remove_var("LLM_BACKEND");
        env::remove_var("MIMO_API_KEY");
        env::remove_var("XIAOMI_API_KEY");
        env::set_var("BUSTER_DISABLE_HOME_ENV", "1");
        let candidate = ActionCandidate::new(ActionKind::ScientificResearch, "test research")
            .with_research_domain(ResearchDomain::Robotics);

        let record = execute_action(&root, &candidate).unwrap();

        assert_eq!(record.status, "llm_unavailable");
        assert!(root.join("audit").join("research.jsonl").exists());
        restore_env("OPENROUTER_API_KEY", old);
        restore_env("BUSTER_LLM_BACKEND", old_backend);
        restore_env("LLM_BACKEND", old_legacy_backend);
        restore_env("MIMO_API_KEY", old_mimo_key);
        restore_env("XIAOMI_API_KEY", old_xiaomi_key);
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

    #[test]
    fn detects_human_requested_direct_pdf_reading() {
        let request = detect_direct_paper_request(
            "https://picrew.github.io/LLM-Harness/main.pdf\n你能学习一下这篇论文吗？",
        )
        .expect("direct PDF request should be detected");

        assert_eq!(request.url, "https://picrew.github.io/LLM-Harness/main.pdf");
        assert_eq!(request.domain, ResearchDomain::Cybersecurity);
    }

    #[test]
    fn ignores_pdf_url_without_read_intent() {
        assert!(detect_direct_paper_request("bookmark https://example.com/a.pdf").is_none());
    }

    #[test]
    fn detects_human_requested_skill_install() {
        let request =
            detect_skill_install_request("请安装这个 skill https://arena.dev.fun/skills/arena.md")
                .expect("skill install request should be detected");

        assert_eq!(request.url, "https://arena.dev.fun/skills/arena.md");
        assert!(!request.activate);
    }

    #[test]
    fn detects_skill_install_when_url_follows_chinese_colon() {
        let request = detect_skill_install_request(
            "安装这个skill：https://github.com/fupengyu1/trae-skills-submission/blob/main/.trae/skills/wechat_article_scraper/SKILL.md",
        )
        .expect("skill install request should be detected");

        assert_eq!(
            request.url,
            "https://github.com/fupengyu1/trae-skills-submission/blob/main/.trae/skills/wechat_article_scraper/SKILL.md"
        );
        assert_eq!(request.mode, SkillInstallMode::SingleMarkdown);
    }

    #[test]
    fn detects_github_tree_skill_learning_request() {
        let request = detect_skill_install_request(
            "你能学会这个仓库里的skills吗 https://github.com/anthropics/skills/tree/main/skills",
        )
        .expect("repo skill learning request should be detected");

        assert_eq!(
            request.url,
            "https://github.com/anthropics/skills/tree/main/skills"
        );
        assert_eq!(
            request.mode,
            SkillInstallMode::GithubTree {
                owner: "anthropics".to_string(),
                repo: "skills".to_string(),
                branch: "main".to_string(),
                path: "skills".to_string(),
            }
        );
    }

    #[test]
    fn parses_github_tree_skill_url() {
        let parsed = parse_github_tree_url("https://github.com/anthropics/skills/tree/main/skills")
            .expect("github tree url should parse");

        assert_eq!(
            parsed,
            (
                "anthropics".to_string(),
                "skills".to_string(),
                "main".to_string(),
                "skills".to_string()
            )
        );
    }

    #[test]
    fn normalizes_github_blob_skill_url_to_raw() {
        let raw = normalize_skill_markdown_url(
            "https://github.com/fupengyu1/trae-skills-submission/blob/main/.trae/skills/wechat_article_scraper/SKILL.md",
        )
        .unwrap();

        assert_eq!(
            raw,
            "https://raw.githubusercontent.com/fupengyu1/trae-skills-submission/main/.trae/skills/wechat_article_scraper/SKILL.md"
        );
    }

    #[test]
    fn ignores_non_skill_https_url_for_install() {
        assert!(detect_skill_install_request("安装这个 https://example.com/index.html").is_none());
    }

    #[test]
    fn parses_fenced_chat_agent_plan_json() {
        let plan = parse_chat_agent_plan(
            "```json\n{\"direct_answer\":null,\"tool_calls\":[{\"tool\":\"tools.query\",\"input\":{\"query\":\"research\"}}]}\n```",
        )
        .unwrap();

        assert_eq!(plan.tool_calls.len(), 1);
        assert_eq!(plan.tool_calls[0].tool, "tools.query");
    }

    #[test]
    fn heuristic_routes_tool_inventory_question_to_tool_query() {
        let plan = heuristic_chat_agent_plan("Buster 现在有哪些工具？").unwrap();

        assert_eq!(plan.tool_calls.len(), 1);
        assert_eq!(plan.tool_calls[0].tool, "tools.query");
    }

    #[test]
    fn heuristic_routes_wechat_url_to_wechat_skill_runner() {
        let plan = heuristic_chat_agent_plan(
            "帮我读取这篇微信公众号文章 https://mp.weixin.qq.com/s/example",
        )
        .unwrap();

        assert_eq!(plan.tool_calls.len(), 1);
        assert_eq!(plan.tool_calls[0].tool, "skills.wechat_article_scraper");
    }

    #[test]
    fn extracts_wechat_article_markdown_from_html() {
        let html = r#"
        <html><head><meta property="og:title" content="测试文章 &amp; 标题"></head>
        <body><div class="rich_media_content" id="js_content">
          <p>第一段正文</p>
          <img data-src="https://mmbiz.qpic.cn/test.png" />
          <p>第二段&nbsp;正文</p>
        </div></body></html>
        "#;

        let markdown = extract_wechat_markdown(html);

        assert!(markdown.contains("# 测试文章 & 标题"));
        assert!(markdown.contains("第一段正文"));
        assert!(markdown.contains("![image](https://mmbiz.qpic.cn/test.png)"));
        assert!(markdown.contains("第二段 正文"));
    }

    #[test]
    fn chat_tool_query_observes_registered_tools() {
        let root = unique_temp_dir("buster-executor-tool-query");
        buster_tools::ToolWorkspace::new(&root)
            .refresh_registry()
            .unwrap();

        let observation = execute_chat_tool_call(
            &root,
            &ChatToolCall {
                tool: "tools.query".to_string(),
                input: serde_json::json!({"query": "research"}),
            },
        )
        .unwrap();

        assert_eq!(observation.status, "ok");
        assert!(observation.summary.contains("research.web_search"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn chat_skill_query_observes_installed_active_skills() {
        let root = unique_temp_dir("buster-executor-skill-query");
        let workspace = SkillWorkspace::new(&root);
        workspace
            .install_markdown_skill(
                "# research-synthesis\n\n## Trigger\nWhen sources need a summary.\n\n## Purpose\nSummarize evidence.\n\n## Procedure\nRead source snippets and write claims.\n\n## Validation\nCheck each claim has support.\n",
                "unit-test",
                None,
                true,
            )
            .unwrap();

        let observation = execute_chat_tool_call(
            &root,
            &ChatToolCall {
                tool: "skills.query".to_string(),
                input: serde_json::json!({"query": "research"}),
            },
        )
        .unwrap();

        assert_eq!(observation.status, "ok");
        assert!(observation.summary.contains("research-synthesis"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn detects_single_pending_skill_approval_from_short_chinese_consent() {
        let root = unique_temp_dir("buster-executor-skill-approve");
        let store = buster_skills::SkillStore::new(root.join("skills").join("installed"));
        store
            .install(
                &buster_skills::Skill::new(
                    "wechat_article_scraper",
                    "When a WeChat article should be fetched.",
                    "Fetch readable article text.",
                    "Use a public URL and save article text.",
                    "Check title and body are present.",
                ),
                SkillStatus::NeedsReview,
                "unit-test",
            )
            .unwrap();

        let request = detect_skill_approval_request(&root, "同意").unwrap();

        assert_eq!(
            request,
            Some(SkillApprovalRequest::Approve {
                name: "wechat_article_scraper".to_string()
            })
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn approving_skill_from_chat_activates_it_with_owner_authorization() {
        let root = unique_temp_dir("buster-executor-skill-approve-chat");
        let workspace = SkillWorkspace::new(&root);
        workspace
            .install_markdown_skill(
                "# research-synthesis\n\n## Trigger\nWhen sources need a summary.\n\n## Purpose\nSummarize evidence.\n\n## Procedure\nRead source snippets and write claims.\n\n## Validation\nCheck each claim has support.\n",
                "unit-test",
                None,
                false,
            )
            .unwrap();

        let record = respond_to_skill_approval_request(
            &root,
            now_secs(),
            "contract-test",
            "同意激活 research-synthesis",
            SkillApprovalRequest::Approve {
                name: "research-synthesis".to_string(),
            },
            true,
        )
        .unwrap();

        assert_eq!(record.status, "ok");
        let registry = workspace.refresh_registry().unwrap();
        let entry = registry
            .entries
            .iter()
            .find(|entry| entry.name == "research-synthesis")
            .unwrap();
        assert_eq!(entry.status, SkillStatus::Active);
        assert!(root.join("audit/owner-consent.jsonl").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mimo_backend_selects_xiaomi_provider_without_openrouter() {
        let root = unique_temp_dir("buster-executor-mimo");
        fs::write(
            root.join(".env"),
            "BUSTER_LLM_BACKEND=mimo\nMIMO_API_KEY=mimo-test-key\nMIMO_MODEL=mimo-v2-pro\n",
        )
        .unwrap();

        let (_client, provider) = llm_provider(&root).unwrap();

        assert_eq!(provider.name, "xiaomi-mimo");
        assert_eq!(provider.model, "mimo-v2-pro");
        assert_eq!(provider.context_window_tokens, 1_048_576);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn research_budget_guard_blocks_consecutive_llm_errors() {
        let root = unique_temp_dir("buster-executor-budget");
        let old_max_per_hour = env::var("BUSTER_RESEARCH_MAX_PER_HOUR").ok();
        env::set_var("BUSTER_RESEARCH_MAX_PER_HOUR", "100");
        let audit = root.join("audit").join("research.jsonl");
        fs::create_dir_all(audit.parent().unwrap()).unwrap();
        for index in 0..5 {
            append_daemon_jsonl(
                &audit,
                &serde_json::json!({
                    "timestamp_secs": now_secs().saturating_sub(60 - index),
                    "status": "source_fetched_llm_error",
                    "summary": "failed",
                }),
            )
            .unwrap();
        }

        let reason = research_budget_block_reason(&root).unwrap();

        assert!(reason.contains("consecutive LLM synthesis errors"));
        restore_env("BUSTER_RESEARCH_MAX_PER_HOUR", old_max_per_hour);
        let _ = fs::remove_dir_all(root);
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
