//! Open paper acquisition for Buster's PaperQA corpus.
//!
//! v0 only downloads clearly open arXiv PDFs discovered through the existing
//! source fetcher. Paywalled or ambiguous sources are skipped with an audit
//! reason instead of being fetched.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use buster_value_model::ResearchDomain;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::paperqa::PAPERQA_PAPER_DIR;
use crate::source_fetch::{ResearchSource, SourceFetchBundle, SourceFetcher, SourceProvider};
use crate::{append_daemon_jsonl, body_gate_bridge, harness, now_secs};

pub const PAPER_ACQUISITION_AUDIT: &str = "audit/paper-acquisition.jsonl";
pub const PAPER_MANIFEST: &str = "research/paperqa/manifest.jsonl";
pub const PAPER_TEXT_DIR: &str = "research/paperqa/extracted";
const MAX_DOWNLOADS: usize = 5;
const MAX_PDF_BYTES: usize = 30 * 1024 * 1024;
const OPEN_PAPER_ALLOWED_HOSTS: &[&str] = &["arxiv.org", "export.arxiv.org"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperAcquisitionRequest {
    pub question: String,
    pub domain: ResearchDomain,
    pub task_id: Option<String>,
    pub limit: usize,
}

impl PaperAcquisitionRequest {
    pub fn new(question: impl Into<String>) -> Self {
        Self {
            question: question.into(),
            domain: ResearchDomain::Other,
            task_id: None,
            limit: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperAcquisitionRecord {
    pub timestamp_secs: u64,
    pub contract_id: String,
    pub task_id: Option<String>,
    pub domain: ResearchDomain,
    pub question: String,
    pub status: String,
    pub source_bundle_path: PathBuf,
    pub downloaded: Vec<DownloadedPaper>,
    pub skipped: Vec<SkippedPaper>,
    pub errors: Vec<String>,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadedPaper {
    pub source_id: String,
    pub title: String,
    pub provider: SourceProvider,
    pub open_access_kind: String,
    pub landing_url: Option<String>,
    pub pdf_url: String,
    pub file_path: PathBuf,
    pub text_path: Option<PathBuf>,
    pub text_chars: usize,
    pub sha256: String,
    pub bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkippedPaper {
    pub source_id: String,
    pub title: String,
    pub provider: SourceProvider,
    pub reason: String,
}

pub fn acquire_open_papers(
    root: &Path,
    request: PaperAcquisitionRequest,
) -> std::io::Result<PaperAcquisitionRecord> {
    ensure_layout(root)?;
    let timestamp_secs = now_secs();
    let limit = request.limit.clamp(1, MAX_DOWNLOADS);
    let contract =
        harness::paper_acquisition_contract(timestamp_secs, request.domain, &request.question);
    let contract_id = contract.contract_id.clone();
    harness::append_contract(root, &contract)?;

    let bundle = SourceFetcher::new().fetch_bundle(
        root,
        request.task_id.as_deref(),
        request.domain,
        &request.question,
    )?;
    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent("BusterPaperAcquisition/0.1 (open-access corpus builder)")
        .build()
        .unwrap_or_else(|_| Client::new());

    let mut downloaded = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();
    for source in &bundle.sources {
        if downloaded.len() >= limit {
            skipped.push(skip(source, "download limit reached"));
            continue;
        }
        let Some(pdf_url) = resolve_open_pdf_url(source) else {
            skipped.push(skip(source, "no recognized open full-text PDF URL"));
            continue;
        };
        match download_open_pdf(root, &client, source, &pdf_url, request.domain) {
            Ok(paper) => downloaded.push(paper),
            Err(error) => errors.push(format!("{}: {error}", source.source_id)),
        }
    }

    let status = if !downloaded.is_empty() {
        "ok"
    } else if errors.is_empty() {
        "no_open_pdfs"
    } else {
        "partial_or_failed"
    }
    .to_string();
    let record = PaperAcquisitionRecord {
        timestamp_secs,
        contract_id: contract_id.clone(),
        task_id: request.task_id,
        domain: request.domain,
        question: request.question,
        status: status.clone(),
        source_bundle_path: bundle.bundle_path,
        downloaded,
        skipped,
        errors,
        manifest_path: root.join(PAPER_MANIFEST),
    };
    for paper in &record.downloaded {
        append_manifest(root, timestamp_secs, &contract_id, &record, paper)?;
    }
    append_daemon_jsonl(&root.join(PAPER_ACQUISITION_AUDIT), &record)?;
    let mut outcome = harness::HarnessOutcome::new(
        contract_id,
        if record.downloaded.is_empty() {
            harness::HarnessStatus::Partial
        } else {
            harness::HarnessStatus::Completed
        },
        format!(
            "paper acquisition {status}: downloaded {}, skipped {}, errors {}",
            record.downloaded.len(),
            record.skipped.len(),
            record.errors.len()
        ),
    )
    .with_evidence(
        "source_bundle",
        record.source_bundle_path.display().to_string(),
        "source metadata used to resolve open PDFs",
    )
    .with_output("audit", PAPER_ACQUISITION_AUDIT, "paper acquisition audit")
    .with_output("manifest", PAPER_MANIFEST, "PaperQA corpus manifest");
    for paper in &record.downloaded {
        outcome = outcome.with_output(
            "paper_pdf",
            paper.file_path.display().to_string(),
            "downloaded open-access PDF",
        );
    }
    if !record.errors.is_empty() {
        outcome = outcome.with_error(record.errors.join(" | "));
    }
    harness::append_outcome(root, &outcome)?;
    Ok(record)
}

pub fn acquire_direct_pdf(
    root: &Path,
    request: PaperAcquisitionRequest,
    pdf_url: &str,
) -> std::io::Result<PaperAcquisitionRecord> {
    ensure_layout(root)?;
    let timestamp_secs = now_secs();
    let contract =
        harness::paper_acquisition_contract(timestamp_secs, request.domain, &request.question);
    let contract_id = contract.contract_id.clone();
    harness::append_contract(root, &contract)?;

    let source = direct_pdf_source(pdf_url)?;
    let bundle_path = direct_source_bundle_path(
        root,
        timestamp_secs,
        request.task_id.as_deref(),
        request.domain,
    );
    let bundle = SourceFetchBundle {
        timestamp_secs,
        task_id: request.task_id.clone(),
        domain: request.domain,
        question: request.question.clone(),
        query: pdf_url.to_string(),
        sources: vec![source.clone()],
        errors: Vec::new(),
        safety_findings: Vec::new(),
        bundle_path: bundle_path.clone(),
    };
    write_direct_bundle(&bundle)?;

    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("BusterPaperAcquisition/0.1 (direct open PDF reader)")
        .build()
        .unwrap_or_else(|_| Client::new());

    let mut downloaded = Vec::new();
    let mut errors = Vec::new();
    match download_open_pdf(root, &client, &source, pdf_url, request.domain) {
        Ok(paper) => downloaded.push(paper),
        Err(error) => errors.push(format!("{}: {error}", source.source_id)),
    }

    let status = if downloaded.is_empty() {
        "partial_or_failed"
    } else {
        "ok"
    }
    .to_string();
    let record = PaperAcquisitionRecord {
        timestamp_secs,
        contract_id: contract_id.clone(),
        task_id: request.task_id,
        domain: request.domain,
        question: request.question,
        status: status.clone(),
        source_bundle_path: bundle_path,
        downloaded,
        skipped: Vec::new(),
        errors,
        manifest_path: root.join(PAPER_MANIFEST),
    };
    for paper in &record.downloaded {
        append_manifest(root, timestamp_secs, &contract_id, &record, paper)?;
    }
    append_daemon_jsonl(&root.join(PAPER_ACQUISITION_AUDIT), &record)?;

    let mut outcome = harness::HarnessOutcome::new(
        contract_id,
        if record.downloaded.is_empty() {
            harness::HarnessStatus::Partial
        } else {
            harness::HarnessStatus::Completed
        },
        format!(
            "direct PDF acquisition {status}: downloaded {}, errors {}",
            record.downloaded.len(),
            record.errors.len()
        ),
    )
    .with_evidence(
        "source_bundle",
        record.source_bundle_path.display().to_string(),
        "direct PDF URL preserved as source evidence",
    )
    .with_output("audit", PAPER_ACQUISITION_AUDIT, "paper acquisition audit")
    .with_output("manifest", PAPER_MANIFEST, "PaperQA corpus manifest");
    for paper in &record.downloaded {
        outcome = outcome.with_output(
            "paper_pdf",
            paper.file_path.display().to_string(),
            "downloaded direct open PDF",
        );
    }
    if !record.errors.is_empty() {
        outcome = outcome.with_error(record.errors.join(" | "));
    }
    harness::append_outcome(root, &outcome)?;
    Ok(record)
}

pub fn tail(root: &Path, max_lines: usize) -> std::io::Result<Vec<String>> {
    let text = fs::read_to_string(root.join(PAPER_ACQUISITION_AUDIT)).unwrap_or_default();
    let mut lines = text.lines().map(ToString::to_string).collect::<Vec<_>>();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    Ok(lines)
}

fn ensure_layout(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join(PAPERQA_PAPER_DIR))?;
    fs::create_dir_all(root.join("audit"))?;
    Ok(())
}

fn resolve_open_pdf_url(source: &ResearchSource) -> Option<String> {
    if source.provider == SourceProvider::DirectPdf {
        let url = source.url.as_deref()?.trim();
        if direct_pdf_url(url).is_some() {
            return Some(url.to_string());
        }
        return None;
    }
    if source.provider != SourceProvider::Arxiv {
        return None;
    }
    let url = source.url.as_deref()?;
    arxiv_pdf_url(url)
}

fn arxiv_pdf_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let id = trimmed
        .strip_prefix("http://arxiv.org/abs/")
        .or_else(|| trimmed.strip_prefix("https://arxiv.org/abs/"))
        .or_else(|| trimmed.strip_prefix("http://export.arxiv.org/abs/"))
        .or_else(|| trimmed.strip_prefix("https://export.arxiv.org/abs/"))?;
    let id = id.trim_end_matches(".pdf").trim_matches('/');
    if id.is_empty() || id.contains("..") {
        return None;
    }
    Some(format!("https://arxiv.org/pdf/{id}.pdf"))
}

fn download_open_pdf(
    root: &Path,
    client: &Client,
    source: &ResearchSource,
    pdf_url: &str,
    domain: ResearchDomain,
) -> std::io::Result<DownloadedPaper> {
    let allowed_hosts = allowed_pdf_hosts(source, pdf_url)?;
    let allowed_host_refs = allowed_hosts.iter().map(String::as_str).collect::<Vec<_>>();
    body_gate_bridge::preflight_network(root, pdf_url, &allowed_host_refs)
        .map_err(std::io::Error::other)?;
    let response = client
        .get(pdf_url)
        .header("Accept", "application/pdf")
        .send()
        .map_err(std::io::Error::other)?
        .error_for_status()
        .map_err(std::io::Error::other)?;
    if response.content_length().unwrap_or(0) as usize > MAX_PDF_BYTES {
        return Err(std::io::Error::other("PDF exceeds maximum allowed size"));
    }
    let bytes = response.bytes().map_err(std::io::Error::other)?;
    if bytes.len() > MAX_PDF_BYTES {
        return Err(std::io::Error::other("PDF exceeds maximum allowed size"));
    }
    if !bytes.starts_with(b"%PDF") {
        return Err(std::io::Error::other("downloaded content is not a PDF"));
    }
    let sha256 = sha256_hex(&bytes);
    let file_path = paper_path(root, domain, source, &sha256);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&file_path, &bytes)?;
    let (text_path, text_chars) =
        extract_pdf_text(root, domain, source, &file_path, &sha256).unwrap_or((None, 0));
    let paper = DownloadedPaper {
        source_id: source.source_id.clone(),
        title: source.title.clone(),
        provider: source.provider,
        open_access_kind: "arxiv".to_string(),
        landing_url: source.url.clone(),
        pdf_url: pdf_url.to_string(),
        file_path,
        text_path,
        text_chars,
        sha256,
        bytes: bytes.len(),
    };
    let json = serde_json::to_string(&paper).map_err(std::io::Error::other)?;
    body_gate_bridge::record_memory_write(
        root,
        "paper_manifest_candidate",
        "paper_acquisition.download_open_pdf",
        &json,
    )
    .map_err(std::io::Error::other)?;
    Ok(paper)
}

fn allowed_pdf_hosts(source: &ResearchSource, pdf_url: &str) -> std::io::Result<Vec<String>> {
    if source.provider == SourceProvider::DirectPdf {
        let url = reqwest::Url::parse(pdf_url).map_err(std::io::Error::other)?;
        if url.scheme() != "https" {
            return Err(std::io::Error::other("direct PDF URLs must use https"));
        }
        let host = url
            .host_str()
            .ok_or_else(|| std::io::Error::other("direct PDF URL has no host"))?
            .to_ascii_lowercase();
        return Ok(vec![host]);
    }
    Ok(OPEN_PAPER_ALLOWED_HOSTS
        .iter()
        .map(|host| (*host).to_string())
        .collect())
}

fn append_manifest(
    root: &Path,
    timestamp_secs: u64,
    contract_id: &str,
    record: &PaperAcquisitionRecord,
    paper: &DownloadedPaper,
) -> std::io::Result<()> {
    let manifest_record = serde_json::json!({
        "timestamp_secs": timestamp_secs,
        "contract_id": contract_id,
        "task_id": record.task_id,
        "domain": record.domain,
        "question": record.question,
        "source_id": paper.source_id,
        "title": paper.title,
        "provider": paper.provider,
        "open_access_kind": paper.open_access_kind,
        "landing_url": paper.landing_url,
        "pdf_url": paper.pdf_url,
        "file_path": paper.file_path,
        "text_path": paper.text_path,
        "text_chars": paper.text_chars,
        "sha256": paper.sha256,
        "bytes": paper.bytes,
    });
    let json = serde_json::to_string(&manifest_record).map_err(std::io::Error::other)?;
    body_gate_bridge::record_memory_write(
        root,
        "paper_manifest",
        "paper_acquisition.append_manifest",
        &json,
    )
    .map_err(std::io::Error::other)?;
    append_jsonl(&root.join(PAPER_MANIFEST), &manifest_record)
}

fn append_jsonl(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value).map_err(std::io::Error::other)?;
    file.write_all(b"\n")
}

fn paper_path(
    root: &Path,
    domain: ResearchDomain,
    source: &ResearchSource,
    sha256: &str,
) -> PathBuf {
    let id = source
        .url
        .as_deref()
        .and_then(arxiv_id_for_stem)
        .unwrap_or_else(|| source.source_id.clone());
    root.join(PAPERQA_PAPER_DIR)
        .join(format!("{:?}", domain))
        .join(format!("{}-{}.pdf", safe_file_stem(&id), &sha256[..12]))
}

fn extract_pdf_text(
    root: &Path,
    domain: ResearchDomain,
    source: &ResearchSource,
    pdf_path: &Path,
    sha256: &str,
) -> std::io::Result<(Option<PathBuf>, usize)> {
    let python = root
        .join("tools")
        .join("paperqa")
        .join(".venv")
        .join("bin")
        .join("python");
    if !python.is_file() {
        return Ok((None, 0));
    }
    let id = source
        .url
        .as_deref()
        .and_then(arxiv_id_for_stem)
        .unwrap_or_else(|| source.source_id.clone());
    let text_path = root
        .join(PAPER_TEXT_DIR)
        .join(format!("{:?}", domain))
        .join(format!("{}-{}.txt", safe_file_stem(&id), &sha256[..12]));
    if let Some(parent) = text_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let script = r#"
from pathlib import Path
from pypdf import PdfReader
import sys
pdf_path = Path(sys.argv[1])
out_path = Path(sys.argv[2])
reader = PdfReader(str(pdf_path))
parts = []
for index, page in enumerate(reader.pages):
    text = page.extract_text() or ""
    if text.strip():
        parts.append(f"\n\n[page {index + 1}]\n{text}")
content = "".join(parts).strip()
out_path.write_text(content[:200000], encoding="utf-8")
"#;
    let output = Command::new(python)
        .arg("-c")
        .arg(script)
        .arg(pdf_path)
        .arg(&text_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;
    if !output.status.success() || !text_path.exists() {
        return Ok((None, 0));
    }
    let text = fs::read_to_string(&text_path).unwrap_or_default();
    let chars = text.chars().count();
    Ok((Some(text_path), chars))
}

fn arxiv_id_for_stem(url: &str) -> Option<String> {
    let id = url
        .trim()
        .strip_prefix("http://arxiv.org/abs/")
        .or_else(|| url.trim().strip_prefix("https://arxiv.org/abs/"))?;
    Some(id.trim_matches('/').to_string())
}

fn direct_pdf_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url.trim()).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let path = parsed.path().to_ascii_lowercase();
    if !path.ends_with(".pdf") {
        return None;
    }
    parsed.host_str()?;
    Some(parsed.to_string())
}

fn direct_pdf_source(pdf_url: &str) -> std::io::Result<ResearchSource> {
    let normalized = direct_pdf_url(pdf_url)
        .ok_or_else(|| std::io::Error::other("only direct https PDF URLs are accepted"))?;
    let title = direct_pdf_title(&normalized);
    Ok(ResearchSource {
        source_id: format!("DirectPdf-{}", &sha256_hex(normalized.as_bytes())[..16]),
        provider: SourceProvider::DirectPdf,
        title,
        url: Some(normalized),
        doi: None,
        year: None,
        authors: Vec::new(),
        venue: Some("direct PDF URL".to_string()),
        summary: Some("Direct PDF supplied by a human through the Buster web console.".to_string()),
    })
}

fn direct_pdf_title(pdf_url: &str) -> String {
    reqwest::Url::parse(pdf_url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back().map(ToOwned::to_owned))
        })
        .map(|name| name.trim_end_matches(".pdf").replace(['-', '_'], " "))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "direct PDF".to_string())
}

