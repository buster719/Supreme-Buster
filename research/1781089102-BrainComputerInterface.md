# Secondary Research: BrainComputerInterface

- task_id: followup-1780978414-22cd1991e436
- question: What neurophysiological markers (P300, ERN, frontal theta) covary with subjective trust erosion when the AI repeatedly contradicts self-reports?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 6
- source_bundle: ../research/sources/1781089102-BrainComputerInterface-followup-1780978414-22cd1991e436.json

## 1. Follow-up Questions
- Does the P300 amplitude decrease specifically during AI contradictions compared to human contradictions, or is it a general response to any detected inconsistency?
- Is frontal theta power increase during trust erosion correlated with a subjective report of "confusion" versus "deliberate distrust"?
- Can the Error-Related Negativity (ERN) be elicited by an AI's external error, or is it limited to one's own action outcomes?
- Does repeated exposure to contradictory AI self-reports lead to a measurable decline in skin conductance response (SCR), indicating a shift from emotional to more cognitive processing of errors?
- Are there distinct neural signatures that differentiate between trust erosion due to factual inaccuracy versus perceived deception?

## 2. Verification Leads
- **Paper Search:** "frontal midline theta trust prediction artificial agent" or "P300 amplitude trust violation humanoid robot". Use Google Scholar, PsycINFO, or arXiv for recent preprints.
- **Dataset:** Look for the "Trust Game with AI Agents" datasets mentioned in human-computer interaction (HCI) or neuroeconomics literature. Check OpenNeuro for EEG datasets with behavioral trust measures.
- **Official Source:** The IEEE Brain Initiative or the BCI Society often publish consensus papers on neural correlates of cognitive states.
- **Search Query:** `("error related negativity" OR "ERN") AND ("trust" OR "distrust") AND ("AI" OR "agent") site:.edu OR site:.org`
- **Uncertain Lead:** The claim about FRN (Feedback-Related Negativity) covarying with trust is plausible but requires a source that uses social feedback from an AI, not just monetary outcomes.

## 3. Claim Table

| Claim | Source Support | Uncertainty | Falsifier |
|-------|----------------|-------------|-----------|
| **Frontal midline theta (FMT) power increases during cognitive conflict and could covary with trust erosion when processing contradictory AI statements.** | Well-established link between FMT and conflict monitoring/error detection in human cognition. Some HCI studies link theta to "mental effort" with AI. | No direct study found on *subjective trust erosion* with AI contradictions specifically. Correlation ≠ causation; theta may reflect general cognitive load, not trust-specific processing. | An experiment showing significant theta increase during contradictory AI statements *without* any change in reported trust. |
| **P300 amplitude (a marker of context updating) may attenuate with