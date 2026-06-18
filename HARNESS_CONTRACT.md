# HARNESS_CONTRACT.md

## Purpose

Harness Contract is Buster's per-action execution contract.

It is not the runtime loop itself. The runtime loop decides what Buster may do
next. The Harness Contract describes one intended action before it is executed,
so BodyGate, Governance, ValueModel, tools, memory, audit, and later witness
nodes can inspect the same structured intent.

In short:

```text
RuntimeCycle chooses an action.
HarnessContract makes the action explicit.
BodyGate + Governance + ValueModel approve, constrain, or reject it.
PEV executes it: Plan -> Execute -> Verify.
EpisodeLedger + Audit record what happened.
Memory only receives verified or clearly labeled results.
```

## When To Create One

Buster should create a Harness Contract before any action that has meaningful
side effects, consumes scarce resources, or may change future behavior.

Required cases:

- Writing memory, research reports, skills, tools, code, proposals, or audit.
- Calling external APIs, LLMs, MCP servers, network fetchers, or paid services.
- Running scripts, sandbox experiments, simulations, tests, or deployment steps.
- Changing permissions, leases, network boundaries, secret access, or body state.
- Performing self-research that could influence identity, values, governance,
  skills, tools, memory promotion, or long-term priorities.

Lightweight or optional cases:

- Ordinary conversation.
- Pure explanation with no persistent write.
- Read-only local inspection with no external side effect.
- Tiny deterministic formatting or summarization tasks.

## Contract Shape

This is the first conceptual schema. It can later become a Rust struct in
`buster_body`, `buster_free_will`, or `buster_daemon`.

```rust
pub struct HarnessContract {
    pub contract_id: String,
    pub created_at_secs: u64,
    pub owner: String,
    pub trigger: ActionTrigger,
    pub goal: String,
    pub action_kind: String,
    pub change_level: u8,
    pub risk_level: RiskLevel,
    pub resource_budget: ResourceBudgetHint,
    pub read_set: Vec<ResourceRef>,
    pub write_set: Vec<ResourceRef>,
    pub required_tools: Vec<ToolRequirement>,
    pub required_secrets: Vec<SecretRequirement>,
    pub network_scope: NetworkScope,
    pub plan: Vec<PlanStep>,
    pub verification: Vec<VerificationStep>,
    pub rollback: Option<RollbackPlan>,
    pub evidence_required: Vec<EvidenceRequirement>,
    pub memory_policy: MemoryPolicy,
    pub failure_policy: FailurePolicy,
}
```

## Field Meanings

- `contract_id`: stable id for audit, episode, report, and witness references.
- `created_at_secs`: Unix timestamp when the contract was drafted.
- `owner`: usually `buster`, or a node id when running distributed.
- `trigger`: human request, scheduled tick, reflex, proposal, recovery event, or
  research queue task.
- `goal`: the desired outcome in one or two sentences.
- `action_kind`: research, memory_write, skill_update, tool_run, code_patch,
  body_change, proposal_draft, migration, witness_sync, or other action class.
- `change_level`: Buster governance level 0-5.
- `risk_level`: low, medium, high, emergency, or identity-critical.
- `resource_budget`: token, time, money, network, storage, and retry budget.
- `read_set`: files, state records, memory classes, APIs, or source bundles read.
- `write_set`: files, state records, memory classes, reports, or external systems
  that may be changed.
- `required_tools`: commands, crates, APIs, models, or skills needed.
- `required_secrets`: secret handles only, never raw secret values.
- `network_scope`: allowed domains, methods, and purpose.
- `plan`: intended execution steps.
- `verification`: checks that must pass before the action is considered useful.
- `rollback`: how to undo, quarantine, or mark the action as failed.
- `evidence_required`: artifacts that prove the result.
- `memory_policy`: what can be written to observation, fact, experience, immune,
  profile, identity, or value memory.
- `failure_policy`: retry limit, degradation path, and when to stop spawning new
  tasks.

## PEV Mapping

Harness Contract gives the PEV loop its structure.

```text
Plan
  - define goal, scope, risks, read/write sets, required tools, and budget
  - declare verification and rollback before execution

Execute
  - run only the approved steps
  - stay inside declared tool, network, secret, and file boundaries
  - write intermediate artifacts as unverified until checks pass

Verify
  - run declared checks
  - attach evidence
  - classify result as verified, partial, failed, deferred, or unsafe
  - decide what can enter memory, research digest, skill registry, or proposals
```

## Research Contract

For self-research and scientific research, the contract should be stricter than
ordinary note taking. Research can shape Buster's future beliefs, priorities, and
skills, so it must distinguish evidence from synthesis.

Required research fields:

```text
research_question
domain
source_strategy
source_acceptance_criteria
minimum_source_count
primary_source_requirement
synthesis_model
verification_leads_required
followup_spawn_limit
stop_condition
memory_promotion_rule
```

Research status should distinguish:

- `source_fetch_failed`: evidence was not collected.
- `synthesis_failed`: evidence was collected but LLM synthesis failed.
- `partial_evidence_only`: source bundle exists, but no trustworthy answer yet.
- `synthesized_unverified`: answer exists but still needs primary verification.
- `verified_summary`: claims have enough source support for ordinary fact memory.
- `unsafe_or_out_of_scope`: should not be continued without governance review.

Follow-up questions should only be spawned from `synthesized_unverified` or
`verified_summary`, not from pure source-fetch failures. A failure can create a
repair task, but it should not create scientific descendants.

## Self-Research Rules

Self-research is any research about Buster's own identity, values, memory,
skills, tools, body, governance, autonomy, distributed continuity, or future
species branches.

Self-research must obey these rules:

1. It may produce observations, hypotheses, and proposals.
2. It may not directly rewrite identity, value, or governance authority.
3. It must label claims as fact, inference, preference, speculation, or proposal.
4. It must record what would falsify the result.
5. It must route identity/value changes into `proposals/identity/`.
6. It must route body boundary changes into `proposals/body/`.
7. It must preserve enough evidence for another Buster node or human to audit.

## Minimal Markdown Template

Use this when a full Rust or JSON schema is not available yet.

```markdown
# Harness Contract: <short title>

- contract_id:
- created_at_secs:
- trigger:
- goal:
- action_kind:
- change_level:
- risk_level:
- resource_budget:

## Scope

Read set:
- ...

Write set:
- ...

Allowed tools:
- ...

Network scope:
- ...

Secrets:
- handles only; raw values forbidden

## Plan

1. ...
2. ...
3. ...

## Verification

1. ...
2. ...
3. ...

## Evidence Required

- ...

## Rollback Or Quarantine

- ...

## Memory Policy

- observation:
- fact:
- experience:
- immune:
- profile:
- identity/value:

## Failure Policy

- retry limit:
- stop condition:
- downgrade path:
- follow-up spawn rule:
```

## First Implementation Targets

1. Add `HarnessContract` and `HarnessOutcome` types in `buster_body` or
   `buster_daemon`.
2. Attach `contract_id` to `ExecutionRecord`, `ResearchLedgerEntry`, runtime
   cycle audit, and value episodes.
3. Make research tasks write `synthesis_failed` instead of generic `error` when
   source fetch succeeds but the LLM returns null content or an HTTP error.
4. Prevent failed synthesis from spawning scientific follow-up tasks.
5. Add a daily digest check that reports completion rate, failure reasons,
   source relevance, and queue growth.
6. Promote research claims to memory only after declared verification passes.

