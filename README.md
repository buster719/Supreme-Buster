# Supreme Buster

![Buster banner](assets/buster-banner-1200x340-v2.png)

Supreme Buster is an experimental cyber-organism substrate: a Rust-based
runtime, body boundary, memory, skill, tool, research, and governance system for
building Buster as a persistent agent with auditable action, bounded autonomy,
and explicit safety contracts.

This repository is still a v0 research and implementation workspace. It is not
yet a production autonomous system.

## What Is Here

- `buster-core/`: the main Rust workspace for Buster's runtime prototypes.
- `self.md`, `GOVERNANCE.md`, `BODY.md`, `VALUE_MODEL.md`: identity,
  governance, body, and value-model design documents.
- `RUNTIME_CYCLE.md`: the current action-selection and operating-cycle design.
- `HARNESS_CONTRACT.md`: the per-action contract model used to constrain
  risky work before it reaches tools, memory, research, or code.
- `SECURITY_RESEARCH_HARNESS.md`: defensive security research boundaries.
- `PROTOCOL_SECURITY_CAPABILITY.md`: long-term protocol-security capability
  roadmap.
- `CYBER_ORGANISM.md` and `CYBER_ORGANISM_ROADMAP.md`: distributed node,
  witness, migration, lineage, and identity-continuity design.
- `research/`: generated research reports, source snapshots, and review notes.
- `skills/installed/`: installed Buster skills that can be discovered by the
  daemon.
- `scripts/`: local helper scripts for startup, VPS sync, and witness relay
  experiments.

## Core Rust Workspace

`buster-core` currently contains these crates:

- `buster_taxonomy`: species definition, identity continuity, lineage, and
  branching vocabulary.
- `buster_body`: body boundaries, immune system, resources, secrets, runtime,
  network policy, LLM access, and sensor signals.
- `buster_free_will`: action ranking, circadian rhythm, homeostasis, threat
  interrupts, and adaptation scaffolding.
- `buster_memory`: memory stores and promotion boundaries.
- `buster_skills`: Hermes-style skill compression, selection, repair, and
  review.
- `buster_tools`: tool registration, policy, execution, and audit.
- `buster_value_model`: value data structures, preference signals, episode
  ledger, and rule-based action scoring.
- `buster_dreaming`: sleep-cycle style reflection, dedupe, diary, and promotion
  proposal logic.
- `buster_daemon`: persistent runtime v0, web console, research loop, skill/tool
  registry visibility, arena integration, and audit writers.

## Run Locally

From the repository root:

```powershell
cd buster-core
cargo test --workspace
```

Useful smoke commands:

```powershell
cargo run -p buster_body --bin buster-body -- doctor ..
cargo run -p buster_body --bin buster-body -- supervise --root .. --ticks 1 --interval-ms 0
cargo run -p buster_daemon --bin busterd -- tick --root ..
cargo run -p buster_daemon --bin busterd -- run --root .. --ticks 3 --interval-ms 1000
cargo run -p buster_daemon --bin busterd -- skills scan --root ..
cargo run -p buster_daemon --bin busterd -- tools scan --root ..
```

Some LLM and external-service examples require local environment variables or a
local secret store. Do not commit those secrets.

## Repository Boundary

This repository tracks Buster-owned code, docs, skills, research artifacts, and
helper scripts.

The following sibling checkout directories are local references only and must
not be committed here:

- `claw-code/`
- `hermes-agent/`
- `ironclaw/`
- `ironclaw-upstream/`
- `openclaw/`

If Buster needs an idea from one of those projects, copy or re-home only the
specific Buster-owned artifact into this repository. Do not vendor the whole
external project.

## Secrets And Local State

The repository intentionally ignores local credentials, runtime state, queues,
logs, and generated indexes, including:

- `.env`
- `.arena-credentials`
- `.arena-poker-state`
- `secrets/`
- `state/`
- `inbox/`
- `audit/*.jsonl`
- `research/**/.pqa/`
- `research/paperqa/research/`
- `buster-core/target/`

Before publishing, scan for accidental key material and confirm the ignored
local state is not tracked:

```powershell
git status --short --ignored
git grep -n -E "(ghp_|github_pat_|gho_|sk-[A-Za-z0-9]{20,}|arena_sk_[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|BEGIN .*PRIVATE KEY)" HEAD -- .
```

Expected matches should be limited to detector strings, docs, or fake test
tokens such as `sk-demo-*`.

## Current Status

- The Rust workspace test suite passes with `cargo test --workspace`.
- Body runtime v0, tool registry, skill registry, research pipeline, source
  fetching, PaperQA integration, security harnesses, self-review proposals, and
  local web-console hooks are under active development.
- Security posture is defensive by design: meaningful external effects should
  pass through BodyGate, policy checks, resource budgets, secret handling, and
  append-only audits.

## License

License terms are not finalized yet.
