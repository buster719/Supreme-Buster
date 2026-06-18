# Secondary Research: BrainComputerInterface

- task_id: followup-1781087463-8b66e040b030
- question: What empirical studies have directly measured user trust (self-report or behavioral) in BCI systems before, during, and after an adaptive decoder is removed or replaced?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 9
- source_bundle: ../research/sources/1781793106-BrainComputerInterface-followup-1781087463-8b66e040b030.json

# Research Chain Note

**Task:** followup-1781087463-8b66e040b030
**Domain:** BrainComputerInterface
**Question:** What empirical studies have directly measured user trust (self-report or behavioral) in BCI systems before, during, and after an adaptive decoder is removed or replaced?

---

## 1. Follow-up Questions

1. Are there any BCI studies that use the "trust battery" paradigm (pre/during/post exposure) specifically around decoder swaps, or has this only been done in non-BCI automation contexts (e.g., Lee & See 2004)?
2. Does the Frontiers in Human Neuroscience 2024 paper on BCI evaluation methods (DOI: 10.3389/fnhum.2024.1429130) include any trust measurement instruments applied to decoder-replacement scenarios?
3. Have any EEG-based BCI studies measured neural correlates of "surprise" or "expectation violation" when a well-calibrated decoder is suddenly swapped, as a proxy for trust disruption?
4. In closed-loop BCI experiments where decoder parameters drift or are reset mid-session, do behavioral metrics (e.g., trial completion rate, reaction time, voluntary session length) correlate with any self-reported trust measures?
5. Could Buster reconstruct a trust-removal proxy by comparing BCI "unlearning" or "washout" studies (where decoder parameters are reset) with analogous human-automation trust recovery studies?

---

## 2. Verification Leads

| # | Lead | Type | Uncertainty |
|---|------|------|-------------|
| V1 | Frontiers paper: "Comprehensive evaluation methods for translating BCI into practical applications" (2024), DOI 10.3389/fnhum.2024.1429130 | Full paper | Uncertain whether it covers decoder-removal trust specifically |
| V2 | Springer: "Human factors engineering of BCI" (2022), DOI 10.1007/s11571-022-09808-z | Full paper | Likely covers user factors; trust subsection unconfirmed |
| V3 | Frontiers in Psychology: "Measurement of Trust in Automation" (2021), DOI 10.3389/fpsyg.2021.604977 | Review paper | Known to cover trust scales; BCI applicability uncertain |
| V4 | Search query: `("brain-computer interface" OR BCI) AND ("trust" OR "user confidence") AND ("decoder" OR "classifier") AND ("removal" OR "replacement" OR "swap")` on PubMed/Google Scholar | Deductive search | May return zero direct hits, which itself