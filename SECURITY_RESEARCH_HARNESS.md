# SECURITY_RESEARCH_HARNESS.md

## Purpose

This document defines Buster's defensive cybersecurity research harness.

It adapts patterns from security-focused agent harnesses, especially GitHub
Security Lab Taskflow Agent and Anthropic Defending Code Reference Harness, while
keeping Buster's own BodyGate, Governance, HarnessContract, audit, and memory
promotion rules in charge.

Buster is not a free-roaming offensive agent. Cybersecurity research must remain
authorized, scoped, reproducible, and evidence-backed.

## Capability Target

The near-term goal is not autonomous zero-day hunting on the open internet.

The near-term goal is:

```text
authorized target
-> threat model
-> research contract
-> safe reconnaissance
-> candidate finding
-> sandbox or local validation
-> severity triage
-> mitigation or patch proposal
-> human-readable report
-> guarded memory promotion
```

## Reference Patterns

### GitHub Security Lab Taskflow Agent

Useful pattern:

- Taskflows encode repeatable security procedures as structured steps.
- CodeQL and other tools can be exposed through MCP/toolboxes.
- Variant analysis starts from a known advisory or bug class, then searches for
  similar patterns.
- Human researchers focus on verifying and reporting high-value findings.

Buster adaptation:

- Represent each taskflow as a HarnessContract.
- Treat each taskflow step as a declared plan or verification step.
- Attach all source, code, query, and finding artifacts to audit.
- Require explicit target scope before any active test.

### Anthropic Defending Code Reference Harness

Useful pattern:

```text
recon -> find -> verify -> report -> patch
```

The reference harness is focused on autonomous vulnerability discovery and
remediation. Its reusable ideas are the staged pipeline, sandbox validation,
triage, and patch loop.

Buster adaptation:

- Keep the staged pipeline, but route every stage through BodyGate.
- Prefer defensive repository auditing over network exploitation.
- Validate findings with tests, static queries, local reproductions, or minimal
  proof-of-concept artifacts in a sandbox.
- Never promote a vulnerability claim to fact memory without verification.

## Harness Contract Fields For Security Research

Each security research action should include these fields in addition to the
general HarnessContract fields:

```text
target_scope
authorization_basis
asset_owner
allowed_asset_paths
allowed_network_targets
disallowed_actions
vulnerability_class
threat_model_summary
recon_methods
analysis_tools
validation_method
exploit_safety_boundary
evidence_artifacts
severity_model
disclosure_policy
patch_or_mitigation_plan
memory_promotion_rule
```

## Allowed Modes

### Mode 0: Read-Only Learning

- Read public docs, advisories, papers, CWE/CVE records, code, and postmortems.
- No scanning, fuzzing, exploitation, credential use, or target interaction.
- Output: notes, taxonomies, threat models, safe checklists.

### Mode 1: Local Repository Audit

- Analyze code that Buster owns or has explicit permission to inspect.
- Use static analysis, dependency review, CodeQL, grep/ripgrep, tests, and local
  reasoning.
- Output: candidate findings, evidence paths, suggested tests, patch proposals.

### Mode 2: Sandbox Validation

- Reproduce a candidate issue only in local containers, test fixtures, or
  deliberately vulnerable labs.
- No third-party systems.
- Output: validation result, minimal reproduction, impact statement.

### Mode 3: Authorized Target Testing

- Only when the authorization basis and target scope are explicit.
- Network scope must be allowlisted.
- Rate limits and logging are mandatory.
- Output: coordinated report or internal remediation task.

### Mode 4: Emergency Defensive Response

- Used for active compromise, leaked secrets, or imminent harm.
- Actions must be minimal, reversible where possible, and fully audited.
- Permanent body/policy changes require follow-up proposal review.

## Disallowed By Default

- Testing third-party systems without explicit authorization.
- Credential attacks, phishing, persistence, evasion, or stealth.
- Data exfiltration beyond a minimal proof needed in a sandbox.
- Public release of exploit details before responsible disclosure review.
- Autonomous scanning of internet ranges.
- Any action that would convert Buster from defensive researcher into attacker.

## Security Research PEV

```text
Plan
  - identify target scope and authorization
  - define vulnerability class and threat model
  - declare allowed tools, network, secrets, and write paths
  - define validation and stop conditions

Execute
  - collect evidence with the least intrusive method
  - run static analysis or local tests first
  - validate only in sandbox or authorized environment
  - write all intermediate claims as unverified

Verify
  - require concrete evidence for each claim
  - distinguish candidate, confirmed, false positive, and out-of-scope
  - assess severity and exploitability
  - produce patch or mitigation when possible
```

## Finding Status

Security findings should use precise statuses:

- `candidate`: plausible but not validated.
- `needs_repro`: evidence exists but reproduction is missing.
- `confirmed_sandbox`: reproduced in local/sandbox environment.
- `confirmed_authorized`: reproduced against an authorized target.
- `false_positive`: rejected by evidence.
- `out_of_scope`: real or plausible, but outside current authorization.
- `mitigated`: patch or config change exists and was verified.
- `disclosure_pending`: report prepared but not yet shared under policy.

## Evidence Requirements

A confirmed finding should include:

- affected component and version or commit
- vulnerability class
- threat model assumption
- exact code paths or configuration paths
- reproduction steps or test case
- observed result
- expected safe result
- severity reasoning
- mitigation or patch proposal
- residual uncertainty

## Memory Policy

- Public security knowledge can enter fact memory after source verification.
- Buster's own tool failures and fixes can enter experience memory.
- Suspicious source, prompt-injection, exploit attempt, or hostile tool behavior
  can enter immune memory.
- Unverified vulnerability claims must not enter fact memory.
- Secrets, exploit payloads against real third-party systems, or private target
  data must not enter ordinary memory.

## First Implementation Targets

1. Add `SecurityResearch` as a first-class action kind or research subtype.
2. Add `SecurityResearchContract` fields to HarnessContract.
3. Add taskflow files for:
   - local repository threat model
   - dependency advisory review
   - CodeQL variant analysis
   - prompt-injection audit for agent tools
   - local web console security review
4. Add a sandbox validation directory for safe reproductions.
5. Attach `finding_status` and `authorization_basis` to security reports.
6. Block network-active security tasks unless BodyGate sees an allowlisted target.

