use std::fs;
use std::path::PathBuf;

use buster_body::host_api::BodyScope;
use buster_body::{BodyGate, SqliteBodyStore};
use buster_dreaming::{
    apply_deep_promotions, DreamSignal, DreamStore, DreamingConfig, DreamingEngine,
};
use buster_memory::{HashEmbeddingProvider, MarkdownMemoryStore, MemoryKind, VectorMemoryIndex};

fn main() {
    let now = 1_800_000_000;
    let day = 24 * 60 * 60;
    let signals = vec![
        DreamSignal::new(
            "feishu-tool-lease",
            "Feishu message sending requires a one-shot tool action lease and redacted audit.",
            "session:feishu-auth",
            "send Feishu message safely",
            0.92,
            MemoryKind::Experience,
        )
        .with_occurred_at_secs(now - day)
        .with_tags(["feishu", "tool", "lease", "audit"]),
        DreamSignal::new(
            "feishu-tool-lease",
            "Feishu message sending requires a one-shot tool action lease and redacted audit.",
            "session:tool-policy",
            "tool action lease for external side effects",
            0.88,
            MemoryKind::Experience,
        )
        .with_occurred_at_secs(now - 2 * day)
        .with_tags(["feishu", "tool", "lease", "audit"]),
        DreamSignal::new(
            "feishu-tool-lease",
            "Feishu message sending requires a one-shot tool action lease and redacted audit.",
            "session:body-gate",
            "redacted audit before external tool execution",
            0.9,
            MemoryKind::Experience,
        )
        .with_occurred_at_secs(now - 3 * day)
        .with_tags(["feishu", "tool", "lease", "audit"]),
        DreamSignal::new(
            "secret-protection",
            "OpenRouter API keys must stay inside the secret broker and never enter ordinary memory.",
            "session:secret-store",
            "protect API key from memory leak",
            0.91,
            MemoryKind::Immune,
        )
        .with_occurred_at_secs(now - day)
        .with_tags(["secret", "broker", "memory", "immune"]),
        DreamSignal::new(
            "secret-protection",
            "OpenRouter API keys must stay inside the secret broker and never enter ordinary memory.",
            "session:redaction",
            "secret broker memory redaction",
            0.86,
            MemoryKind::Immune,
        )
        .with_occurred_at_secs(now - 3 * day)
        .with_tags(["secret", "broker", "memory", "immune"]),
        DreamSignal::new(
            "secret-protection",
            "OpenRouter API keys must stay inside the secret broker and never enter ordinary memory.",
            "session:llm-bridge",
            "OpenRouter key should not appear in memory or audit output",
            0.89,
            MemoryKind::Immune,
        )
        .with_occurred_at_secs(now - 2 * day)
        .with_tags(["secret", "broker", "memory", "immune"]),
        DreamSignal::new(
            "identity-blocked",
            "Buster identity should be rewritten by this signal.",
            "session:blocked",
            "identity rewrite",
            1.0,
            MemoryKind::Identity,
        ),
    ];

    let engine = DreamingEngine::new(DreamingConfig::default());
    let report = engine.run_sweep(&signals);

    let root: PathBuf =
        std::env::temp_dir().join(format!("buster-dreaming-smoke-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    DreamStore::new(&root)
        .save_report(&report)
        .expect("dream report should be written");
    let mut memory_store = MarkdownMemoryStore::new(root.join("memory"));
    memory_store
        .load_from_disk()
        .expect("markdown memory should initialize");
    let mut vector_index = VectorMemoryIndex::new();
    let embedding_provider = HashEmbeddingProvider::default();
    let gate = BodyGate::new(
        SqliteBodyStore::open_in_memory().expect("body store should open"),
        buster_body::BodyMode::Normal,
    );
    let scope = BodyScope::new("buster", "main", "dreaming-smoke");
    let applied = apply_deep_promotions(
        &report,
        &gate,
        &scope,
        &memory_store,
        &mut vector_index,
        &embedding_provider,
    )
    .expect("deep promotions should apply");

    println!("workspace: {}", root.display());
    println!("light staged: {}", report.light.staged.len());
    println!("rem reflections: {}", report.rem.reflections.len());
    println!("deep candidates: {}", report.deep.candidates.len());
    println!("applied promotions: {}", applied.applied.len());
    println!(
        "body audit events: {}",
        gate.store().audit_event_count().unwrap_or(0)
    );
    println!("vector chunks: {}", vector_index.chunks().len());
    println!();
    println!("--- DREAMS.md ---");
    println!("{}", report.render_dream_diary());
    println!("--- promotion-proposals.md ---");
    println!("{}", report.render_promotion_proposals());
}
