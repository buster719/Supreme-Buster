# PaperQA Literature Report

- timestamp_secs: 1781106100
- domain: QuantumComputing
- task_id: ad-hoc
- status: ok
- command: `../tools/paperqa/.venv/bin/pqa --settings fast -i buster-papers --agent.index.paper_directory . --agent.index.recurse_subdirectories true --agent.index.sync_with_paper_directory true --agent.rebuild_index true --llm openai/mimo-v2-pro ask What milestones does the local corpus identify for fault-tolerant quantum computing?`

## Question

What milestones does the local corpus identify for fault-tolerant quantum computing?

## Answer

[23:42:23] Starting paper search for '# Keyword Searches for Fault-Tolerant     
           Quantum Computing Milestones'.                                       
           paper_search for query '# Keyword Searches for Fault-Tolerant Quantum
           Computing Milestones' and offset 0 returned 0 papers.                
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
           Starting paper search for 'Here are three unique searches to help    
           identify milestone papers:'.                                         
           paper_search for query 'Here are three unique searches to help       
           identify milestone papers:' and offset 0 returned 0 papers.          
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
           Starting paper search for '**fault-tolerant quantum computing**,     
           1996-2026'.                                                          
           paper_search for query '**fault-tolerant quantum computing**,        
           1996-2026' and offset 0 returned 0 papers.                           
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
           Starting paper search for '**quantum error correction threshold      
           theorem**, 1996-2010'.                                               
           paper_search for query '**quantum error correction threshold         
           theorem**, 1996-2010' and offset 0 returned 0 papers.                
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
           Starting paper search for '**surface code logical qubit              
           demonstration**, 2020-2026'.                                         
           paper_search for query '**surface code logical qubit demonstration**,
           2020-2026' and offset 0 returned 0 papers.                           
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
           Starting paper search for '**Rationale:**'.                          
           paper_search for query '**Rationale:**' and offset 0 returned 0      
           papers.                                                              
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
           Starting paper search for '- **Search 1** is broad and will capture  
           the overall landscape of fault-tolerant QC progress, including review
           papers that explicitly discuss milestones.'.                         
           Starting paper search for '- **Search 2** is narrower, targeting     
           early foundational theoretical work on the threshold theorem—a       
           critical milestone proving fault tolerance is achievable in          
           principle.'.                                                         
           Starting paper search for '- **Search 3** is very specific and       
           recent, focusing on experimental demonstrations of logical qubits    
           using surface codes, which represent the current frontier of         
           practical milestones in the field.'.                                 
           Generating answer for 'What milestones does the local corpus identify
           for fault-tolerant quantum computing?'.                              
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
[23:42:29] Starting paper search for 'fault-tolerant quantum computing          
           milestones'.                                                         
           paper_search for query 'fault-tolerant quantum computing milestones' 
           and offset 0 returned 0 papers.                                      
           Status: Paper Count=0 | Relevant Papers=0 | Current Evidence=0 |     
           Current Cost=$0.0000                                                 
           Answer: I cannot answer this question due to having no papers.

## Tool Stderr Tail

```text
Encountered exception during tool call for tool paper_search: ValueError('Syntax Error: - Search 1 is broad and will capture the overall landscape of fault-tolerant QC progress, including review papers that explicitly discuss milestones.')
Encountered exception during tool call for tool paper_search: ValueError('Syntax Error: - Search 2 is narrower, targeting early foundational theoretical work on the threshold theorem—a critical milestone proving fault tolerance is achievable in principle.')
Encountered exception during tool call for tool paper_search: ValueError('Syntax Error: - Search 3 is very specific and recent, focusing on experimental demonstrations of logical qubits using surface codes, which represent the current frontier of practical milestones in the field.')
Encountered exception during tool call for tool gather_evidence: EmptyDocsError('Not gathering evidence due to having no papers.')
```
