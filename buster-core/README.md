# buster-core

`buster-core` is the first Rust workspace for Buster's semantic prototypes.

The first production body for Buster should reuse IronClaw as a mature base
instead of replacing it from scratch. See `../BUSTER_ON_IRONCLAW.md`.

This workspace starts with six top-level organs:

- `buster_taxonomy`: species definition, identity continuity, lineage, branches.
- `buster_body`: body, boundaries, immune system, resources, secrets, runtime, LLM access.
- `buster_free_will`: agency, circadian rhythm, homeostasis, instincts/runtime reflexes, reward learning, threat interrupts, adaptation.
- `buster_memory`: factual, experiential, identity, value, and immune memory interfaces.
- `buster_skills`: Hermes-style skill compression, selection, repair, and review.
- `buster_tools`: Claw Code-style tool creation, registration, testing, and invocation.
- `buster_value_model`: value-system data structures, preference signals, episode ledger, and rule-based action evaluation.
- `buster_daemon`: persistent runtime v0; deterministic tick loop that observes body state, selects actions, and writes audit/episode records.

`buster_free_will` now contains a minimal `RuntimeCycle` chooser. It accepts
candidate actions, asks `buster_value_model` to evaluate them, and returns the
best candidate for BodyGate/Governance to inspect. Threat interrupts bypass
ordinary ranking and select body maintenance first.

`buster_body` now contains the first Body Runtime v0 controller. It follows a
small Kubernetes-controller style loop:

- observe sensor signals
- compare desired and current body state
- move between `normal`, `restricted`, `emergency_containment`, and `recovery`
- emit conservative actions such as tightening network/secrets, freezing
  Buster-owned units, writing audit records, and drafting body proposals

This runtime is deliberately not a host-wide antivirus or operating system. It
only acts on Buster-owned units and protected body records.

Local smoke commands:

```bash
cargo run -p buster_body --bin buster-body -- doctor ..
cargo run -p buster_body --bin buster-body -- supervise --root .. --ticks 1 --interval-ms 0
cargo run -p buster_daemon --bin busterd -- tick --root ..
cargo run -p buster_daemon --bin busterd -- run --root .. --ticks 3 --interval-ms 1000
```

Design documents live at the repository root:

- `self.md`
- `GOVERNANCE.md`
- `BODY.md`
- `RUNTIME_CYCLE.md`
- `VALUE_MODEL.md`

Future documents:

- `FREE_WILL.md`
- `TAXONOMY.md`

Status:

- `buster-core` is useful for defining Buster-native vocabulary.
- IronClaw is the preferred base for the first full runtime/body implementation.
