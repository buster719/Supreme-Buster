# Secondary Research: BrainComputerInterface

- task_id: followup-1780980349-2e0a3ac13f5c
- question: How does the frequency of discrete doubt messages influence the “cry wolf” effect in BCI‑based motor imagery control?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 6
- source_bundle: ../research/sources/1781091785-BrainComputerInterface-followup-1780980349-2e0a3ac13f5c.json

### Research-Chain Note: Doubt Message Frequency and the "Cry Wolf" Effect in BCI Motor Imagery

**1. Follow-up Questions**
- At what specific frequency do discrete doubt messages transition from improving user calibration to eroding trust in a BCI system?
- How does the "cry wolf" effect in BCI compare to its manifestation in traditional alarm systems, given the cognitive load of maintaining motor imagery?
- Does the neural correlate of trust erosion (e.g., in the anterior cingulate cortex) differ based on the temporal distribution of false alarms?
- Could adaptive doubt messaging, which varies frequency based on user performance metrics, mitigate the effect more effectively than a fixed schedule?

**2. Verification Leads**
- **Literature Search**: Systematic review of "alarm fatigue" in clinical monitoring and vigilance tasks (e.g., Getty et al., 1995; Bliss et al., 1995) for foundational models. Apply search terms: `"cry wolf" AND "alarm" AND "frequency"`.
- **BCI-Specific Studies**: Search for work on user trust and adaptive feedback in BCI (e.g., Lotte & Roy, 2022; Rashid et al., 2020). Look for empirical data on false positive feedback rates.
- **Datasets**: The BNCI Horizon 2020 database may contain session logs where false feedback rates varied. Also, simulation environments like BCI2000 or OpenBCI with experiment logs.
- **Theoretical Frameworks**: Cross-reference with "Signal Detection Theory" models of trust in automation (e.g., Parasuraman & Riley, 1997).

**3. Claim Table**

| Claim | Source Support | Uncertainty | Falsifier |
| :--- | :--- | :--- | :--- |
| The "cry wolf" effect occurs when frequent false alarms lead users to ignore true warnings or correct signals. | Foundational alarm research (e.g., Breznitz, 1983; Bliss et al., 1995) across domains. | High: Direct translation to BCI's closed-loop, high-cognitive-load context is unverified. | Controlled BCI study showing maintained user vigilance and trust despite high false-alarm doubt messages