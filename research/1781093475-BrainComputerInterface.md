# Secondary Research: BrainComputerInterface

- task_id: followup-1780981945-9140e6d2d383
- question: What concrete definitions of “effective bandwidth” have been proposed that separate user-intent information from machine-projected intent in shared-autonomy BCI?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 9
- source_bundle: ../research/sources/1781093475-BrainComputerInterface-followup-1780981945-9140e6d2d383.json

**Follow-up Questions:**
1.  What are the standard, quantifiable metrics used to evaluate information transfer in single-user BCI systems (e.g., BCI classification accuracy, bits-per-minute, and information transfer rate)?
2.  How do shared-autonomy frameworks (e.g., shared control, adaptive autonomy) define the *blend* or *handover* between user input and autonomous agent action? What mathematical models describe this?
3.  In the literature on brain-computer interface and robotics fusions, are there any proposed metrics for disentangling the contributions of human neural commands versus predictive machine models to the final action?
4.  Can methods from information theory, such as partial information decomposition or transfer entropy, be adapted to measure the unique, synergistic, and redundant information flow between a user's neural signals and an AI controller?
5.  Are there any controlled experiments in the open BCI literature that explicitly varied autonomy levels and measured user performance using metrics that could be interpreted as "effective bandwidth" for intent?

**Verification Leads (Uncertain):**
*   **Search Query:** "Information transfer rate" AND "shared autonomy" AND "brain-computer interface" on Google Scholar or Semantic Scholar.
*   **Search Query:** "Mutual information" AND "user intent" AND "autonomous agent" AND "BCI".
*   **Source to Examine:** The fetched Crossref document "Brain-Computer Interface and Brain-Computer Fusion (2026)" (DOI: 10.1007/978-981-95-5585-7_6) may contain relevant discussion. Its 2026 date suggests it's a very recent or future publication, possibly a book chapter. This requires verification.
*   **Dataset/Method:** Analyze open datasets from shared-control BCI experiments (e.g., for wheelchair navigation or robotic arm control). Look for raw neural data, discrete command signals, and the resulting trajectory. A simple correlation or information-theoretic analysis between user commands and final path could be a starting point.
*   **Systematic Review Search:** Look for