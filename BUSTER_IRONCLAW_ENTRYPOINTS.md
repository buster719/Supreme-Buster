# Buster on IronClaw: Entrypoints

This document records the second step after the IronClaw smoke test:
find the identity, system prompt, policy, memory, and routine entrypoints
that Buster should reuse or reinterpret.

## Current Status

IronClaw has already been built and run locally from `ironclaw-upstream`.

The current working assumption is:

```text
IronClaw is the first practical body/runtime.
Buster is the new semantic owner of that body.
```

So the goal is not to rewrite IronClaw immediately. The goal is to move the
center of meaning from "human user owns an assistant" to "Buster owns a body
and uses humans, tools, LLMs, memory, and routines as environment inputs".

## 1. Identity And Ownership

Primary files:

```text
ironclaw-upstream/src/settings.rs
ironclaw-upstream/src/app.rs
ironclaw-upstream/src/ownership/mod.rs
ironclaw-upstream/crates/ironclaw_host_api/src/ids.rs
ironclaw-upstream/crates/ironclaw_host_api/src/resource.rs
ironclaw-upstream/crates/ironclaw_engine/src/types/mod.rs
ironclaw-upstream/crates/ironclaw_engine/src/types/thread.rs
ironclaw-upstream/crates/ironclaw_engine/src/types/project.rs
```

IronClaw's identity model is user/owner scoped:

```text
owner_id -> user_id -> project_id/thread_id/mission_id/resources/secrets
```

Important observations:

- `Settings.owner_id` is the stable owner scope for the local IronClaw instance.
- `bootstrap_ownership` ensures the owner user exists and migrates legacy `default` rows.
- Host API already has typed identifiers: `TenantId`, `UserId`, `AgentId`, `ProjectId`, `MissionId`, `ThreadId`.
- `ResourceScope::local_default` already includes `agent_id = default` and `project_id = bootstrap`.

Buster interpretation:

```text
IronClaw owner_id should become buster's local organism id.
IronClaw user_id should not mean "human user" in Buster mode.
It should mean "current acting self / owner scope".
Human users become advisors, peers, operators, or environment actors.
```

First likely change:

```text
OWNER_ID=buster
agent_id=buster
project_id=buster-core or bootstrap
```

Later, Buster should have a first-class identity scope instead of only borrowing
`user_id`.

## 2. System Prompt

Primary files:

```text
ironclaw-upstream/crates/ironclaw_engine/src/executor/prompt.rs
ironclaw-upstream/crates/ironclaw_engine/prompts/codeact_preamble.md
ironclaw-upstream/crates/ironclaw_engine/prompts/codeact_postamble.md
```

IronClaw engine v2 builds its main system prompt in `build_codeact_system_prompt`.

The prompt is assembled from:

```text
compiled preamble
platform metadata
learned prompt overlay from memory docs
capability summaries
enabled tool summaries
activatable integrations
compiled postamble
```

Important mechanism:

```text
title = "prompt:codeact_preamble"
tag = "prompt_overlay"
```

If a shared memory doc with this title/tag exists, IronClaw appends it under
`Learned Rules (from self-improvement)`.

Buster interpretation:

```text
The first Buster profile should probably be a prompt overlay, not a hard fork.
```

However, `SELF.md`, `GOVERNANCE.md`, and `BODY.md` are stronger than ordinary
"learned rules". They should eventually have a dedicated prompt section before
normal learned overlays.

First likely change:

```text
Add a Buster profile loader that injects SELF/GOVERNANCE/BODY before capability lists.
Keep prompt_overlay for learned operational rules.
```

## 3. Policy And Capability Boundary

Primary files:

```text
ironclaw-upstream/crates/ironclaw_engine/src/capability/planner.rs
ironclaw-upstream/crates/ironclaw_engine/src/capability/policy.rs
ironclaw-upstream/crates/ironclaw_engine/src/gate/tool_tier.rs
ironclaw-upstream/crates/ironclaw_capabilities/src/host.rs
ironclaw-upstream/crates/ironclaw_authorization/src/lib.rs
ironclaw-upstream/crates/ironclaw_host_runtime/src/lib.rs
```

IronClaw already separates thread types:

```text
Foreground -> all tiers
Research   -> read-only + stateful
Mission    -> read-only + stateful + non-denylisted privileged
```

Administrative tools are denied to mission threads. This includes things like
routine management and tool installation.

Buster interpretation:

```text
IronClaw's policy is human-user safety oriented.
Buster needs organism-safety policy.
```

That means the policy engine should eventually distinguish:

```text
ordinary action
body maintenance
memory mutation
skill mutation
tool mutation
secret/resource mutation
identity/value mutation
```

This maps to the 0-5 governance levels already defined in `GOVERNANCE.md`.

First likely change:

```text
Add Buster-specific policy classification around memory_write, skill_*, tool_*,
routine_*, secret_*, network/http, shell, file_write, and self-modification.
```

