# Secondary Research: BrainComputerInterface

- task_id: followup-1780981945-ed9b21f25444
- question: Are there existing datasets where the same user performs identical tasks while the machine’s intent model is deliberately varied across different personalisation depths?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 9
- source_bundle: ../research/sources/1781092460-BrainComputerInterface-followup-1780981945-ed9b21f25444.json

# Research-Chain Note: BCI Datasets with Varied Intent-Model Personalisation Depths

---

## 1. Follow-up Questions

- **Q1:** In standard BCI decoder calibration studies (e.g., EEG P300 spellers, motor-imagery BCIs), do any published experiments explicitly compare the *same user, same task* against multiple decoder variants that differ in personalisation depth (e.g., generic → session-calibrated → user-adapted → user-adapted with history), while logging behavioural and neural outcomes together?
- **Q2:** Does the "adaptive BCI" literature on co-adaptive calibration (e.g., Faller et al., Vidaurre et al.) produce or release the raw trial-level data needed to reconstruct such a comparison post-hoc, even when not originally framed as a personalisation-depth experiment?
- **Q3:** Are there datasets in the broader human-AI interaction literature (not BCI-specific) where the same human performs identical tasks while the AI assistant's "intent model" or recommendation policy is deliberately varied across user-model sophistication levels?
- **Q4:** What would a minimal protocol look like to generate a synthetic version of such a dataset using publicly available BCI motor-imagery datasets (e.g., BCI Competition IV, PhysioNet EEG) by re-decoding with models of varying personalisation depth?

---

## 2. Verification Leads

| Lead | Type | Uncertainty |
|------|------|-------------|
| **BCI Competition IV datasets** (esp. 2a, 2b) — multi-subject motor-imagery EEG with labelled trials. Could be re-decoded with generic vs. user-specific models to simulate personalisation-depth variation. | Dataset | Available on official site; labelled and well-documented. |
| **PhysioNet EEG Motor Movement/Imagery Dataset** — 109 subjects, multiple sessions, multiple runs. Allows same-subject-same-task comparison across sessions with different decoder calibration states. | Dataset | Public, but not originally designed for this comparison. |
| **Faller et al. (2012), Vidaurre et al. (2011)** — co-adaptive BCI calibration papers. They explicitly vary the decoder's adaptation state while the user performs the same task, and some include per-trial performance logs. | Papers | Accessible via IEEE/journal archives. Raw data release uncertain