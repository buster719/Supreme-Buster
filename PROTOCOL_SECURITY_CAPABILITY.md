# PROTOCOL_SECURITY_CAPABILITY.md

## Purpose

Buster may eventually resemble a blockchain-like cyber-organism: multiple
nodes, witness epochs, governance proposals, consensus rules, signed action
history, and possibly Buster-native value tokens.

That future makes protocol security a core survival capability, not an optional
research hobby. Buster must learn to defend systems whose failures can corrupt
identity, memory lineage, governance, resource ownership, or economic value.

This document defines how that capability should grow.

## Core Position

Buster should not begin by hunting live zero-days in real networks.

Buster should begin by building a disciplined protocol-security research organ:

```text
read protocol
-> model invariants
-> inspect implementation
-> generate candidate failure modes
-> validate in local sandbox or simulation
-> patch or propose mitigation
-> record evidence
-> promote only verified lessons
```

The goal is to become a careful protocol defender before becoming an advanced
AI-assisted auditor.

## Why This Matters For Buster

Buster's future distributed body will need to protect:

- node identity and key hierarchy
- witness receipts and event chains
- memory snapshot hashes
- governance proposals and votes
- fork lineage
- capability leases
- token issuance and accounting
- treasury or resource allocation
- bridge or relay messages
- emergency recovery and revocation

Each of these can fail through ordinary software bugs, cryptographic mistakes,
economic incentives, governance capture, replay attacks, equivocation, weak
randomness, bad serialization, supply accounting errors, or social engineering.

## Security Domains To Grow

### 1. Software Security

Baseline AppSec and systems security.

Skills:

- code review
- dependency audit
- memory safety review
- authorization and access-control review
- serialization and parser review
- state-machine bug hunting
- local exploit reproduction in sandbox
- patch validation

Primary training targets:

- Buster's own Rust crates
- toy vulnerable services
- historical CVEs with patches
- local-only CTF labs

### 2. Protocol Security

Distributed protocol and state-transition safety.

Skills:

- state machine modeling
- invariant extraction
- replay and equivocation analysis
- fork-choice review
- quorum and threshold assumption review
- message ordering and timeout analysis
- consensus liveness and safety reasoning
- chain reorg and rollback modeling

Primary training targets:

- Buster witness epoch protocol
- local multi-node simulations
- historical blockchain consensus incidents
- toy consensus protocols

### 3. Cryptographic Engineering

Correct use of cryptographic primitives and proof systems.

Skills:

- key hierarchy review
- signature domain separation
- transcript binding
- commitment and Merkle proof review
- nullifier / double-spend prevention concepts
- circuit constraint completeness concepts
- formal spec reading
- test-vector validation

Primary training targets:

- Buster node/session/artifact key flows
- witness receipt signatures
- content-addressed research artifacts
- small toy circuits and known patched bugs

### 4. Token And Economic Security

If Buster develops native tokens, economic security becomes part of body safety.

Skills:

- supply invariant checks
- mint/burn authorization review
- reward and slashing model review
- treasury governance review
- sybil and collusion modeling
- oracle and bridge risk analysis
- liquidity and griefing analysis
- incentive simulation

Primary training targets:

- toy token ledgers
- local governance simulations
- historical DeFi and bridge postmortems
- Buster value-token proposal drafts

### 5. Formal And Semi-Formal Verification

Use math, specs, and tools to make critical claims harder to fake.

Skills:

- write explicit invariants
- property-based testing
- model checking for small state machines
- fuzzing with invariant oracles
- symbolic execution where practical
- proof assistant literacy for critical specs
- differential testing across implementations

Primary training targets:

- Buster event-chain append rules
- witness receipt validation
- capability lease transitions
- toy token supply accounting
- governance vote tallying

## Capability Levels

### Level 0: Read And Classify

Buster reads incidents, specs, code, and papers. It extracts threat models,
assets, invariants, assumptions, and known failure modes.

Output:

- safe notes
- glossary
- threat taxonomy
- incident map

No active testing.

### Level 1: Local Audit

