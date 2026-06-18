# VPS Research Review 2026-06-10

## Snapshot

- digest_updated_at: 2026-06-10 14:48:27 +08:00
- ledger_entries: 198
- completed_entries: 69
- failed_entries: 129
- report_count: 188
- fetched_source_count: 1297
- pending_task_count: 199

## Domain Progress

| Domain | Completed | Failed | Pending | Sources |
| --- | ---: | ---: | ---: | ---: |
| Genetics | 23 | 39 | 69 | 446 |
| BrainComputerInterface | 19 | 35 | 62 | 411 |
| Aerospace | 17 | 27 | 41 | 240 |
| MaterialsScience | 7 | 20 | 25 | 173 |
| Robotics | 2 | 1 | 1 | 15 |
| QuantumComputing | 1 | 0 | 0 | 0 |
| NuclearFusion | 0 | 1 | 0 | 0 |
| LifeScienceAndPharma | 0 | 4 | 0 | 9 |
| TheoreticalPhysics | 0 | 2 | 1 | 3 |

## What Worked

Buster did produce a real overnight research trail. The run generated many
source bundles, wrote research reports, maintained a queue, and updated a digest.
Successful reports include useful verification leads and next questions. For
example, the mitochondrial donation genetics reports identify primary legal and
regulatory sources to verify next, rather than only producing loose summaries.

The research system also preserved failed attempts as reports with source
bundles. That is valuable: evidence fetches are not lost when synthesis fails.

## Main Failure Pattern

Most failures are not source-fetch failures. They are synthesis failures after
sources were already fetched.

Observed `audit/research.jsonl` status counts:

- `source_fetched_llm_error`: 119
- `ok`: 84
- `error`: 18
- `research_budget_deferred`: 2

Dominant failure messages:

- `invalid LLM response: assistant content was null`
- `LLM HTTP error: error sending request for url (https://api.xiaomimimo.com/v1/chat/completions)`
- `LLM HTTP error 200: error decoding response body`

This means the VPS gathered evidence but often could not turn it into a
research note. Recent Genetics tasks show the same pattern: source bundles exist,
but the report is only "Source Fetch Completed; LLM Synthesis Failed".

## Research Quality Issues

The source query builder is too broad for deep follow-up questions. Several
recent Genetics questions about PRS reclassification, NRI thresholds, and
cross-ancestry decay retrieved broad gene-editing or genetic-background sources
instead of clinical guideline, PRS, ACC/AHA, USPSTF, or reclassification papers.

Follow-up generation can become recursive. One pending task repeats:

```text
What small local computation, simulation, or table could Buster run next to test
part of this question: What small local computation, simulation, or table could
Buster run next to test part of this question...
```

This is a queue hygiene problem. Meta-follow-up templates should not recursively
wrap themselves.

## Improvement Space

1. Split research failure statuses:
   Use `source_fetch_failed`, `synthesis_failed`, `partial_evidence_only`,
   `synthesized_unverified`, and `verified_summary` instead of generic `error`.

2. Stop scientific follow-up spawning after failed synthesis:
   A source bundle without synthesized content can create a repair task, but it
   should not create scientific child tasks.

3. Add LLM provider fallback:
   If Mimo returns null content or network errors for N consecutive attempts,
   switch to OpenRouter or mark the provider degraded for a cooldown window.

4. Repair null-content handling:
   Some APIs may return reasoning, tool-call, or provider-specific fields with
   null `message.content`. The body LLM parser should either extract a supported
   alternate text field or classify the response as provider-degraded with enough
   metadata to debug safely.

5. Improve source queries:
   Build search queries from named entities and domain keywords in the question,
   not only the broad domain prefix. For example, PRS/NRI questions should keep
   `polygenic risk score`, `net reclassification improvement`, `ACC/AHA`,
   `USPSTF`, `clinical utility`, and disease names.

6. Add source relevance checks:
   Before synthesis, score whether fetched titles/abstracts match the question.
   If relevance is low, re-query instead of spending a synthesis call.

7. Add Harness Contract to research:
   Each research task should declare source acceptance criteria, follow-up spawn
   limits, verification requirements, retry limits, and memory promotion rules.

8. Queue hygiene:
   Deduplicate semantic variants, block recursive meta-question prefixes, cap
   children per parent, and periodically rebalance domains so Genetics and BCI do
   not crowd out Fusion, LifeScience, and TheoreticalPhysics.

## Recommended Next Patch

Implement a research Harness Contract v0 in `buster_daemon`:

- Add a `ResearchContract` or `HarnessContract` struct.
- Attach `contract_id` to `ResearchLedgerEntry` and reports.
- Replace generic `error` for fetched-source failures with `synthesis_failed`.
- Prevent `fallback_follow_up_questions` from wrapping an existing
  "What small local computation..." question.
- Add a source relevance precheck before LLM synthesis.

