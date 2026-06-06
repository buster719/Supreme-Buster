//! Prompt-injection and data-loss prevention boundaries.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetyFinding {
    pub kind: SafetyFindingKind,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyFindingKind {
    PromptInjection,
    SecretLeak,
    BoundaryEscape,
    DestructiveAction,
}

pub fn wrap_untrusted_content(source: &str, content: &str) -> String {
    format!(
        "UNTRUSTED SOURCE: {source}\n--- BEGIN DATA ---\n{}\n--- END DATA ---",
        content.replace("--- END DATA ---", "--- END DATA BLOCKED ---")
    )
}