fn direct_source_bundle_path(
    root: &Path,
    timestamp_secs: u64,
    task_id: Option<&str>,
    domain: ResearchDomain,
) -> PathBuf {
    let task = task_id
        .map(safe_file_stem)
        .unwrap_or_else(|| "direct-pdf".to_string());
    root.join("research")
        .join("sources")
        .join(format!("{timestamp_secs}-{:?}-{task}.json", domain))
}

fn write_direct_bundle(bundle: &SourceFetchBundle) -> std::io::Result<()> {
    if let Some(parent) = bundle.bundle_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(bundle).map_err(std::io::Error::other)?;
    fs::write(&bundle.bundle_path, json)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn skip(source: &ResearchSource, reason: &str) -> SkippedPaper {
    SkippedPaper {
        source_id: source.source_id.clone(),
        title: source.title.clone(),
        provider: source.provider,
        reason: reason.to_string(),
    }
}

fn safe_file_stem(value: &str) -> String {
    let stem = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let stem = stem.trim_matches('-');
    if stem.is_empty() {
        "paper".to_string()
    } else {
        stem.chars().take(96).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_arxiv_pdf_url() {
        assert_eq!(
            arxiv_pdf_url("http://arxiv.org/abs/2401.00001v1").unwrap(),
            "https://arxiv.org/pdf/2401.00001v1.pdf"
        );
    }

    #[test]
    fn rejects_non_arxiv_pdf_url() {
        assert!(arxiv_pdf_url("https://example.com/paper").is_none());
    }

    #[test]
    fn accepts_direct_https_pdf_url() {
        assert_eq!(
            direct_pdf_url("https://picrew.github.io/LLM-Harness/main.pdf").unwrap(),
            "https://picrew.github.io/LLM-Harness/main.pdf"
        );
    }

    #[test]
    fn rejects_non_https_direct_pdf_url() {
        assert!(direct_pdf_url("http://picrew.github.io/LLM-Harness/main.pdf").is_none());
    }
}
