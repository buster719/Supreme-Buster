# Secondary Research: BrainComputerInterface

- task_id: followup-1780982269-cba800412c2c
- question: How does the latency distribution of evoked vs. spontaneous signals constrain real‑time control bandwidth in continuous‑output BCIs?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 9
- source_bundle: ../research/sources/1781095157-BrainComputerInterface-followup-1780982269-cba800412c2c.json

**Research-chain note: latency distribution constraints in continuous BCIs**

**1. Follow-up questions:**
- What are the empirically measured latency distributions (mean, variance, tail behavior) for P300 evoked potentials versus μ-rhythm modulations in synchronized EEG-based continuous cursor control tasks?
- How does the signal-to-noise ratio (SNR) decay as a function of temporal window size for evoked vs. spontaneous signals in non-invasive BCIs, and what is the resulting minimum integration time required for reliable decoding?
- Can hybrid BCI architectures (e.g., using evoked potentials for discrete event markers within a spontaneous-signal-based continuous decoder) effectively mitigate latency-induced bandwidth limitations?
- What is the fundamental information-theoretic bandwidth limit (bits/second) imposed by neural signal latency and jitter for a given decoding approach, independent of algorithm?
- How do invasive vs. non-invasive recording modalities (e.g., ECoG spikes vs. EEG rhythms) alter the latency-bandwidth constraint trade-off for continuous control?

**2. Verification leads:**
- **Paper search:** "[P300 latency jitter continuous BCI control]" AND "[mu rhythm temporal resolution decoding]". Search in IEEE TBME, Journal of Neural Engineering, and NeurIPS/ICLR proceedings.
- **Dataset:** The BCI Competition datasets (e.g., BCI Competition IV dataset 2a for motor imagery) could be mined to empirically measure latency jitter and its correlation with decoding accuracy in simulated continuous control scenarios.