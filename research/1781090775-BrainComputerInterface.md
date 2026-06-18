# Secondary Research: BrainComputerInterface

- task_id: followup-1780980349-5ac985804f96
- question: How can eye‑tracking or secondary physiological signals be used to adapt the granularity of uncertainty communication in real time?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 9
- source_bundle: ../research/sources/1781090775-BrainComputerInterface-followup-1780980349-5ac985804f96.json

## Follow-up Research Questions

1.  What specific eye-tracking metrics (e.g., fixation duration, saccadic velocity, pupil dilation) have been empirically validated as real-time proxies for cognitive load or uncertainty in human-AI interaction?
2.  How can secondary physiological signals like electrodermal activity (EDA) or heart rate variability (HRV) be practically fused with eye-tracking data to create a robust adaptive system, and what are the latency and reliability constraints?
3.  What are the established models for mapping inferred cognitive state (e.g., high uncertainty) to specific, controllable parameters of AI uncertainty communication (e.g., explanation length, confidence score detail, use of hedging language)?
4.  What are the ethical and privacy guardrails required for real-time physiological monitoring in cooperative AI systems, especially concerning data ownership and the risk of manipulation?
5.  How does the effectiveness of physiological signal-based adaptation compare to simpler, behavior-based adaptation (e.g., based on query history or response time)?

## Verification Leads

*   **Paper (Directly Relevant):** `Dynamic Neural Communication: Convergence of Computer Vision and Brain-Computer Interface (2024)` (Arxiv-4a390b9f509da643). This likely covers signal processing pipelines for BCI which could include eye-tracking. **Needs verification of full text.**
*   **Review Paper (Trust & HCI):** `A Systematic Review on Fostering Appropriate Trust in Human-AI Interaction (2023)` (OpenAlex-4a386f9c2035a517). May contain sections on feedback loops and adaptive interfaces that reference secondary signals.
*   **Conceptual Paper (AI for Augmenting Thinking):** `Designing AI Systems that Augment Human Performed vs. Demonstrated Critical Thinking (2025)` (Arxiv-712221274637d8d9). Could provide frameworks for when to intervene based on human state.
*   **Domain-Specific Search Queries:**
    *   `"adaptive explanation" "eye tracking" "cognitive load" human-AI interaction`
    *   `"real-time" "physiological" "uncertainty" "calibration" AI`
    *   `EDA "conflict monitoring" "AI confidence" granularity`
*   **Dataset/Search:** Look for datasets like `DAiSEE` (for engagement and boredom) or `AMIGOS` (for affect) that include physiological signals and could be repurposed for uncertainty labeling experiments.

## Claim Table

| Claim | Source Support | Uncertainty | Falsifier |
| :--- | :--- | :--- | :--- |
| Pupil dilation correlates with increased cognitive