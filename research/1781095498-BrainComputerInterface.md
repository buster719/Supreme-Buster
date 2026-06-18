# Secondary Research: BrainComputerInterface

- task_id: followup-1780982909-986b7ebd8f3f
- question: How do recent shared‑control frameworks (hybrid BCI + autonomy) change the minimum useful ITR for practical prosthesis or vehicle control?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 6
- source_bundle: ../research/sources/1781095498-BrainComputerInterface-followup-1780982909-986b7ebd8f3f.json

**Follow-up Questions:**
1.  What empirical studies, particularly since 2020, have measured BCI user performance (speed, accuracy, task completion) as a function of shared-control autonomy level, and how did they report or calculate the effective or required Information Transfer Rate (ITR)?
2.  How do specific shared-control architectures (e.g., hierarchical, predictive, shared-initiative) quantitatively affect the signal-to-noise ratio or error tolerance requirements of the BCI decoder, and can this be translated into a theoretical reduction in minimum ITR?
3.  For a given real-world task (e.g., a multi-degree-of-freedom prosthesis pick-and-place, or wheelchair navigation in a cluttered environment), what is the measurable "minimum useful ITR" benchmark under 100% manual control versus a validated shared-control system?
4.  Are there formal models or simulation frameworks (e.g., using reinforcement learning, Bayesian decision theory) that predict the functional relationship between shared-control autonomy level and the BCI channel's required information rate?
5.  How do user learning, cognitive load, and error-related potentials interact with shared control to dynamically alter the effective ITR threshold over a session?

**Verification Leads (Uncertain, need primary source check):**
*   **Key Papers/Authors:** Search for recent work by José del R. Millán, Surjo R. Soekadar, or Andrea Kübler on shared-control BCIs. Look for empirical performance metrics in their systems.
*   **Datasets:** Search BCI competition datasets or public repositories (e.g., OpenNeuro) for shared-control paradigms. Analysis of these could reveal ITR in hybrid modes.
*   **Search Queries:**
    *   `"shared control" AND "BCI" AND ("information transfer rate" OR "ITR" OR "bits per minute")`
    *   `"hybrid BCI" AND "autonomy" AND "prosthetic" AND "performance metrics"`
    *   `"minimum channel capacity" "