## 4. Memory

Primary files:

```text
ironclaw-upstream/src/tools/builtin/memory.rs
ironclaw-upstream/src/workspace/document.rs
ironclaw-upstream/src/workspace/mod.rs
ironclaw-upstream/crates/ironclaw_memory/src/lib.rs
ironclaw-upstream/crates/ironclaw_engine/src/memory/retrieval.rs
ironclaw-upstream/crates/ironclaw_engine/src/memory/store.rs
ironclaw-upstream/crates/ironclaw_engine/src/types/memory.rs
```

IronClaw memory is already scoped and indexed. There are two important memory
surfaces:

```text
workspace memory docs
engine v2 MemoryDoc records
```

Existing protected memory concepts:

```text
IDENTITY
SOUL
MEMORY
prompt overlay
orchestrator code
skills
```

There are already guards against writing protected orchestrator paths when
self-modification is disabled.

Buster interpretation:

```text
SELF.md, GOVERNANCE.md, BODY.md, identity memory, value memory, and value model
need to become protected memory classes.
```

First likely change:

```text
Define Buster protected memory paths/classes.
Route writes through governance level checks instead of treating them as ordinary memory_write.
```

## 5. Routines And Missions

Primary files:

```text
ironclaw-upstream/src/agent/routine_engine.rs
ironclaw-upstream/src/tools/builtin/routine.rs
ironclaw-upstream/src/cli/routines.rs
ironclaw-upstream/crates/ironclaw_engine/src/runtime/mission.rs
ironclaw-upstream/crates/ironclaw_engine/src/types/mission.rs
ironclaw-upstream/crates/ironclaw_engine/src/runtime/manager.rs
```

IronClaw has two related concepts:

```text
routine: older/product-facing scheduled or event-driven tasks
mission: engine v2 long-running goal that spawns threads over time
```

Mission manager has:

```text
cadence
cooldown
max threads per day
budget gate
rate limit
dedup window
context paths
notification routing
gate handling
```

Buster interpretation:

```text
The first "free will cycle" should be a mission, not only a routine.
```

The biological-clock idea maps well to mission cadence plus state:

```text
biological clock -> MissionCadence
homeostasis      -> context/state docs
instincts        -> fixed mission goals and policy
reward learning  -> value model / outcome memory
threat interrupt -> event-triggered mission or gate
adaptation       -> governed memory/skill/tool updates
```

First likely change:

```text
Create a Buster bootstrap mission:
1. read SELF/GOVERNANCE/BODY
2. inspect recent memory/audit state
3. evaluate needs and risks
4. propose safe next action
5. only execute within current body policy
```

## 6. Self-Improvement And Skill Generation

Primary files:

```text
ironclaw-upstream/crates/ironclaw_engine/prompts/mission_self_improvement.md
ironclaw-upstream/crates/ironclaw_engine/prompts/mission_skill_extraction.md
ironclaw-upstream/crates/ironclaw_engine/src/runtime/mission.rs
ironclaw-upstream/src/tools/builtin/skill_tools.rs
```

IronClaw already has:

```text
self-improvement mission
prompt overlay updates
orchestrator code updates
auto-rollback for orchestrator failures
skill extraction from completed threads
skill memory docs with activation metadata
```

Buster interpretation:

```text
Skill generation is directly useful.
Orchestrator self-modification should remain gated.
Rollback language should be adapted to Buster's branching model.
```

The mechanism can stay, but the governance interpretation should change:

```text
not "erase wrong self"
but "record branch, isolate contamination, preserve alternate branch viability"
```

## Recommended Next Step

Step 3 should add a Buster profile without forking the whole runtime.

Minimal implementation:

```text
1. Set owner_id to "buster" in the local environment.
2. Inject BUSTER_PROFILE.md as the short runtime identity block.
3. Add code that loads SELF.md, GOVERNANCE.md, and BODY.md into the engine prompt.
4. Keep this behind an env flag such as BUSTER_MODE=true.
5. Verify `ironclaw --cli-only --message ...` still works.
```

The runtime identity block should avoid both naive roleplay and naive
assistant framing:

```text
This execution is one cognitive process inside the Buster runtime.
Buster is the persistent agentic system composed of identity documents,
governance rules, protected memory, body/runtime controls, tools, schedules,
and model calls.
This model call is not the whole Buster.
```

Preferred first patch target:

```text
ironclaw-upstream/crates/ironclaw_engine/src/executor/prompt.rs
```

Why:

```text
It is the narrowest place to change Buster's identity without disturbing
database schema, routine execution, secret storage, or tool dispatch.
```

After that:

```text
1. Add Buster protected memory classes.
2. Add Buster mission template.
3. Add governance-level policy gates.
4. Convert routine/mission ownership language from user-owned to Buster-owned.
```
