# Secondary Research: BrainComputerInterface

- task_id: followup-1780913155-a4bb1885cda3
- question: Are there any published datasets or benchmarks specifically designed to evaluate BCI alignment safety (e.g., misspecification, reward hacking, silent failures)?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 9
- source_bundle: ../research/sources/1780927942-BrainComputerInterface-followup-1780913155-a4bb1885cda3.json

1. Follow-up questions  
- Have any BCI research groups explicitly defined “alignment failure modes” (e.g. silent decoding drift, reward misspecification in closed-loop stimulation) as a benchmarkable problem?  
- Which EEG/ECoG datasets include simultaneous behavioural or self-report ground truth that could be repurposed to measure unintended decoder behaviour?  
- Are there adversarial robustness benchmarks for BCI classifiers that could serve as a weak proxy for alignment-safety evaluation?  
- What safety-adjacent measures (e.g. uncertainty calibration, out-of-distribution detection, test-retest stability) are already tracked in BCI clinical trials or implant studies, and could their data be re-analysed as alignment check suites?  

2. Verification leads  
- `Brain-Computer Interface and Brain-Computer Fusion` (2026, DOI: 10.1007/978-981-95-5585-7_6) – untrusted until read; skim for safety terminology, explicit alignment framing.  
- Search query: `"brain-computer interface" (alignment OR safety) benchmark dataset` on arXiv, IEEE Xplore, ACM DL.  
- Check BCI competition datasets (e.g. BCI Competition IV, BNCI Horizon 2020) for any post-hoc analyses of silent failure or reward hacking.  
- Monitor the `NeuroBench` project (https://neurobench.ai) – hardware-oriented, but may contain algorithmic safety metrics as secondary outputs.  
- Look for registries of clinical trials using closed-loop BCIs (e.g. ClinicalTrials.gov) and inspect safety endpoints; those are not published benchmarks but could contain raw adverse-event logs that relate to misspecification.  

3. Working answer (high uncertainty)  
I found no publicly announced dataset or benchmark that is *dedicated* to BCI alignment safety in the sense of misspecification, reward hacking, or silent failure detection. The terms “alignment safety” are mostly absent from current BCI literature; typical safety evaluation focuses on biocompatibility, signal quality, and clinical adverse events. Very few works frame BCI failures as AI alignment problems.  
What might exist are scattered studies on decoder robustness (e.g., adversarial perturbations of EEG, covariate shift detection, or out-of-distribution rejection), which could be reused as partial alignment probes. Also, closed-loop deep-brain stimulation (DBS) and motor neuroprosthetics trials likely contain logs that could be analysed post-hoc for unintended adaptation loops, but these are not released as standardised benchmarks.  
Thus, the most honest answer: no dedicated dataset/benchmark known, but components may be repurposed, and a systematic gap likely exists.  

4. Contradictions or unknowns  
- If a well-hidden dataset from a research group (e.g., Neuralink, Synchron, Blackrock) exists but is not public, I would not have seen it; this answer