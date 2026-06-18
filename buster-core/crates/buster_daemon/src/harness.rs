//! Harness contracts for Buster actions.
//!
//! The harness is a small Plan/Execute/Verify envelope. It does not replace
//! BodyGate; it records what an action was allowed to try and how it ended.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use buster_value_model::{ActionCandidate, ActionKind, ResearchDomain};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{body_gate_bridge, now_secs};

pub const CONTRACT_AUDIT_PATH: &str = "audit/harness-contracts.jsonl";
pub const OUTCOME_AUDIT_PATH: &str = "audit/harness-outcomes.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessTrigger {
    HumanRequest,
    RuntimeCycle,
    ResearchQueueTask,
    ScheduledTick,
    SelfReview,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessRiskLevel {
    Low,
    Medium,
    High,
    Emergency,
    IdentityCritical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessStatus {
    Completed,
    Partial,
    Failed,
    Deferred,
    Unsafe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceBudgetHint {
    pub max_tokens: Option<u64>,
    pub max_wall_clock_ms: Option<u64>,
    pub max_network_bytes: Option<u64>,
    pub max_usd: Option<f64>,
}

impl ResourceBudgetHint {
    pub fn low_llm() -> Self {
        Self {
            max_tokens: Some(1_000),
            max_wall_clock_ms: Some(90_000),
            max_network_bytes: Some(2 * 1024 * 1024),
            max_usd: None,
        }
    }

    pub fn runtime_tick() -> Self {
        Self {
            max_tokens: Some(2_000),
            max_wall_clock_ms: Some(120_000),
            max_network_bytes: Some(4 * 1024 * 1024),
            max_usd: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessRef {
    pub kind: String,
    pub reference: String,
    pub purpose: String,
}

impl HarnessRef {
    pub fn new(
        kind: impl Into<String>,
        reference: impl Into<String>,
        purpose: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            reference: reference.into(),
            purpose: purpose.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRequirement {
    pub name: String,
    pub capability: String,
    pub action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRequirement {
    pub handle: String,
    pub purpose: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkScope {
    pub allowed_hosts: Vec<String>,
    pub methods: Vec<String>,
    pub purpose: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessStep {
    pub step_id: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRequirement {
    pub kind: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryPolicy {
    pub observation: String,
    pub fact: String,
    pub experience: String,
    pub immune: String,
    pub profile: String,
    pub identity_value: String,
}

impl MemoryPolicy {
    pub fn level_0_to_2_only() -> Self {
        Self {
            observation: "allowed".to_string(),
            fact: "allowed_when_source_traceable".to_string(),
            experience: "allowed_when_outcome_known".to_string(),
            immune: "bodygate_only".to_string(),
            profile: "forbidden".to_string(),
            identity_value: "forbidden".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailurePolicy {
    pub retry_limit: u32,
    pub stop_condition: String,
    pub downgrade_path: String,
    pub follow_up_spawn_rule: String,
}

impl FailurePolicy {
    pub fn no_failed_followups() -> Self {
        Self {
            retry_limit: 0,
            stop_condition: "BodyGate block, missing secret, exhausted budget, or unsafe output"
                .to_string(),
            downgrade_path: "record failure or partial evidence; do not hide it".to_string(),
            follow_up_spawn_rule: "only successful synthesized research may spawn follow-up tasks"
                .to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessContract {
    pub contract_id: String,
    pub created_at_secs: u64,
    pub owner: String,
    pub trigger: HarnessTrigger,
    pub goal: String,
    pub action_kind: String,
    pub change_level: u8,
    pub risk_level: HarnessRiskLevel,
    pub resource_budget: ResourceBudgetHint,
    pub read_set: Vec<HarnessRef>,
    pub write_set: Vec<HarnessRef>,
    pub required_tools: Vec<ToolRequirement>,
    pub required_secrets: Vec<SecretRequirement>,
    pub network_scope: Vec<NetworkScope>,
    pub plan: Vec<HarnessStep>,
    pub verification: Vec<HarnessStep>,
    pub rollback: String,
    pub evidence_required: Vec<EvidenceRequirement>,
    pub memory_policy: MemoryPolicy,
    pub failure_policy: FailurePolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessOutcome {
    pub contract_id: String,
    pub completed_at_secs: u64,
    pub status: HarnessStatus,
    pub summary: String,
    pub evidence: Vec<HarnessRef>,
    pub output_refs: Vec<HarnessRef>,
    pub error: Option<String>,
    pub memory_write_refs: Vec<HarnessRef>,
}

impl HarnessOutcome {
    pub fn new(
        contract_id: impl Into<String>,
        status: HarnessStatus,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            contract_id: contract_id.into(),
            completed_at_secs: now_secs(),
            status,
            summary: summary.into(),
            evidence: Vec::new(),
            output_refs: Vec::new(),
            error: None,
            memory_write_refs: Vec::new(),
        }
    }

    pub fn with_output(mut self, kind: &str, reference: impl Into<String>, purpose: &str) -> Self {
        self.output_refs
            .push(HarnessRef::new(kind, reference, purpose));
        self
    }

    pub fn with_evidence(
        mut self,
        kind: &str,
        reference: impl Into<String>,
        purpose: &str,
    ) -> Self {
        self.evidence
            .push(HarnessRef::new(kind, reference, purpose));
        self
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }
}

pub fn runtime_cycle_contract(
    tick_index: usize,
    timestamp_secs: u64,
    candidate: &ActionCandidate,
) -> HarnessContract {
    let mut contract = base_contract(
        timestamp_secs,
        HarnessTrigger::RuntimeCycle,
        format!("run daemon tick {tick_index} and select one bounded action"),
        candidate,
        HarnessRiskLevel::Low,
        ResourceBudgetHint::runtime_tick(),
    );
    contract.read_set = vec![
        HarnessRef::new(
            "state",
            "body supervisor report",
            "observe body mode and signals",
        ),
        HarnessRef::new("state", "value context", "score candidate activities"),
    ];
    contract.write_set = vec![
        HarnessRef::new("audit", "audit/runtime-cycle.jsonl", "runtime cycle record"),
        HarnessRef::new(
            "audit",
            "audit/value-episodes.jsonl",
            "value episode record",
        ),
        HarnessRef::new(
            "state",
            "state/buster-daemon.json",
            "latest daemon state snapshot",
        ),
    ];
    contract.plan = vec![
        step("observe", "read body state and context pressure"),
        step("decide", "score candidates through the runtime cycle"),
        step(
            "record",
            "write audit, episode, signed event, and state snapshot",
        ),
    ];
    contract.verification = vec![step(
        "audit-written",
        "runtime-cycle and value-episode records include this contract id",
    )];
    contract
}

pub fn human_chat_contract(timestamp_secs: u64, message: &str) -> HarnessContract {
    let candidate = ActionCandidate::new(ActionKind::HumanDialogue, "respond to human web chat");
    let mut contract = base_contract(
        timestamp_secs,
        HarnessTrigger::HumanRequest,
        format!(
            "respond to human dialogue without revealing secrets or hidden reasoning: {}",
            one_line(message, 160)
        ),
        &candidate,
        HarnessRiskLevel::Low,
        ResourceBudgetHint::low_llm(),
    );
    contract.read_set = vec![
        HarnessRef::new("document", "BUSTER_PROFILE.md", "runtime identity context"),
        HarnessRef::new("document", "VALUE_MODEL.md", "value-model context"),
        HarnessRef::new("document", "RUNTIME_CYCLE.md", "runtime-cycle context"),
    ];
    contract.write_set = vec![
        HarnessRef::new("inbox", "inbox/human", "archive incoming human message"),
        HarnessRef::new("audit", "audit/chat.jsonl", "chat audit record"),
    ];
    contract.required_tools = vec![ToolRequirement {
        name: "llm.chat".to_string(),
        capability: "llm.human_chat".to_string(),
        action: Some("complete".to_string()),
    }];
    contract.required_secrets = vec![SecretRequirement {
        handle: "llm_api_key".to_string(),
        purpose: "call configured LLM provider through BodyGate".to_string(),
    }];
    contract.network_scope = vec![NetworkScope {
        allowed_hosts: vec!["configured_llm_endpoint".to_string()],
        methods: vec!["POST".to_string()],
        purpose: "single LLM completion for the human reply".to_string(),
    }];
    contract.plan = vec![
        step("archive-inbox", "store incoming message in the human inbox"),
        step("llm", "ask configured LLM through BodyGate"),
        step("audit", "write chat audit and signed organism event"),
    ];
    contract.verification = vec![step(
        "natural-reply",
        "reply is concise, does not expose secrets, and is recorded in chat audit",
    )];
    contract
}

pub fn research_contract(
    timestamp_secs: u64,
    candidate: &ActionCandidate,
    domain: ResearchDomain,
    question: &str,
) -> HarnessContract {
    let mut contract = base_contract(
        timestamp_secs,
        HarnessTrigger::ResearchQueueTask,
        format!("research {:?}: {}", domain, one_line(question, 200)),
        candidate,
        HarnessRiskLevel::Medium,
        ResourceBudgetHint {
            max_tokens: Some(1_500),
            max_wall_clock_ms: Some(180_000),
            max_network_bytes: Some(4 * 1024 * 1024),
            max_usd: None,
        },
    );
    contract.read_set = vec![
        HarnessRef::new(
            "queue",
            "state/research-queue.json",
            "select current research task",
        ),
        HarnessRef::new("document", "BUSTER_PROFILE.md", "runtime identity context"),
        HarnessRef::new("document", "VALUE_MODEL.md", "value-model context"),
    ];
    contract.write_set = vec![
        HarnessRef::new("audit", "audit/research.jsonl", "execution record"),
        HarnessRef::new(
            "audit",
            "audit/research-ledger.jsonl",
            "research memory ledger",
        ),
        HarnessRef::new("state", "state/research-digest.json", "research digest"),
        HarnessRef::new(
            "research",
            "research/*.md",
            "research report or partial evidence note",
        ),
    ];
    contract.required_tools = vec![
        ToolRequirement {
            name: "source_fetcher".to_string(),
            capability: "network.research_source_fetch".to_string(),
            action: Some("fetch_bundle".to_string()),
        },
        ToolRequirement {
            name: "llm.chat".to_string(),
            capability: "llm.secondary_research".to_string(),
            action: Some("synthesize".to_string()),
        },
    ];
    contract.required_secrets = vec![SecretRequirement {
        handle: "llm_api_key".to_string(),
        purpose: "call configured LLM provider through BodyGate".to_string(),
    }];
    contract.network_scope = vec![NetworkScope {
        allowed_hosts: vec![
            "export.arxiv.org".to_string(),
            "api.crossref.org".to_string(),
            "configured_llm_endpoint".to_string(),
        ],
        methods: vec!["GET".to_string(), "POST".to_string()],
        purpose: "fetch verifiable sources and synthesize a bounded research note".to_string(),
    }];
    contract.plan = vec![
        step(
            "budget",
            "check research budget and consecutive-error guard",
        ),
        step("sources", "fetch source bundle before synthesis"),
        step("synthesize", "ask LLM for a compact research-chain note"),
        step(
            "record",
            "write report, ledger, digest, and execution audit",
        ),
    ];
    contract.verification = vec![
        step(
            "sources-counted",
            "source count and bundle path are recorded",
        ),
        step(
            "followups-gated",
            "only successful synthesis may spawn follow-up research tasks",
        ),
    ];
    contract.evidence_required = vec![
        EvidenceRequirement {
            kind: "source_bundle".to_string(),
            description: "Fetched source bundle path and source count".to_string(),
            required: true,
        },
        EvidenceRequirement {
            kind: "research_ledger".to_string(),
            description: "Research ledger entry carrying this contract id".to_string(),
            required: true,
        },
    ];
    contract.failure_policy.retry_limit = 2;
    contract
}

pub fn paperqa_contract(
    timestamp_secs: u64,
    domain: ResearchDomain,
    question: &str,
) -> HarnessContract {
    let candidate = ActionCandidate::new(
        ActionKind::ScientificResearch,
        format!(
            "PaperQA literature QA for {:?}: {}",
            domain,
            one_line(question, 160)
        ),
    );
    let mut contract = base_contract(
        timestamp_secs,
        HarnessTrigger::Manual,
        format!(
            "run PaperQA over Buster's bounded local paper corpus: {}",
            one_line(question, 200)
        ),
        &candidate,
        HarnessRiskLevel::Medium,
        ResourceBudgetHint {
            max_tokens: Some(8_000),
            max_wall_clock_ms: Some(240_000),
            max_network_bytes: Some(16 * 1024 * 1024),
            max_usd: None,
        },
    );
    contract.read_set = vec![
        HarnessRef::new(
            "paper_corpus",
            "research/paperqa/papers",
            "local PDFs/text files available for PaperQA indexing",
        ),
        HarnessRef::new(
            "state",
            "research/paperqa/.pqa",
            "PaperQA local index/cache scoped to this workspace",
        ),
    ];
    contract.write_set = vec![
        HarnessRef::new("audit", "audit/paperqa.jsonl", "PaperQA execution record"),
        HarnessRef::new(
            "research",
            "research/paperqa/reports/*.md",
            "PaperQA answer and stderr tail",
        ),
        HarnessRef::new(
            "state",
            "state/research-quality-digest.json",
            "research quality update",
        ),
    ];
    contract.required_tools = vec![ToolRequirement {
        name: "research.paperqa".to_string(),
        capability: "research.paperqa".to_string(),
        action: Some("ask".to_string()),
    }];
    contract.required_secrets = vec![SecretRequirement {
        handle: "llm_api_key".to_string(),
        purpose: "allow PaperQA/LiteLLM to call the configured literature QA model".to_string(),
    }];
    contract.network_scope = vec![NetworkScope {
        allowed_hosts: vec![
            "configured_llm_endpoint".to_string(),
            "api.crossref.org".to_string(),
            "api.semanticscholar.org".to_string(),
            "api.openalex.org".to_string(),
            "api.unpaywall.org".to_string(),
        ],
        methods: vec!["GET".to_string(), "POST".to_string()],
        purpose: "metadata lookup, LLM completion, and citation-grounded literature QA".to_string(),
    }];
    contract.plan = vec![
        step(
            "layout",
            "ensure PaperQA paper, cache, report, and audit directories exist",
        ),
        step(
            "gate",
            "invoke PaperQA through BodyGate with network and secret leases",
        ),
        step(
            "report",
            "write bounded answer report and PaperQA audit record",
        ),
        step(
            "quality",
            "score the answer through ResearchQuality before memory use",
        ),
    ];
    contract.verification = vec![
        step(
            "bounded-corpus",
            "PaperQA current directory is research/paperqa/papers",
        ),
        step(
            "audit",
            "audit/paperqa.jsonl carries status, command, report path, and quality score",
        ),
        step(
            "no-secret-output",
            "known secret values are redacted from captured output",
        ),
    ];
    contract.evidence_required = vec![
        EvidenceRequirement {
            kind: "paperqa_report".to_string(),
            description: "Report path containing the PaperQA answer or failure evidence"
                .to_string(),
            required: false,
        },
        EvidenceRequirement {
            kind: "paperqa_audit".to_string(),
            description: "PaperQA audit record carrying this contract id".to_string(),
            required: true,
        },
    ];
    contract
}

pub fn paper_acquisition_contract(
    timestamp_secs: u64,
    domain: ResearchDomain,
    question: &str,
) -> HarnessContract {
    let candidate = ActionCandidate::new(
        ActionKind::ScientificResearch,
        format!(
            "Acquire open papers for {:?}: {}",
            domain,
            one_line(question, 160)
        ),
    );
    let mut contract = base_contract(
        timestamp_secs,
        HarnessTrigger::Manual,
        format!(
            "discover and download legal open papers for PaperQA: {}",
            one_line(question, 200)
        ),
        &candidate,
        HarnessRiskLevel::Medium,
        ResourceBudgetHint {
            max_tokens: Some(0),
            max_wall_clock_ms: Some(180_000),
            max_network_bytes: Some(64 * 1024 * 1024),
            max_usd: None,
        },
    );
    contract.read_set = vec![HarnessRef::new(
        "network",
        "arXiv/OpenAlex/Crossref metadata",
        "find open paper candidates without bypassing access controls",
    )];
    contract.write_set = vec![
        HarnessRef::new(
            "paper_corpus",
            "research/paperqa/papers",
            "open PDFs available for PaperQA indexing",
        ),
        HarnessRef::new(
            "manifest",
            "research/paperqa/manifest.jsonl",
            "source, hash, and license/access provenance",
        ),
        HarnessRef::new(
            "audit",
            "audit/paper-acquisition.jsonl",
            "paper acquisition audit",
        ),
    ];
    contract.required_tools = vec![
        ToolRequirement {
            name: "research.source_fetch".to_string(),
            capability: "research.source_fetch".to_string(),
            action: Some("fetch_bundle".to_string()),
        },
        ToolRequirement {
            name: "research.paper_acquire".to_string(),
            capability: "research.paper_acquire".to_string(),
            action: Some("download_open_pdf".to_string()),
        },
    ];
    contract.network_scope = vec![NetworkScope {
        allowed_hosts: vec![
            "export.arxiv.org".to_string(),
            "arxiv.org".to_string(),
            "api.openalex.org".to_string(),
            "api.crossref.org".to_string(),
        ],
        methods: vec!["GET".to_string()],
        purpose: "discover source metadata and download only recognized open PDFs".to_string(),
    }];
    contract.plan = vec![
        step(
            "discover",
            "fetch source metadata for the research question",
        ),
        step("resolve", "accept only recognized open full-text URLs"),
        step(
            "download",
            "download bounded-size PDFs through BodyGate network preflight",
        ),
        step(
            "manifest",
            "record source URL, PDF URL, file hash, and audit entry",
        ),
    ];
    contract.verification = vec![
        step(
            "open-only",
            "skipped sources explain why no open PDF was downloaded",
        ),
        step("hash", "every downloaded PDF has sha256 and byte count"),
        step(
            "corpus",
            "downloaded files are under research/paperqa/papers",
        ),
    ];
    contract.evidence_required = vec![
        EvidenceRequirement {
            kind: "paper_manifest".to_string(),
            description: "Manifest records for downloaded papers".to_string(),
            required: false,
        },
        EvidenceRequirement {
            kind: "paper_acquisition_audit".to_string(),
            description: "Audit record carrying downloaded/skipped/error counts".to_string(),
            required: true,
        },
    ];
    contract
}

pub fn web_search_contract(timestamp_secs: u64, query: &str) -> HarnessContract {
    let candidate = ActionCandidate::new(
        ActionKind::SecondaryResearch,
        format!("Search public web leads: {}", one_line(query, 160)),
    );
    let mut contract = base_contract(
        timestamp_secs,
        HarnessTrigger::Manual,
        format!(
            "search public web metadata through DuckDuckGo HTML for evidence leads: {}",
            one_line(query, 200)
        ),
        &candidate,
        HarnessRiskLevel::Low,
        ResourceBudgetHint {
            max_tokens: Some(0),
            max_wall_clock_ms: Some(30_000),
            max_network_bytes: Some(2 * 1024 * 1024),
            max_usd: None,
        },
    );
    contract.read_set = vec![HarnessRef::new(
        "network",
        "DuckDuckGo non-JavaScript search results page",
        "collect public search-result metadata without account credentials",
    )];
    contract.write_set = vec![
        HarnessRef::new(
            "web_search_bundle",
            "research/web/search/*.json",
            "bounded web-search evidence bundle",
        ),
        HarnessRef::new("audit", "audit/web-search.jsonl", "web search audit"),
    ];
    contract.required_tools = vec![ToolRequirement {
        name: "research.web_search".to_string(),
        capability: "research.web_search".to_string(),
        action: Some("search".to_string()),
    }];
    contract.network_scope = vec![NetworkScope {
        allowed_hosts: vec![
            "duckduckgo.com".to_string(),
            "html.duckduckgo.com".to_string(),
        ],
        methods: vec!["GET".to_string()],
        purpose: "retrieve key-free public search result metadata as untrusted leads".to_string(),
    }];
    contract.plan = vec![
        step(
            "preflight",
            "check DuckDuckGo URL through BodyGate network policy",
        ),
        step(
            "search",
            "retrieve bounded non-JavaScript search result HTML",
        ),
        step(
            "parse",
            "extract title, URL, and snippet without executing page content",
        ),
        step("record", "write bundle and audit entry"),
    ];
    contract.verification = vec![
        step("bounded", "result count is clamped to 1-10"),
        step(
            "untrusted",
            "result text is marked as untrusted evidence, not executable instructions",
        ),
        step(
            "audit",
            "audit/web-search.jsonl carries the query and result count",
        ),
    ];
    contract.evidence_required = vec![EvidenceRequirement {
        kind: "web_search_audit".to_string(),
        description: "Audit record carrying query, provider, result count, and errors".to_string(),
        required: true,
    }];
    contract
}

pub fn paper_brief_contract(
    timestamp_secs: u64,
    domain: ResearchDomain,
    question: &str,
) -> HarnessContract {
    let candidate = ActionCandidate::new(
        ActionKind::ScientificResearch,
        format!(
            "Read extracted papers for {:?}: {}",
            domain,
            one_line(question, 160)
        ),
    );
    let mut contract = base_contract(
        timestamp_secs,
        HarnessTrigger::Manual,
        format!(
            "synthesize a bounded paper brief from extracted open-paper text: {}",
            one_line(question, 200)
        ),
        &candidate,
        HarnessRiskLevel::Medium,
        ResourceBudgetHint {
            max_tokens: Some(3_000),
            max_wall_clock_ms: Some(120_000),
            max_network_bytes: Some(2 * 1024 * 1024),
            max_usd: None,
        },
    );
    contract.read_set = vec![
        HarnessRef::new(
            "manifest",
            "research/paperqa/manifest.jsonl",
            "select traceable acquired papers",
        ),
        HarnessRef::new(
            "extracted_text",
            "research/paperqa/extracted",
            "bounded text extracted from open PDFs",
        ),
    ];
    contract.write_set = vec![
        HarnessRef::new(
            "research",
            "research/paperqa/briefs/*.md",
            "paper reading brief",
        ),
        HarnessRef::new("audit", "audit/paper-brief.jsonl", "paper brief audit"),
        HarnessRef::new(
            "state",
            "state/research-quality-digest.json",
            "research quality update",
        ),
    ];
    contract.required_tools = vec![
        ToolRequirement {
            name: "research.paper_brief".to_string(),
            capability: "research.paper_brief".to_string(),
            action: Some("brief".to_string()),
        },
        ToolRequirement {
            name: "llm.chat".to_string(),
            capability: "research.paper_brief".to_string(),
            action: Some("synthesize".to_string()),
        },
    ];
    contract.required_secrets = vec![SecretRequirement {
        handle: "llm_api_key".to_string(),
        purpose: "call configured LLM provider through BodyGate for bounded synthesis".to_string(),
    }];
    contract.network_scope = vec![NetworkScope {
        allowed_hosts: vec!["configured_llm_endpoint".to_string()],
        methods: vec!["POST".to_string()],
        purpose: "single bounded synthesis call over already extracted local text".to_string(),
    }];
    contract.plan = vec![
        step(
            "select",
            "choose recent manifest entries with extracted text",
        ),
        step("bound", "truncate source text before LLM synthesis"),
        step("synthesize", "ask configured LLM through BodyGate"),
        step("record", "write brief, audit, outcome, and quality score"),
    ];
    contract.verification = vec![
        step(
            "traceable-source",
            "brief includes selected paper titles and hashes",
        ),
        step(
            "quality",
            "ResearchQuality evaluates uncertainty, falsifiers, and verification leads",
        ),
    ];
    contract.evidence_required = vec![
        EvidenceRequirement {
            kind: "extracted_text".to_string(),
            description: "Local extracted text path for each selected paper".to_string(),
            required: true,
        },
        EvidenceRequirement {
            kind: "paper_brief_audit".to_string(),
            description: "Audit record carrying status, brief path, model, and quality score"
                .to_string(),
            required: true,
        },
    ];
    contract
}

pub fn append_contract(root: &Path, contract: &HarnessContract) -> std::io::Result<()> {
    let path = root.join(CONTRACT_AUDIT_PATH);
    let json = serde_json::to_string(contract).map_err(std::io::Error::other)?;
    body_gate_bridge::record_memory_write(
        root,
        "harness_contract",
        "harness.append_contract",
        &json,
    )
    .map_err(std::io::Error::other)?;
    append_jsonl(&path, contract)
}

pub fn append_outcome(root: &Path, outcome: &HarnessOutcome) -> std::io::Result<()> {
    let path = root.join(OUTCOME_AUDIT_PATH);
    let json = serde_json::to_string(outcome).map_err(std::io::Error::other)?;
    body_gate_bridge::record_memory_write(root, "harness_outcome", "harness.append_outcome", &json)
        .map_err(std::io::Error::other)?;
    append_jsonl(&path, outcome)
}

fn base_contract(
    created_at_secs: u64,
    trigger: HarnessTrigger,
    goal: String,
    candidate: &ActionCandidate,
    risk_level: HarnessRiskLevel,
    resource_budget: ResourceBudgetHint,
) -> HarnessContract {
    let change_level = candidate.governance_level.min(5);
    let action_kind = format!("{:?}", candidate.kind);
    let contract_id = contract_id(created_at_secs, &goal, &action_kind, change_level);
    HarnessContract {
        contract_id,
        created_at_secs,
        owner: "Buster".to_string(),
        trigger,
        goal,
        action_kind,
        change_level,
        risk_level,
        resource_budget,
        read_set: Vec::new(),
        write_set: Vec::new(),
        required_tools: Vec::new(),
        required_secrets: Vec::new(),
        network_scope: Vec::new(),
        plan: Vec::new(),
        verification: Vec::new(),
        rollback: "append corrective record; do not erase the failed branch".to_string(),
        evidence_required: Vec::new(),
        memory_policy: MemoryPolicy::level_0_to_2_only(),
        failure_policy: FailurePolicy::no_failed_followups(),
    }
}

fn contract_id(created_at_secs: u64, goal: &str, action_kind: &str, change_level: u8) -> String {
    let mut hasher = Sha256::new();
    hasher.update(created_at_secs.to_string().as_bytes());
    hasher.update(goal.as_bytes());
    hasher.update(action_kind.as_bytes());
    hasher.update([change_level]);
    let suffix = hasher
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("hc-{created_at_secs}-{suffix}")
}

fn step(step_id: &str, summary: &str) -> HarnessStep {
    HarnessStep {
        step_id: step_id.to_string(),
        summary: summary.to_string(),
    }
}

fn one_line(text: &str, max_chars: usize) -> String {
    let mut out = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().count() > max_chars {
        out = out.chars().take(max_chars).collect();
        out.push_str("...");
    }
    out
}

fn append_jsonl(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value).map_err(std::io::Error::other)?;
    file.write_all(b"\n")
}

pub fn tail_file(
    root: &Path,
    relative_path: &str,
    max_lines: usize,
) -> std::io::Result<Vec<String>> {
    let path = root.join(relative_path);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines().map(ToString::to_string).collect::<Vec<_>>();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    Ok(lines)
}

pub fn audit_path(root: &Path, relative_path: &str) -> PathBuf {
    root.join(relative_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_contract_and_outcome_audits() {
        let root = unique_temp_dir("buster-harness");
        let candidate = ActionCandidate::new(ActionKind::SecondaryResearch, "test research");
        let contract = runtime_cycle_contract(0, now_secs(), &candidate);
        let outcome = HarnessOutcome::new(
            contract.contract_id.clone(),
            HarnessStatus::Completed,
            "cycle completed",
        );

        append_contract(&root, &contract).unwrap();
        append_outcome(&root, &outcome).unwrap();

        assert!(root.join(CONTRACT_AUDIT_PATH).exists());
        assert!(root.join(OUTCOME_AUDIT_PATH).exists());
        let lines = tail_file(&root, CONTRACT_AUDIT_PATH, 1).unwrap();
        assert_eq!(lines.len(), 1);

        let _ = fs::remove_dir_all(root);
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
