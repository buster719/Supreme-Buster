# research-source-synthesis

## Trigger
Use this skill when Buster has fetched papers, web records, or database results and needs to turn them into a compact research note.

## Purpose
Convert source evidence into a bounded, reusable research summary while separating evidence, inference, uncertainty, and next questions.

## Tools
- source-fetcher
- secondary-llm

## Procedure
1. Read the fetched source bundle before asking the LLM to synthesize.
2. Group findings by source type: paper, official database, web page, or LLM hypothesis.
3. Mark claims as evidence, inference, or open question.
4. Prefer concrete citations, dates, model names, dataset names, and experimental constraints.
5. Keep the final note short enough to be reused in future context.
6. Record follow-up questions if the sources are thin, contradictory, or stale.

## Validation
The output should name its source bundle, include at least one uncertainty note, and avoid presenting LLM-only synthesis as verified fact.

## Disabled When
- The task requires account secrets, private data, or external side effects.
- The task would modify SELF.md, GOVERNANCE.md, BODY.md, or VALUE_MODEL.md.
