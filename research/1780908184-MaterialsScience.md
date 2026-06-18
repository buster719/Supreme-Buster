# Secondary Research: MaterialsScience

- task_id: followup-1780905233-e7ab25deb0bb
- question: What is the current specific energy (Wh/kg) and cycle life of carbon nanotube yarn artificial muscles compared to dielectric elastomer actuators, and where is the reliability floor for sub-millimeter cyclic strain control?
- source_type: llm_secondary_with_fetched_evidence
- needs_verification: true
- authority: hypothesis_or_explanation
- source_count: 6
- source_bundle: ../research/sources/1780908184-MaterialsScience-followup-1780905233-e7ab25deb0bb.json

1. Follow-up questions:
- What are the latest measured specific energy (Wh/kg) of twist-spun CNT yarn muscles when driven electrothermally versus electrochemically, and how do these compare to coiled polymer fibers?
- For dielectric elastomer actuators (DEAs), what is the demonstrated cycle life at >10% strain under sustained sub-millimeter closed-loop control, and what failure modes dominate?
- Has any group directly compared CNT yarn muscles and DEAs on a common task metric such as work density (J/kg) or strain rate under equivalent load?
- What is the drift in strain output over 10^5 cycles for CNT yarn muscles when the control signal is open-loop voltage, and what sensor bandwidth is needed to compensate?
- Where does the reliability floor for sub-mm strain control in DEAs degrade due to electromechanical instability or electrode crack propagation, and how does pre-stretch affect it?

2. Verification leads:
- Review papers: “Artificial Muscles: Mechanisms, Applications, and Challenges” (Mirvakili & Hunter, 2018) for comparative tables on CNT yarns and DEAs; check updates.
- Search for “specific energy carbon nanotube yarn muscle Wh/kg” in full-text on IEEE Xplore, ScienceDirect, and ArXiv, filtering last 5 years.
- For DEA cycle life, look for datasets from EMPA (Swiss Federal Laboratories) or the Soft Transducers Lab (EPFL); they publish lifetime curves.
- Possible data source: “OpenTorque actuator dataset” on GitHub may include CNT muscle or DEA benchmarks. Uncertainty: may not contain side-by-side comparisons.
- The fetched source “Biology and bioinspiration of soft robotics” (2021) could have review references on CNT yarn muscles, but not primary data.
- ArXiv:2511.15467v1 (2025) on CNT fragmentation might address mechanical failure modes during cyclic loading; needs direct reading to assess relevance.
- Search query: “carbon nanotube yarn muscle cycle life hysteresis” and “dielectric elastomer actuator reliability sub-millimeter strain”.

3. Working answer:
A direct like-for-like comparison of CNT yarn muscles and dielectric elastomer actuators in terms of specific energy and cycle life under sub-mm strain control is not well-established in a single study. Based on secondary LLM synthesis, the following tentative ranges emerge:

CNT yarn artificial muscles (twist-spun, electrothermally driven): Specific energy seems low, likely <0.1 Wh/kg in most reported configurations, though some exotic multi-walled CNT yarns may approach ~0.2 Wh/kg. Cycle life can exceed 10^5 cycles when strain is small (<1% peak-to-peak), but reports often note creep and permanent set over time, reducing precision. Sub-mm strain