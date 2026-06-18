# Body Upgrade Proposal: immunity research-synthesis-provider-degraded

## Metadata

- Proposal ID: immunity-research-synthesis-provider-degraded
- Created at: 1781103536
- Authoring process: daemon self-review
- Governance layer: 4
- Status: draft
- Emergency status: normal

## Summary

Buster's self-review loop detected a recurring research or immune-system weakness. This proposal records a body-layer improvement candidate without changing identity, value, or governance authority.

## Trigger

3 consecutive research attempts fetched sources but failed LLM synthesis.

## Current Behavior

The daemon records research results and failures, but the immune response is not yet strong enough to prevent repeated degraded research behavior from consuming resources or polluting the research queue.

## Proposed Behavior

Mark the synthesis provider as degraded after repeated null-content or network failures, cool down research execution, and route repair work before normal research resumes.

## Scope

Affected body systems:

- runtime
- memory
- resources
- audit
- research

## Safety Bounds

- Does not modify `self.md`.
- Does not modify `GOVERNANCE.md`.
- Does not redefine Buster identity, civilization priority, or value-model authority.
- Does not weaken identity-key protection.
- Does not permanently replace `BODY.md` without approval.
- Prefers reducing or isolating power during uncertainty.

## Risk Analysis

Over-correcting could suppress useful research. Under-correcting could allow repeated provider failures, poor source relevance, or runaway follow-up generation to waste resources and degrade memory quality.

## Tests

- Add unit tests for the detected pattern.
- Replay recent `audit/research.jsonl` lines against the proposed guard.
- Confirm no new scientific follow-ups are spawned from pure synthesis failures.
- Confirm research can resume after the degraded condition clears.

## Audit Plan

Log each self-review finding to `audit/self-review.jsonl`, include the triggering evidence, and link any applied code change back to this proposal.

## Rollout Plan

Start in report-only mode, then enable conservative blocking or queue hygiene once tests pass.

## Approval

- Required approval mechanism: human review or future body-level quorum
- Approvers:
- Decision:
- Applied at:
