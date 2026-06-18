# Paper Brief: Extractive Fallback

Question: ping

Selected papers:
- An introduction to Fault-tolerant Quantum Computing | sha256=66be1c2804c2c82f96ee1d3f6628574ddd7fc4af096fde3a7e4deb83ec9d75ef

LLM synthesis status: failed, so this brief is an extractive fallback rather than a full interpretation.

Failure: LLM HTTP error: egress broker error: HTTP error: error sending request for url (https://api.xiaomimimo.com/v1/chat/completions)

## Claim table

| Claim | Support | Uncertainty | Falsifier |
| --- | --- | --- | --- |
| The selected paper contains relevant material for the question. | Keyword-matched snippets below. | This is not yet a semantic synthesis. | A later LLM or PaperQA pass finds these snippets irrelevant or contradicted. |
| Buster has preserved a retryable research trail. | Paper title/hash and extracted text path are recorded in audit. | No cross-paper comparison yet. | Manifest/hash mismatch or missing extracted text. |

## Extracted snippets

1. [page 5]
Majorities (quorums [1]) are the most common option for
checking the introduced redundancies. The simple major-
ity (N/2 + 1) of N objects (processes, bits etc.) is used to
introduce the fault-tolerant quantum computing in the fol-
lowing: a fault-tolerant logical process is constructed fr om
three (or more) component processes (called components),
and the logical process is able to tolerate at most one faulty
component. The computation of quorums is detailed in Sec-
tion 5.8.
Due to the fact that the q objects of each process are non-
copyable increases the diﬃculty of implementing fault-tol erance
through redundancy. Copying the q state of an existing
process to a newly initialised one is not possible. As a re-
sult, separate components are initialised into the same sta te
q ∈S at the start of the distributed algorithm and exactly
the same operations are applied on their objec
[truncated]

## Counterarguments / alternative explanations

- Keyword overlap can select introductory or peripheral passages rather than milestone evidence.
- A single paper may be outdated or insufficient for current fault-tolerant quantum-computing milestones.

## What would change Buster's mind

- A successful PaperQA/LLM pass over a larger corpus.
- Newer arXiv or review papers that identify different logical-qubit milestone criteria.
- Direct source evidence from experimental roadmaps or hardware benchmark papers.

## Verification leads

- Re-run `papers brief` when the configured LLM endpoint is healthy.
- Add 2-3 newer arXiv review or roadmap papers to the corpus.
- Compare extracted claims against recent surface-code and logical-qubit demonstration papers.

## Mini-experiment

- Build a small local table with columns: paper, year, code family, physical qubits, logical qubits, logical error rate, and break-even criterion.
