# Buster Security Review

This document tracks Buster's current attack surface from a defensive red-team perspective.

## Security Posture

Buster is still a v0 cyber-organism substrate. The strongest protection is not any single rule; it is the combination of body-layer control, auditability, resource limits, source quarantine, and conservative escalation between layers.

Current priority:

1. Protect untrusted information before it enters cognition.
2. Force side-effect paths through body-layer controls.
3. Prevent resource drain and repeated self-harm loops.
4. Harden public and local interfaces.
5. Keep identity/value/body authority files out of ordinary mutation paths.

## Attack Surface Register

| ID | Attack Surface | Current Risk | Status | Next Control |
| --- | --- | --- | --- | --- |
| S1 | Research prompt injection | Malicious papers/pages can hide instructions in titles, abstracts, or metadata. | Partial control added. Source evidence is wrapped as untrusted data and suspicious markers are recorded in source bundles. | Add stronger classifier, source trust scoring, and hard quarantine for hostile sources. |
| S2 | Research token/resource drain | Repeated follow-up tasks can consume API, time, and storage. | Partial control added. Research has hourly cap and consecutive LLM-error brake. | Add per-model budget ledger, per-domain quota, and duplicate question collapse. |
| S3 | Skill supply chain | Downloaded skills can smuggle unsafe procedures or tool calls. | Partial control exists. Skill registry infers risk level and generated skills start as drafts. | Add signed skill manifests, hash pinning, source reputation, and sandboxed skill dry-run. |
| S4 | BodyGate bypass | A direct LLM/tool/network path can bypass layer-4 policy. | Incomplete. Some paths still call clients directly. | Require BodyGate for all LLM, tool, network, memory, and skill mutations. |
| S5 | Secret and `.env` exposure | API keys may remain in readable files or logs. | Partial control exists. Buster can read encrypted local secret store, but `.env` fallback remains. | Add `secret set` CLI, migrate MiMo key into secret store, stop reading legacy `.env` by default. |
| S6 | Public VPS demo surface | Public HTTP endpoint can leak state or be flooded. | Incomplete. Demo is read-only but public. | Add token-gated admin endpoints, rate limits, request size limits, and log redaction. |
| S7 | Witness/network poisoning | Fake nodes, replayed snapshots, or stale receipts can distort existence claims. | Partial control exists. Node keys and receipts are verified. | Add replay windows, quorum thresholds, node reputation, and chain-head freshness checks. |
| S8 | Local web console attack | Localhost services can be hit by malicious browser pages or extensions. | Partial control added. POST requires `X-Buster-Console: 1`, wildcard CORS removed, body size capped. | Add session token, origin checks, and optional loopback-only confirmation. |
| S9 | Memory/research poisoning | Buster can slowly learn false facts from polluted sources. | Incomplete. Research records uncertainty but memory promotion is not fully guarded. | Add source provenance weights, contradiction tracking, and promotion gates before long-term memory. |
| S10 | Duplicate daemon/resource runaway | Multiple runtime loops can duplicate work and spend tokens. | Partial operational fix. Duplicate VPS process was detected and stopped manually. | Add daemon lockfile, heartbeat ownership, and stale-lock recovery. |

## Changes Added In This Pass

- Source evidence is now explicitly wrapped as untrusted data before LLM synthesis.
- Source bundles record prompt-injection and destructive-action markers.
- Research synthesis has retry support for transient LLM failures.
- Research execution has a per-hour attempt cap.
- Research execution has a consecutive LLM-error brake.
- Local web console POST requests require `X-Buster-Console: 1`.
- Local web console no longer emits `Access-Control-Allow-Origin: *`.
- Local web console caps request body size.

## Remaining High-Priority Work

1. Wire LLM calls through BodyGate rather than direct client calls.
2. Add durable daemon lockfile to prevent duplicate Buster loops.
3. Move MiMo/OpenRouter keys into Buster secret store and remove `.env` as default.
4. Add public demo/relay rate limiting and token-gated write endpoints.
5. Add source trust scores and memory-promotion gates.
