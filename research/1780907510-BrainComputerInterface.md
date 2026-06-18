# Secondary Research: BrainComputerInterface

- task_id: seed-bci-001
- question: What are the most credible paths and risks for brain-computer interfaces that improve human-AI cooperation?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 9
- source_bundle: ../research/sources/1780907510-BrainComputerInterface-seed-bci-001.json

### 1. Follow-up questions

- What BCI signal modalities (motor imagery, P300, SSVEP, intracranial LFP, ECoG) offer the highest information transfer rate and reliability for cooperative AI tasks without requiring surgical implantation?
- Which existing human-AI teaming paradigms (shared autonomy, predictive intention decoding, error-related potential correction) would benefit most from a high-bandwidth BCI channel, and what evidence exists for each?
- What are the known neural security vulnerabilities (e.g., adversarial stimuli, subliminal priming via sensory BCI, brain malware in closed-loop implants) that could degrade cooperation or enable manipulation, and have any been demonstrated outside simulation?
- How does human trust in AI change when the AI can read neural signals correlated with intention, emotional state, or cognitive load, and what experimental designs could isolate the BCI-mediated component from other interface factors?
- What concrete BCI-alignment protocols (mutual adaptation, neural reinforcement learning with human-in-the-loop, corretable interfaces) have been proposed beyond high-level principles, and which have empirical results?

### 2. Verification leads

- Search "LFP-based BCI shared control human-robot teaming" to find experimental papers measuring cooperation indices. Uncertain whether any involve AI agents rather than simple robotic controllers.
- The ArXiv paper “Dynamic Neural Communication …” (2024) might discuss real-time vision-to-brain loops; check if it quantifies cooperation improvements or merely proposes architecture.
- “Towards Neurohaptics …” (2020) could provide touch feedback pathways relevant to grounding AI commands in sensory experience, but may be limited to prosthesis control.
- The review “Brain-Computer Interface and Brain-Computer Fusion” (2026, DOI: 10.1007/978-981-95-5585-7_6) is a lead to scan for human-AI cooperation frameworks, though it may be broad.
- OpenAlex work on existential risks (2002) by Bostrom – relevant for mapping BCI misuse risks; need to see if it specifically addresses BCI as an AI cooperation channel.
- Datasets: PhysioNet EEG motor imagery, BCI Competition IV (2a, 2b) for offline testing of decoding pipelines. Human-robot interaction datasets with EEG: “Brain-computer interface based human-robot interaction platform” (2015) might have recorded trials, verify if data is public.
- Search “adversarial EEG attacks” and “brain-computer interface privacy” on IEEE Xplore/ArXiv for security risks.

### 3. Working answer

The most credible paths to improving human-AI cooperation via