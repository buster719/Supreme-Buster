# Paper Brief Failed

Question: fault tolerant quantum computing logical qubit milestones

Selected papers:
- An introduction to Fault-tolerant Quantum Computing | sha256=66be1c2804c2c82f96ee1d3f6628574ddd7fc4af096fde3a7e4deb83ec9d75ef

Failure: LLM HTTP error: egress broker error: HTTP error: error sending request for url (https://api.xiaomimimo.com/v1/chat/completions)

Partial source excerpt retained for later retry:

```text


=== Paper: An introduction to Fault-tolerant Quantum Computing ===
source_id: Arxiv-3e285785adc9414f
sha256: 66be1c2804c2c82f96ee1d3f6628574ddd7fc4af096fde3a7e4deb83ec9d75ef
pdf_url: https://arxiv.org/pdf/1508.03695v1.pdf

[page 1]
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
principals of fault-tolerance from largely a classical fra me-
work. As quantum fault-tolerance essentially is avoiding t he
uncontrollable cascade of errors caused by the interaction
of quantum-bits, these concepts can be directly mapped to
quantum information.
Categories and Subject Descriptors
H.4 [Information Systems Applications ]: Miscellaneous;
D.2.8 [ Software Engineering ]: Metrics— complexity mea-
sures, performance measures
General Terms
Quantum Information, Quantum Error Correction
Keywords
ACM proceedings, LATEX, text tagging
1. INTRODUCTION
Fault-tolerant, error corrected, digital quantum computi ng
underpins a signiﬁcant worldwide eﬀort to construct viable ,
commercial quantum computing systems [7]. The size of
such error corrected machines is somewhat daunting for a
ﬁeld that has only managed to experimentally fabricate ar-
rays of up to about ten functional quantum-bits (qubits)
∗(Does NOT produce the permission block, copyright
information nor page numbering). For use with
ACM
PROC ARTICLE-SP.CLS. Supported by ACM.
[4, 3]. However, the theoretical framework for fault-toler ant
quantum computing has existed for nearly 20 years and is
very well understood and quantum computing is competing
with the vast classical computing power currently in exis-
tence. It would be unreasonable to believe (even given the
apparent computational power quantum information pro-
cessing has over classical computing), that a small, error
prone array of qubits could computationally outperform a
classical system comprising of potentially millions of com -
puting cores, each itself containing billions (or even tril lions)
of transistors.
Fault-tolerant quantum computing refers to the framework
of ideas that allow qubits to be protected from quantum
errors introduced by poor control or environmental interac -
tions (Quantum Error Correction, QEC) and the appropri-
ate design of quantum circuits to implement both QEC and
encoded logic operations in a way to avoid these errors cas-
cading through quantum circuits [8]. By avoiding a cascade
of errors, there becomes a point (when the fundamental ac-
curacy of individual qubits is high enough), where QEC is
correcting more errors than are being created. Once this
threshold has been achieved, expanding the size of the pro-
tective quantum will exponentially decrease the failure of
the encoded information and allows us to achieve arbitraril y
long quantum algorithms implemented with noisy devices.
In this paper we will provide a basic introduction to some
of the key principals of QEC and then pivot into a discus-
sion about fault-tolerance that have been investigated in t he
classical computing world. As the goal of fault-tolerance i s
to prevent errors to cascade uncontrollably, a large amount
of classical work can be easily transferred to the quantum
world.
2. QUANTUM COMPUTING
In this section we
[truncated]
```
