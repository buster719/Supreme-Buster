# Paper Brief: Extractive Fallback

Question: fault tolerant quantum computing logical qubit milestones

Selected papers:
- An introduction to Fault-tolerant Quantum Computing | sha256=66be1c2804c2c82f96ee1d3f6628574ddd7fc4af096fde3a7e4deb83ec9d75ef

LLM synthesis status: failed, so this brief is an extractive fallback rather than a full interpretation.

Failure: LLM HTTP error: egress broker error: HTTP error: error sending request for url (https://api.xiaomimimo.com/v1/chat/completions): operation timed out

## Claim table

| Claim | Support | Uncertainty | Falsifier |
| --- | --- | --- | --- |
| The selected paper contains relevant material for the question. | Keyword-matched snippets below. | This is not yet a semantic synthesis. | A later LLM or PaperQA pass finds these snippets irrelevant or contradicted. |
| Buster has preserved a retryable research trail. | Paper title/hash and extracted text path are recorded in audit. | No cross-paper comparison yet. | Manifest/hash mismatch or missing extracted text. |

## Extracted snippets

1. [page 1]
arXiv:1508.03695v1  [quant-ph]  15 Aug 2015
An introduction to Fault-tolerant Quantum Computing
∗
Alexandru Paler
University of Passau
Innstr. 43
Passau, Germany, 94032
alexandru.paler@uni-passau.de
Simon J. Devitt
Ochanomizu University
2-1-1, Otsuka, Bunkyo-ku
Tokyo, 112-8610, Japan
devitt1@mac.com
ABSTRACT
In this paper we provide a basic introduction of the core
ideas and theories surrounding fault-tolerant quantum com -
putation. These concepts underly the theoretical frame-
work of large-scale quantum computation and communica-
tions and are the driving force for many recent experimental
eﬀorts to construct small to medium sized arrays of con-
trollable quantum bits. We examine the basic principals
of redundant quantum encoding, required to protect quan-
tum bits from errors generated from both imprecise con-
trol and environmental interactions and then examine the
principa
[truncated]

2. [page 5]
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

3. === Paper: An introduction to Fault-tolerant Quantum Computing ===
source_id: Arxiv-3e285785adc9414f
sha256: 66be1c2804c2c82f96ee1d3f6628574ddd7fc4af096fde3a7e4deb83ec9d75ef
pdf_url: https://arxiv.org/pdf/1508.03695v1.pdf

4. [page 2]
Quantum measurement is deﬁned with respect to a basis
and yields one of the basis vectors with a probability re-
lated to the amplitudes of the quantum state. Common
measurements are known as Z- and X-measurements. Z-
measurement is deﬁned with respect to basis ( |0⟩, |1⟩). Ap-
plying a Z-measurement to a qubit in state |ψ⟩ = α0 |0⟩ +
α1 |1⟩ yields |0⟩ with probability |α0|2 and |1⟩ with prob-
ability |α1|2. Moreover, the state |ψ⟩ collapses into the
measured state (i.e. only the components of |ψ⟩ consistent
with the measurement result remains). X-measurement is
deﬁned with respect to the basis ( |+⟩, |−⟩), where |±⟩ =
1
√
2 (|0⟩ ± |1⟩).
A state may be modiﬁed by applying single-qubit quantum
gates. Each quantum gate corresponds to a complex unitary
matrix, and gate function is given by multiplying that ma-
trix with the quantum state. The application of X gate to a
state result
[truncated]

5. [page 3]
an ancilla qubit that is initialised, interacted with a pair of
qubits in the code block and measured. The result of the
measurement on the ancilla (either |0⟩ or |1⟩) will determine
the parity of the two qubits (odd or even), and also force
these two qubits to be in an even or odd parity state if they
were not beforehand.
The principal of a codespace within quantum computation
is to construct encoded codewords that are always in cer-
tain, well deﬁned parity states regardless of the state of
the encoded information. Physical errors will then perturb
encoded information away from these well deﬁned parities
which can be detected without determining any information
regarding the encoding.
Returning back to the example of a redundancy code, the
two encoded states |0⟩L and |1⟩L are constructed to be even
parity states of any pairwise Z operators. i.e. applying
the operator ZiZj for 
[truncated]

6. [page 6]
fault corrector has two options: to either correct the fault s,
or to try and track their eﬀect throught the distributed al-
gorithm. The direct correction could introduce failures, a nd
for this reason fault-tracking is more advantageous. Fault -
tracking is performed based on commutativity properties:
it is known how faults are transformed by both local and
global operations. Hence, corrections are required only af ter
the distributed computation was terminated and the output
states were read out from the distributed system.
5.7 Transversality
The transversal application of a logical operation (local o r
distributed) is its decomposition into (local or distribut ed)
operations applied on the component processes. For exam-
ple, the logical local operation Gl is the n-fold application
of G on each of the n components.
Faults are propagated by inter-process communication. In
this
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
