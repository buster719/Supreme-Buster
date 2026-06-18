# Secondary Research: BrainComputerInterface

- task_id: followup-1780931793-5497ed68036b
- question: In shared‑autonomy frameworks that learn a user’s intent over time, does the effective bandwidth *increase* as the machine model becomes more personalised, making direct comparison across bandwidth levels insufficient?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 6
- source_bundle: ../research/sources/1780981917-BrainComputerInterface-followup-1780931793-5497ed68036b.json

1. Follow-up questions
- What concrete definitions of “effective bandwidth” have been proposed that separate user-intent information from machine-projected intent in shared-autonomy BCI?
- Under what conditions does personalisation improve task completion rate but reduce the measured information transfer rate, and does that trade-off appear in real BCI data?
- How do model confidence and uncertainty in intent prediction influence the user’s residual control signals, and does that feed back into bandwidth measurements over time?
- Are there existing datasets where the same user performs identical tasks while the machine’s intent model is deliberately varied across different personalisation depths?

2. Verification leads
- Search arxiv, IEEE Xplore, PubMed for: “shared autonomy” AND “information transfer rate” AND “personalization” or “user model adaptation”, especially works that measure both bitrate and task metrics.
- Examine the BCI Competition datasets (e.g., IV-2a motor imagery, Wadsworth) for any that include trials with adaptive classifiers; check if raw EEG and task labels allow re-analysis with a simulated adaptive system.
- Look for explicit disambiguation in: Tonin & Mill