# PaperQA Literature Report

- timestamp_secs: 1781105797
- domain: QuantumComputing
- task_id: ad-hoc
- status: ok
- command: `../tools/paperqa/.venv/bin/pqa --settings fast -i buster-papers --llm openai/mimo-v2-pro ask What milestones does the local corpus identify for fault-tolerant quantum computing?`

## Question

What milestones does the local corpus identify for fault-tolerant quantum computing?

## Answer

[23:37:23] Starting paper search for 'quantum error correction milestones,      
           1995-2010'.                                                          
           paper_search for query 'quantum error correction milestones,         
           1995-2010' and offset 0 returned 0 papers.                           
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
           Starting paper search for 'error rate 10^-2 fault tolerance,         
           2010-2023'.                                                          
           paper_search for query 'error rate 10^-2 fault tolerance, 2010-2023' 
           and offset 0 returned 0 papers.                                      
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
           Starting paper search for 'logical qubit scalability fault-tolerant, 
           2020-2026'.                                                          
           paper_search for query 'logical qubit scalability fault-tolerant,    
           2020-2026' and offset 0 returned 0 papers.                           
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
           Generating answer for 'What milestones does the local corpus identify
           for fault-tolerant quantum computing?'.                              
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
[23:37:26] Starting paper search for 'fault-tolerant quantum computing'.        
           paper_search for query 'fault-tolerant quantum computing' and offset 
           0 returned 0 papers.                                                 
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
           Answer: I cannot answer this question due to having no papers.

## Tool Stderr Tail

```text
Encountered exception during tool call for tool gather_evidence: EmptyDocsError('Not gathering evidence due to having no papers.')
```