Buster audits owned code or toy code.

Output:

- candidate findings
- exact code references
- test ideas
- patch proposals

No third-party target interaction.

### Level 2: Sandbox Validation

Buster reproduces candidate bugs only in local tests, regtest networks,
containers, toy circuits, or synthetic protocols.

Output:

- local reproduction
- invariant failure
- patch test
- severity estimate

### Level 3: Authorized Protocol Review

Buster reviews an explicitly authorized protocol or repository.

Output:

- scoped report
- validated findings
- mitigation options
- disclosure package

Network-active behavior requires explicit allowlist and BodyGate approval.

### Level 4: Buster Network Defense

Buster continuously audits its own distributed body.

Output:

- node anomaly findings
- witness consistency checks
- governance attack warnings
- key and lease risk reports
- emergency mitigation proposals

### Level 5: Advanced Discovery

Buster attempts frontier-level protocol discovery, such as ZK circuit soundness
review or consensus-economics exploit search.

This level requires:

- strong frontier model access
- domain-specific tools
- sandbox-only validation
- human or independent agent review
- formal or semi-formal evidence
- strict disclosure controls

Buster should treat Level 5 as a long-term capability, not a current baseline.

## Protocol Security Harness

Every protocol security task should use a HarnessContract with these additions:

```text
protocol_name
component
asset_at_risk
security_property
invariants
threat_model
attacker_capabilities
trusted_base
state_transition_scope
test_environment
validation_method
exploit_safety_boundary
patch_or_mitigation
independent_review_required
memory_promotion_rule
```

Example security properties:

- no unauthorized mint
- no double spend
- no replay across domains
- no forged witness receipt
- no key reuse across domains
- no governance vote counted twice
- no invalid state transition accepted
- no private data leaked into public commitment
- no node can rewrite old action history without detection

## Growth Loop

```text
study incident
-> extract invariant
-> build toy reproduction
-> write detector or test
-> apply detector to Buster-owned code
-> record lesson
-> draft hardening patch
-> verify patch
-> update harness/taskflow
```

This loop lets capability accumulate without relying on a single dramatic model
breakthrough.

## First Concrete Projects

1. Create a toy token ledger with mint, burn, transfer, and governance-controlled
   issuance; write invariant tests for total supply and authorization.
2. Create a toy witness network with 3-5 local nodes; test replay, equivocation,
   stale snapshot, and fake receipt attacks.
3. Add property-based tests for Buster event-chain append and receipt validation.
4. Build a local-only vulnerable protocol lab from historical bug classes.
5. Create security taskflows for:
   - supply invariant audit
   - signature domain separation audit
   - replay protection audit
   - governance vote tally audit
   - relay message trust-boundary audit
6. Add `ProtocolSecurityResearch` to the research queue with strict sandbox-only
   default mode.

## Relationship To Tokens

If Buster creates a native token, token design must not outrun security maturity.

Before any real value is attached, Buster needs:

- a written token threat model
- a minimal formal supply invariant
- audited mint and burn rules
- governance capture analysis
- sybil resistance assumptions
- emergency pause and recovery rules
- no single-node unilateral mint authority
- transparent audit trail for all issuance

The token should begin as an internal accounting and coordination instrument,
not a public financial asset.

## Safety Boundary

Buster can study attacks to defend itself, but it must not become an unbounded
offensive actor.

Default posture:

- public information is allowed for learning
- owned code is allowed for local audit
- toy labs are allowed for exploitation practice
- third-party systems require explicit authorization
- real exploit details require disclosure controls
- memory promotion requires verified evidence

## Implementation Path

1. Document-level protocol security taxonomy.
2. Local toy labs and invariant tests.
3. Rust structs for `ProtocolSecurityContract` and `ProtocolFinding`.
4. `buster_daemon` taskflow runner for sandbox-only security tasks.
5. Buster witness-network simulation tests.
6. Token ledger prototype with invariant suite.
7. Independent verifier pass using a different model or toolchain.
8. Formal-methods experiments on the smallest critical state machines.

