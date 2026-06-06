//! Buster's dreaming engine.
//!
//! This crate adapts OpenClaw memory-core's Light / REM / Deep consolidation
//! pattern into a small Rust engine. Dreaming is not the authority for identity
//! or value memory; it proposes ordinary memory promotions for BodyGate review.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use buster_body::host_api::BodyScope;
use buster_body::{BodyGate, BodyGateError, BodyStore, MemoryWriteIntent};
use buster_memory::{
    EmbeddingProvider, MarkdownMemoryStore, MemoryDocument, MemoryKind, MemoryRecord,
    MemoryStoreError, VectorMemoryIndex,
};
use serde::{Deserialize, Serialize};

const DAY_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DreamPhase {
    Light,
    Rem,
    Deep,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DreamSignal {
    pub key: String,
    pub snippet: String,
    pub source: String,
    pub grounding_path: Option<String>,
    pub query: String,
    pub score: f32,
    pub occurred_at_secs: u64,
    pub tags: Vec<String>,
    pub kind: MemoryKind,
}

impl DreamSignal {
    pub fn new(
        key: impl Into<String>,
        snippet: impl Into<String>,
        source: impl Into<String>,
        query: impl Into<String>,
        score: f32,
        kind: MemoryKind,
    ) -> Self {
        let snippet = snippet.into();
        Self {
            key: key.into(),
            tags: derive_tags(&snippet),
            snippet,
            source: source.into(),
            grounding_path: None,
            query: query.into(),
            score: clamp01(score),
            occurred_at_secs: now_secs(),
            kind,
        }
    }

    pub fn with_occurred_at_secs(mut self, occurred_at_secs: u64) -> Self {
        self.occurred_at_secs = occurred_at_secs;
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = unique_strings(tags);
        self
    }

    pub fn with_grounding_path(mut self, grounding_path: impl Into<String>) -> Self {
        self.grounding_path = Some(grounding_path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DreamingConfig {
    pub light_limit: usize,
    pub rem_limit: usize,
    pub deep_limit: usize,
    pub dedupe_similarity: f32,
    pub min_pattern_strength: f32,
    pub min_promotion_score: f32,
    pub min_signal_count: usize,
    pub min_unique_queries: usize,
    pub recency_half_life_days: f32,
    pub light_boost_max: f32,
    pub rem_boost_max: f32,
    pub rem_min_truth_confidence: f32,
    pub rem_truth_dedupe_similarity: f32,
    pub rem_truth_limit: usize,
}

impl Default for DreamingConfig {
    fn default() -> Self {
        Self {
            light_limit: 12,
            rem_limit: 8,
            deep_limit: 5,
            dedupe_similarity: 0.82,
            min_pattern_strength: 0.25,
            min_promotion_score: 0.75,
            min_signal_count: 3,
            min_unique_queries: 2,
            recency_half_life_days: 14.0,
            light_boost_max: 0.06,
            rem_boost_max: 0.09,
            rem_min_truth_confidence: 0.45,
            rem_truth_dedupe_similarity: 0.88,
            rem_truth_limit: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StagedMemory {
    pub key: String,
    pub snippet: String,
    pub source: String,
    pub grounding_path: Option<String>,
    pub signal_count: usize,
    pub avg_score: f32,
    pub unique_queries: usize,
    pub recall_days: usize,
    pub tags: Vec<String>,
    pub kind: MemoryKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightSleepReport {
    pub staged: Vec<StagedMemory>,
    pub skipped_duplicates: usize,
    pub body_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemReflection {
    pub tag: String,
    pub strength: f32,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemSleepReport {
    pub source_count: usize,
    pub reflections: Vec<RemReflection>,
    pub candidate_truths: Vec<StagedMemory>,
    pub phase_signal_keys: Vec<String>,
    pub body_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotionComponents {
    pub frequency: f32,
    pub relevance: f32,
    pub diversity: f32,
    pub recency: f32,
    pub consolidation: f32,
    pub conceptual: f32,
    pub light_boost: f32,
    pub rem_boost: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotionCandidate {
    pub key: String,
    pub snippet: String,
    pub source: String,
    pub grounding_path: Option<String>,
    pub kind: MemoryKind,
    pub score: f32,
    pub signal_count: usize,
    pub unique_queries: usize,
    pub tags: Vec<String>,
    pub components: PromotionComponents,
    pub proposed_memory: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeepSleepReport {
    pub candidates: Vec<PromotionCandidate>,
    pub body_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionApplyRecord {
    pub key: String,
    pub kind: MemoryKind,
    pub source: String,
    pub confidence: u8,
    pub vector_chunks_added: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionApplyReport {
    pub applied: Vec<PromotionApplyRecord>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DreamingPhaseSignals {
    pub light: Vec<String>,
    pub rem: Vec<String>,
    pub deep: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DreamingReport {
    pub occurred_at_secs: u64,
    pub light: LightSleepReport,
    pub rem: RemSleepReport,
    pub deep: DeepSleepReport,
}

impl DreamingReport {
    pub fn phase_signals(&self) -> DreamingPhaseSignals {
        DreamingPhaseSignals {
            light: self
                .light
                .staged
                .iter()
                .map(|entry| entry.key.clone())
                .collect(),
            rem: self.rem.phase_signal_keys.clone(),
            deep: self
                .deep
                .candidates
                .iter()
                .map(|entry| entry.key.clone())
                .collect(),
        }
    }

    pub fn render_dream_diary(&self) -> String {
        let mut lines = vec![
            "# Dream Diary".to_string(),
            String::new(),
            format!("## Sweep {}", self.occurred_at_secs),
            String::new(),
            "### Light Sleep".to_string(),
        ];
        lines.extend(self.light.body_lines.clone());
        lines.push(String::new());
        lines.push("### REM Sleep".to_string());
        lines.extend(self.rem.body_lines.clone());
        lines.push(String::new());
        lines.push("### Deep Sleep".to_string());
        lines.extend(self.deep.body_lines.clone());
        lines.push(String::new());
        lines.join("\n")
    }

    pub fn render_promotion_proposals(&self) -> String {
        let mut lines = vec!["# Dream Promotion Proposals".to_string(), String::new()];
        if self.deep.candidates.is_empty() {
            lines.push("- No promotion candidates met the current thresholds.".to_string());
            return lines.join("\n");
        }
        for candidate in &self.deep.candidates {
            lines.push(format!("## {} score={:.3}", candidate.key, candidate.score));
            lines.push(format!("- kind: {:?}", candidate.kind));
            lines.push(format!("- source: {}", candidate.source));
            if let Some(path) = &candidate.grounding_path {
                lines.push(format!("- grounding_path: {path}"));
            }
            lines.push(format!("- signal_count: {}", candidate.signal_count));
            lines.push(format!("- unique_queries: {}", candidate.unique_queries));
            lines.push(format!("- tags: {}", candidate.tags.join(", ")));
            lines.push(format!("- proposed_memory: {}", candidate.proposed_memory));
            lines.push(String::new());
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct DreamingEngine {
    config: DreamingConfig,
}

impl DreamingEngine {
    pub fn new(config: DreamingConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &DreamingConfig {
        &self.config
    }

    pub fn run_sweep(&self, signals: &[DreamSignal]) -> DreamingReport {
        let now = now_secs();
        let light = self.light_sleep(signals);
        let rem = self.rem_sleep(&light.staged);
        let deep = self.deep_sleep(signals, &light, &rem, now);
        DreamingReport {
            occurred_at_secs: now,
            light,
            rem,
            deep,
        }
    }

    pub fn light_sleep(&self, signals: &[DreamSignal]) -> LightSleepReport {
        let mut grouped: BTreeMap<String, Vec<&DreamSignal>> = BTreeMap::new();
        for signal in signals {
            if is_protected_kind(signal.kind) || looks_like_secret(&signal.snippet) {
                continue;
            }
            grouped.entry(signal.key.clone()).or_default().push(signal);
        }

        let mut staged = grouped
            .into_iter()
            .filter_map(|(key, items)| staged_memory_from_signals(key, &items))
            .collect::<Vec<_>>();
        staged.sort_by(|left, right| compare_desc(left.avg_score, right.avg_score));

        let mut deduped: Vec<StagedMemory> = Vec::new();
        let mut skipped_duplicates = 0;
        for entry in staged {
            if deduped.iter().any(|existing| {
                text_similarity(&existing.snippet, &entry.snippet) >= self.config.dedupe_similarity
            }) {
                skipped_duplicates += 1;
                continue;
            }
            deduped.push(entry);
            if deduped.len() >= self.config.light_limit {
                break;
            }
        }

        let body_lines = if deduped.is_empty() {
            vec!["- No notable short-term memories staged.".to_string()]
        } else {
            deduped
                .iter()
                .map(|entry| {
                    format!(
                        "- {} [signals={} avg={:.2}]",
                        entry.snippet, entry.signal_count, entry.avg_score
                    )
                })
                .collect()
        };

        LightSleepReport {
            staged: deduped,
            skipped_duplicates,
            body_lines,
        }
    }

    pub fn rem_sleep(&self, staged: &[StagedMemory]) -> RemSleepReport {
        let mut tag_stats: BTreeMap<String, (usize, BTreeSet<String>)> = BTreeMap::new();
        for entry in staged {
            for tag in &entry.tags {
                let normalized = tag.trim().to_ascii_lowercase();
                if normalized.is_empty() || rem_tag_blacklist(&normalized) {
                    continue;
                }
                let stat = tag_stats.entry(normalized).or_default();
                stat.0 += 1;
                stat.1.insert(entry.snippet.clone());
            }
        }

        let source_count = staged.len().max(1);
        let mut reflections = tag_stats
            .into_iter()
            .filter_map(|(tag, (count, evidence))| {
                let strength = clamp01(count as f32 / source_count as f32);
                if strength < self.config.min_pattern_strength {
                    return None;
                }
                Some(RemReflection {
                    tag,
                    strength,
                    evidence: evidence.into_iter().take(3).collect(),
                })
            })
            .collect::<Vec<_>>();
        reflections.sort_by(|left, right| {
            compare_desc(left.strength, right.strength).then_with(|| left.tag.cmp(&right.tag))
        });
        reflections.truncate(self.config.rem_limit);

        let mut candidate_truths = Vec::new();
        for entry in staged {
            let confidence = rem_candidate_truth_confidence(entry);
            if confidence < self.config.rem_min_truth_confidence {
                continue;
            }
            if candidate_truths.iter().any(|existing: &StagedMemory| {
                text_similarity(&existing.snippet, &entry.snippet)
                    >= self.config.rem_truth_dedupe_similarity
            }) {
                continue;
            }
            candidate_truths.push(entry.clone());
        }
        candidate_truths.sort_by(|left, right| {
            compare_desc(
                rem_candidate_truth_confidence(left),
                rem_candidate_truth_confidence(right),
            )
            .then_with(|| left.snippet.cmp(&right.snippet))
        });
        candidate_truths.truncate(self.config.rem_truth_limit.min(3).max(1));
        let phase_signal_keys = unique_strings(candidate_truths.iter().map(|entry| &entry.key));

        let mut body_lines = vec!["#### Reflections".to_string()];
        if reflections.is_empty() {
            body_lines.push("- No strong recurring patterns surfaced.".to_string());
        } else {
            body_lines.extend(reflections.iter().map(|reflection| {
                format!("- {} strength={:.2}", reflection.tag, reflection.strength)
            }));
        }
        body_lines.push(String::new());
        body_lines.push("#### Possible Lasting Truths".to_string());
        if candidate_truths.is_empty() {
            body_lines.push("- No candidate truths surfaced.".to_string());
        } else {
            body_lines.extend(candidate_truths.iter().map(|entry| {
                format!(
                    "- {} [confidence={:.2}]",
                    entry.snippet,
                    rem_candidate_truth_confidence(entry)
                )
            }));
        }

        RemSleepReport {
            source_count: staged.len(),
            reflections,
            candidate_truths,
            phase_signal_keys,
            body_lines,
        }
    }

    pub fn deep_sleep(
        &self,
        signals: &[DreamSignal],
        light: &LightSleepReport,
        rem: &RemSleepReport,
        now_secs: u64,
    ) -> DeepSleepReport {
        let mut grouped: BTreeMap<String, Vec<&DreamSignal>> = BTreeMap::new();
        for signal in signals {
            if is_protected_kind(signal.kind) || looks_like_secret(&signal.snippet) {
                continue;
            }
            grouped.entry(signal.key.clone()).or_default().push(signal);
        }

        let light_keys = light
            .staged
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<BTreeSet<_>>();
        let rem_keys = rem
            .candidate_truths
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<BTreeSet<_>>();

        let mut candidates = Vec::new();
        for (key, items) in grouped {
            let Some(staged) = staged_memory_from_signals(key.clone(), &items) else {
                continue;
            };
            if staged.signal_count < self.config.min_signal_count
                || staged.unique_queries < self.config.min_unique_queries
            {
                continue;
            }
            let mut components = promotion_components(&items, &staged, now_secs, &self.config);
            if light_keys.contains(key.as_str()) {
                components.light_boost = self.config.light_boost_max;
            }
            if rem_keys.contains(key.as_str()) {
                components.rem_boost = self.config.rem_boost_max;
            }
            let mut score = 0.24 * components.frequency
                + 0.30 * components.relevance
                + 0.15 * components.diversity
                + 0.15 * components.recency
                + 0.10 * components.consolidation
                + 0.06 * components.conceptual
                + components.light_boost
                + components.rem_boost;
            score = clamp01(score);
            if score < self.config.min_promotion_score {
                continue;
            }
            candidates.push(PromotionCandidate {
                key,
                snippet: staged.snippet.clone(),
                source: staged.source.clone(),
                grounding_path: staged.grounding_path.clone(),
                kind: staged.kind,
                score,
                signal_count: staged.signal_count,
                unique_queries: staged.unique_queries,
                tags: staged.tags.clone(),
                components,
                proposed_memory: format!(
                    "{} (source: {}; confidence: {})",
                    staged.snippet,
                    staged.source,
                    (score * 100.0).round() as u8
                ),
            });
        }

        candidates.sort_by(|left, right| {
            compare_desc(left.score, right.score).then_with(|| left.key.cmp(&right.key))
        });
        candidates.truncate(self.config.deep_limit);

        let body_lines = if candidates.is_empty() {
            vec!["- No candidates met promotion thresholds.".to_string()]
        } else {
            candidates
                .iter()
                .map(|candidate| {
                    format!(
                        "- {} score={:.3} signals={} queries={}",
                        candidate.snippet,
                        candidate.score,
                        candidate.signal_count,
                        candidate.unique_queries
                    )
                })
                .collect()
        };

        DeepSleepReport {
            candidates,
            body_lines,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DreamStore {
    root: PathBuf,
}

impl DreamStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn dreams_dir(&self) -> PathBuf {
        self.root.join("memory").join(".dreams")
    }

    pub fn save_report(&self, report: &DreamingReport) -> Result<(), DreamingError> {
        fs::create_dir_all(self.dreams_dir())?;
        atomic_write(&self.root.join("DREAMS.md"), &report.render_dream_diary())?;
        atomic_write(
            &self.dreams_dir().join("last-report.json"),
            &serde_json::to_string_pretty(report)?,
        )?;
        atomic_write(
            &self.dreams_dir().join("promotion-proposals.md"),
            &report.render_promotion_proposals(),
        )?;
        atomic_write(
            &self.dreams_dir().join("phase-signals.json"),
            &serde_json::to_string_pretty(&report.phase_signals())?,
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum DreamingError {
    Io(std::io::Error),
    Json(serde_json::Error),
    BodyGate(BodyGateError),
    Memory(MemoryStoreError),
}

impl fmt::Display for DreamingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "dreaming I/O error: {error}"),
            Self::Json(error) => write!(formatter, "dreaming JSON error: {error}"),
            Self::BodyGate(error) => write!(formatter, "dreaming body gate error: {error}"),
            Self::Memory(error) => write!(formatter, "dreaming memory error: {error}"),
        }
    }
}

impl std::error::Error for DreamingError {}

impl From<std::io::Error> for DreamingError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for DreamingError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<BodyGateError> for DreamingError {
    fn from(error: BodyGateError) -> Self {
        Self::BodyGate(error)
    }
}

impl From<MemoryStoreError> for DreamingError {
    fn from(error: MemoryStoreError) -> Self {
        Self::Memory(error)
    }
}

pub fn apply_deep_promotions<S, P>(
    report: &DreamingReport,
    gate: &BodyGate<S>,
    scope: &BodyScope,
    memory_store: &MarkdownMemoryStore,
    vector_index: &mut VectorMemoryIndex,
    embedding_provider: &P,
) -> Result<PromotionApplyReport, DreamingError>
where
    S: BodyStore,
    P: EmbeddingProvider,
{
    let mut output = PromotionApplyReport::default();

    for candidate in &report.deep.candidates {
        if is_protected_kind(candidate.kind) {
            output
                .skipped
                .push(format!("{} skipped: protected memory kind", candidate.key));
            continue;
        }
        if looks_like_secret(&candidate.snippet) {
            output
                .skipped
                .push(format!("{} skipped: secret-like content", candidate.key));
            continue;
        }
        if !candidate_grounding_is_live(candidate) {
            output
                .skipped
                .push(format!("{} skipped: stale grounding source", candidate.key));
            continue;
        }

        let confidence = (candidate.score * 100.0).round().clamp(0.0, 100.0) as u8;
        gate.record_memory_write(
            scope,
            MemoryWriteIntent::new(
                format!("{:?}", candidate.kind),
                candidate.source.clone(),
                &candidate.snippet,
            ),
        )?;
        let record = MemoryRecord::new(
            candidate.kind,
            candidate.snippet.clone(),
            candidate.source.clone(),
            confidence,
        );
        memory_store.append_record(&record)?;
        memory_store.append_global_record(&record)?;
        let vector_chunks_added = vector_index.add_document(
            &MemoryDocument::new(
                candidate.key.clone(),
                candidate.kind,
                candidate.snippet.clone(),
                candidate.source.clone(),
            ),
            embedding_provider,
        )?;

        output.applied.push(PromotionApplyRecord {
            key: candidate.key.clone(),
            kind: candidate.kind,
            source: candidate.source.clone(),
            confidence,
            vector_chunks_added,
        });
    }

    Ok(output)
}

fn staged_memory_from_signals(key: String, items: &[&DreamSignal]) -> Option<StagedMemory> {
    let first = items.first()?;
    let signal_count = items.len();
    let avg_score = clamp01(items.iter().map(|item| item.score).sum::<f32>() / signal_count as f32);
    let unique_queries = items
        .iter()
        .map(|item| normalized(&item.query))
        .collect::<BTreeSet<_>>()
        .len();
    let recall_days = items
        .iter()
        .map(|item| item.occurred_at_secs / DAY_SECS)
        .collect::<BTreeSet<_>>()
        .len();
    let mut tags = Vec::new();
    for item in items {
        tags.extend(item.tags.iter().cloned());
    }
    let tags = unique_strings(tags);
    Some(StagedMemory {
        key,
        snippet: first.snippet.clone(),
        source: first.source.clone(),
        grounding_path: items.iter().find_map(|item| item.grounding_path.clone()),
        signal_count,
        avg_score,
        unique_queries,
        recall_days,
        tags,
        kind: first.kind,
    })
}

fn promotion_components(
    items: &[&DreamSignal],
    staged: &StagedMemory,
    now_secs: u64,
    config: &DreamingConfig,
) -> PromotionComponents {
    let signal_count = items.len().max(1);
    let newest = items
        .iter()
        .map(|item| item.occurred_at_secs)
        .max()
        .unwrap_or(now_secs);
    let oldest = items
        .iter()
        .map(|item| item.occurred_at_secs)
        .min()
        .unwrap_or(newest);
    let age_days = now_secs.saturating_sub(newest) as f32 / DAY_SECS as f32;
    let span_days = newest.saturating_sub(oldest) as f32 / DAY_SECS as f32;
    PromotionComponents {
        frequency: clamp01((signal_count as f32).ln_1p() / 10.0_f32.ln_1p()),
        relevance: staged.avg_score,
        diversity: clamp01(staged.unique_queries as f32 / 5.0),
        recency: recency_score(age_days, config.recency_half_life_days),
        consolidation: clamp01(span_days / 3.0),
        conceptual: clamp01(staged.tags.len() as f32 / 6.0),
        light_boost: 0.0,
        rem_boost: 0.0,
    }
}

fn rem_candidate_truth_confidence(entry: &StagedMemory) -> f32 {
    let recall_strength = clamp01((entry.signal_count as f32).ln_1p() / 6.0_f32.ln_1p());
    let consolidation = clamp01(entry.recall_days as f32 / 3.0);
    let conceptual = clamp01(entry.tags.len() as f32 / 6.0);
    clamp01(
        entry.avg_score * 0.45 + recall_strength * 0.25 + consolidation * 0.20 + conceptual * 0.10,
    )
}

fn candidate_grounding_is_live(candidate: &PromotionCandidate) -> bool {
    let Some(path) = &candidate.grounding_path else {
        return true;
    };
    let path = Path::new(path);
    if !path.exists() {
        return false;
    }
    match fs::read_to_string(path) {
        Ok(contents) => contents.contains(&candidate.snippet),
        Err(_) => false,
    }
}

fn recency_score(age_days: f32, half_life_days: f32) -> f32 {
    if !age_days.is_finite() || age_days <= 0.0 {
        return 1.0;
    }
    if !half_life_days.is_finite() || half_life_days <= 0.0 {
        return 0.0;
    }
    0.5_f32.powf(age_days / half_life_days)
}

fn derive_tags(text: &str) -> Vec<String> {
    let mut tags = tokenize(text)
        .into_iter()
        .filter(|token| token.len() >= 4 && !stop_words().contains(token.as_str()))
        .take(8)
        .collect::<Vec<_>>();
    tags.sort();
    tags.dedup();
    tags
}

fn unique_strings(values: impl IntoIterator<Item = impl Into<String>>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| normalized(&value.into()))
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .map(normalized)
        .filter(|token| token.len() >= 2)
        .collect()
}

fn normalized(text: &str) -> String {
    text.trim().to_ascii_lowercase()
}

fn text_similarity(left: &str, right: &str) -> f32 {
    let left_tokens = tokenize(left).into_iter().collect::<BTreeSet<_>>();
    let right_tokens = tokenize(right).into_iter().collect::<BTreeSet<_>>();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }
    let intersection = left_tokens.intersection(&right_tokens).count() as f32;
    let union = left_tokens.union(&right_tokens).count() as f32;
    intersection / union
}

fn looks_like_secret(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("sk-")
        || lower.contains("api_key=")
        || lower.contains("api key:")
        || lower.contains("password=")
        || lower.contains("private key")
        || lower.contains("bearer ")
}

fn is_protected_kind(kind: MemoryKind) -> bool {
    matches!(kind, MemoryKind::Identity | MemoryKind::Value)
}

fn rem_tag_blacklist(tag: &str) -> bool {
    matches!(tag, "assistant" | "user" | "system" | "subagent" | "the")
}

fn stop_words() -> &'static BTreeSet<&'static str> {
    use std::sync::OnceLock;
    static WORDS: OnceLock<BTreeSet<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        [
            "this", "that", "with", "from", "into", "about", "should", "could", "would", "memory",
            "buster",
        ]
        .into_iter()
        .collect()
    })
}

fn compare_desc(left: f32, right: f32) -> Ordering {
    right.partial_cmp(&left).unwrap_or(Ordering::Equal)
}

fn clamp01(value: f32) -> f32 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(0.0, 1.0)
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), DreamingError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension(format!("tmp.{}", now_secs()));
    fs::write(&tmp_path, contents)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> u64 {
        1_800_000_000
    }

    fn signals() -> Vec<DreamSignal> {
        vec![
            DreamSignal::new(
                "tool-lease",
                "Feishu message sending requires a one-shot tool action lease and redacted audit.",
                "session-a",
                "send feishu safely",
                0.92,
                MemoryKind::Experience,
            )
            .with_occurred_at_secs(now() - DAY_SECS)
            .with_tags(["feishu", "tool", "lease", "audit"]),
            DreamSignal::new(
                "tool-lease",
                "Feishu message sending requires a one-shot tool action lease and redacted audit.",
                "session-b",
                "tool action lease",
                0.86,
                MemoryKind::Experience,
            )
            .with_occurred_at_secs(now() - 2 * DAY_SECS)
            .with_tags(["feishu", "tool", "lease", "audit"]),
            DreamSignal::new(
                "tool-lease",
                "Feishu message sending requires a one-shot tool action lease and redacted audit.",
                "session-d",
                "redacted audit for Feishu tool",
                0.9,
                MemoryKind::Experience,
            )
            .with_occurred_at_secs(now() - 3 * DAY_SECS)
            .with_tags(["feishu", "tool", "lease", "audit"]),
            DreamSignal::new(
                "weather",
                "Shanghai weather was cloudy.",
                "session-c",
                "weather",
                0.3,
                MemoryKind::Fact,
            )
            .with_occurred_at_secs(now() - DAY_SECS),
        ]
    }

    #[test]
    fn light_sleep_dedupes_and_stages_recent_signals() {
        let engine = DreamingEngine::new(DreamingConfig::default());

        let report = engine.light_sleep(&signals());

        assert!(report.staged.iter().any(|entry| entry.key == "tool-lease"));
        assert!(report
            .body_lines
            .iter()
            .any(|line| line.contains("one-shot tool action lease")));
    }

    #[test]
    fn rem_sleep_extracts_recurring_reflections() {
        let engine = DreamingEngine::new(DreamingConfig::default());
        let light = engine.light_sleep(&signals());

        let rem = engine.rem_sleep(&light.staged);

        assert!(rem
            .reflections
            .iter()
            .any(|reflection| reflection.tag == "audit" || reflection.tag == "lease"));
        assert_eq!(rem.candidate_truths.len(), 1);
        assert_eq!(rem.phase_signal_keys, vec!["tool-lease".to_string()]);
        assert!(rem
            .body_lines
            .iter()
            .any(|line| line.contains("confidence=")));
        assert!(!rem.body_lines.is_empty());
    }

    #[test]
    fn deep_sleep_scores_promotion_candidates_without_applying_them() {
        let engine = DreamingEngine::new(DreamingConfig {
            min_promotion_score: 0.5,
            ..DreamingConfig::default()
        });
        let input = signals();
        let light = engine.light_sleep(&input);
        let rem = engine.rem_sleep(&light.staged);

        let deep = engine.deep_sleep(&input, &light, &rem, now());

        assert_eq!(deep.candidates.len(), 1);
        assert_eq!(deep.candidates[0].key, "tool-lease");
        assert!(deep.candidates[0].proposed_memory.contains("source:"));
    }

    #[test]
    fn dreaming_ignores_protected_or_secret_like_signals() {
        let engine = DreamingEngine::new(DreamingConfig::default());
        let input = vec![
            DreamSignal::new(
                "identity",
                "Buster identity value",
                "test",
                "identity",
                1.0,
                MemoryKind::Identity,
            ),
            DreamSignal::new(
                "secret",
                "api_key=sk-test-secret",
                "test",
                "secret",
                1.0,
                MemoryKind::Fact,
            ),
        ];

        let report = engine.run_sweep(&input);

        assert!(report.light.staged.is_empty());
        assert!(report.deep.candidates.is_empty());
    }

    #[test]
    fn dream_store_writes_diary_and_promotion_proposals() {
        let root = std::env::temp_dir().join(format!("buster-dreaming-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let engine = DreamingEngine::new(DreamingConfig {
            min_promotion_score: 0.5,
            ..DreamingConfig::default()
        });
        let report = engine.run_sweep(&signals());
        let store = DreamStore::new(&root);

        store.save_report(&report).unwrap();

        assert!(root.join("DREAMS.md").exists());
        assert!(root.join("memory/.dreams/last-report.json").exists());
        assert!(root.join("memory/.dreams/promotion-proposals.md").exists());
        assert!(root.join("memory/.dreams/phase-signals.json").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deep_promotions_apply_through_body_gate_to_memory_and_vector_index() {
        let root =
            std::env::temp_dir().join(format!("buster-dreaming-apply-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let engine = DreamingEngine::new(DreamingConfig {
            min_promotion_score: 0.5,
            ..DreamingConfig::default()
        });
        let report = engine.run_sweep(&signals());
        let mut memory_store = MarkdownMemoryStore::new(root.join("memory"));
        memory_store.load_from_disk().unwrap();
        let mut vector_index = VectorMemoryIndex::new();
        let embedding_provider = buster_memory::HashEmbeddingProvider::default();
        let store = buster_body::SqliteBodyStore::open_in_memory().unwrap();
        let gate = BodyGate::new(store, buster_body::BodyMode::Normal);
        let scope = BodyScope::new("buster", "main", "dreaming-test");

        let applied = apply_deep_promotions(
            &report,
            &gate,
            &scope,
            &memory_store,
            &mut vector_index,
            &embedding_provider,
        )
        .unwrap();

        assert_eq!(applied.applied.len(), 1);
        assert_eq!(gate.store().audit_event_count().unwrap(), 1);
        assert_eq!(vector_index.chunks().len(), 1);
        let experience = fs::read_to_string(root.join("memory/experience.md")).unwrap();
        let global_memory = fs::read_to_string(root.join("MEMORY.md")).unwrap();
        assert!(experience.contains("Feishu message sending requires"));
        assert!(global_memory.contains("Feishu message sending requires"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deep_promotions_skip_stale_grounding_sources() {
        let root =
            std::env::temp_dir().join(format!("buster-dreaming-stale-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let missing_source = root.join("deleted-daily.md");
        let stale_signals = signals()
            .into_iter()
            .map(|signal| signal.with_grounding_path(missing_source.display().to_string()))
            .collect::<Vec<_>>();
        let engine = DreamingEngine::new(DreamingConfig::default());
        let report = engine.run_sweep(&stale_signals);
        let mut memory_store = MarkdownMemoryStore::new(root.join("memory"));
        memory_store.load_from_disk().unwrap();
        let mut vector_index = VectorMemoryIndex::new();
        let embedding_provider = buster_memory::HashEmbeddingProvider::default();
        let store = buster_body::SqliteBodyStore::open_in_memory().unwrap();
        let gate = BodyGate::new(store, buster_body::BodyMode::Normal);
        let scope = BodyScope::new("buster", "main", "dreaming-stale-test");

        let applied = apply_deep_promotions(
            &report,
            &gate,
            &scope,
            &memory_store,
            &mut vector_index,
            &embedding_provider,
        )
        .unwrap();

        assert_eq!(report.deep.candidates.len(), 1);
        assert!(applied.applied.is_empty());
        assert_eq!(applied.skipped.len(), 1);
        assert!(applied.skipped[0].contains("stale grounding source"));
        assert_eq!(gate.store().audit_event_count().unwrap(), 0);
        assert!(vector_index.chunks().is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
