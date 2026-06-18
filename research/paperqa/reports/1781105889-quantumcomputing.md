# PaperQA Literature Report

- timestamp_secs: 1781105889
- domain: QuantumComputing
- task_id: ad-hoc
- status: failed
- command: `../tools/paperqa/.venv/bin/pqa --settings fast -i buster-papers --agent.index.paper_directory . --agent.index.recurse_subdirectories true --agent.index.sync_with_paper_directory true --agent.rebuild_index true --llm openai/mimo-v2-pro ask What milestones does the local corpus identify for fault-tolerant quantum computing?`

## Question

What milestones does the local corpus identify for fault-tolerant quantum computing?

## Answer

[23:38:34] New file to index: QuantumComputing/1508.03695v1-66be1c2804c2.pdf... 
[23:40:25] Error parsing QuantumComputing/1508.03695v1-66be1c2804c2.pdf,        
           skipping index for this file.                                        
           ╭──────────────── Traceback (most recent call last) ────────────────╮
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/pypdf/generic/_image_xobject.py:28   │
           │ in <module>                                                       │
           │                                                                   │
           │    25                                                             │
           │    26                                                             │
           │    27 try:                                                        │
           │ ❱  28 │   from PIL import Image, UnidentifiedImageError           │
           │    29 except ImportError:                                         │
           │    30 │   raise ImportError(                                      │
           │    31 │   │   "pillow is required to do image extraction. "       │
           ╰───────────────────────────────────────────────────────────────────╯
           ModuleNotFoundError: No module named 'PIL'                           
                                                                                
           During handling of the above exception, another exception occurred:  
                                                                                
           ╭──────────────── Traceback (most recent call last) ────────────────╮
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/paperqa/agents/search.py:522 in      │
           │ process_file                                                      │
           │                                                                   │
           │   519 │   │   │                                                   │
           │   520 │   │   │   tmp_docs = Docs()                               │
           │   521 │   │   │   try:                                            │
           │ ❱ 522 │   │   │   │   await tmp_docs.aadd(                        │
           │   523 │   │   │   │   │   path=abs_file_path,                     │
           │   524 │   │   │   │   │   fields=["title", "author", "journal", " │
           │   525 │   │   │   │   │   settings=settings,                      │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/paperqa/docs.py:303 in aadd          │
           │                                                                   │
           │   300 │   │   │   multimodal_kwargs["multimodal_enricher"] = (    │
           │   301 │   │   │   │   all_settings.make_media_enricher()          │
           │   302 │   │   │   )                                               │
           │ ❱ 303 │   │   texts, metadata = await read_doc(                   │
           │   304 │   │   │   path,                                           │
           │   305 │   │   │   doc,                                            │
           │   306 │   │   │   page_size_limit=parse_config.page_size_limit,   │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/paperqa/readers.py:464 in read_doc   │
           │                                                                   │
           │   461 │   │   │   │   path, **parser_kwargs                       │
           │   462 │   │   │   )                                               │
           │   463 │   │   else:                                               │
           │ ❱ 464 │   │   │   parsed_text = cast(SyncPDFParserFn, parse_pdf)( │
           │   465 │   elif str_path.endswith(".txt"):                         │
           │   466 │   │   # TODO: Make parse_text async                       │
           │   467 │   │   parsed_text = await asyncio.to_thread(parse_text, p │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/paperqa_pypdf/reader.py:313 in       │
           │ parse_pdf_to_pages                                                │
           │                                                                   │
           │   310 │   │   │   │   │   # NOTE: if Pillow is not installed,     │
           │   311 │   │   │   │   │   # PyPDF will blow up here with a nice m │
           │   312 │   │   │   │   │   media_list = []                         │
           │ ❱ 313 │   │   │   │   │   for img_idx, img_obj in enumerate(page. │
           │   314 │   │   │   │   │   │   pil_image = cast("Image.Image", img │
           │   315 │   │   │   │   │   │   width, height = pil_image.size      │
           │   316 │   │   │   │   │   │   if pil_image.format == "PNG":       │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/pypdf/_page.py:481 in __iter__       │
           │                                                                   │
           │    478 │   │   return self.get_function(lst[index])               │
           │    479 │                                                          │
           │    480 │   def __iter__(self) -> Iterator[ImageFile]:             │
           │ ❱  481 │   │   for i in range(len(self)):                         │
           │    482 │   │   │   yield self[i]                                  │
           │    483 │                                                          │
           │    484 │   def __str__(self) -> str:                              │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/pypdf/_page.py:443 in __len__        │
           │                                                                   │
           │    440 │   │   self.current = -1                                  │
           │    441 │                                                          │
           │    442 │   def __len__(self) -> int:                              │
           │ ❱  443 │   │   return len(self.ids_function())                    │
           │    444 │                                                          │
           │    445 │   def keys(self) -> list[Union[str, list[str]]]:         │
           │    446 │   │   return self.ids_function()                         │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/pypdf/_page.py:611 in _get_ids_image │
           │                                                                   │
           │    608 │   │   │   return []                                      │
           │    609 │   │   call_stack.append(_i)                              │
           │    610 │   │   if self.inline_images is None:                     │
           │ ❱  611 │   │   │   self.inline_images = self._get_inline_images() │
           │    612 │   │   if obj is None:                                    │
           │    613 │   │   │   obj = self                                     │
           │    614 │   │   if ancest is None:                                 │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/pypdf/_page.py:771 in                │
           │ _get_inline_images                                                │
           │                                                                   │
           │    768 │   │   │   │   if k not in init:                          │
           │    769 │   │   │   │   │   init[k] = v                            │
           │    770 │   │   │   ii["object"] = EncodedStreamObject.initialize_ │
           │ ❱  771 │   │   │   from .generic._image_xobject import _xobj_to_i │
           │    772 │   │   │   extension, byte_stream, img = _xobj_to_image(i │
           │    773 │   │   │   files[f"~{num}~"] = ImageFile(                 │
           │    774 │   │   │   │   name=f"~{num}~{extension}",                │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/pypdf/generic/_image_xobject.py:30   │
           │ in <module>                                                       │
           │                                                                   │
           │    27 try:                                                        │
           │    28 │   from PIL import Image, UnidentifiedImageError           │
           │    29 except ImportError:                                         │
           │ ❱  30 │   raise ImportError(                                      │
           │    31 │   │   "pillow is required to do image extraction. "       │
           │    32 │   │   "It can be installed via 'pip install pypdf[image]' │
           │    33 │   )                                                       │
           ╰───────────────────────────────────────────────────────────────────╯
           ImportError: pillow is required to do image extraction. It can be    
           installed via 'pip install pypdf[image]'

## Tool Stderr Tail

```text
+ Exception Group Traceback (most recent call last):
  |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/bin/pqa", line 10, in <module>
  |     sys.exit(main())
  |              ^^^^^^
  |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa/agents/__init__.py", line 233, in main
  |     ask(args.query, settings)
  |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa/agents/__init__.py", line 110, in ask
  |     return run_or_ensure(
  |            ^^^^^^^^^^^^^^
  |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa/utils.py", line 241, in run_or_ensure
  |     return loop.run_until_complete(coro)
  |            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |   File "/usr/lib/python3.12/asyncio/base_events.py", line 687, in run_until_complete
  |     return future.result()
  |            ^^^^^^^^^^^^^^^
  |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa/agents/main.py", line 71, in agent_query
  |     response = await run_agent(docs, query, settings, agent_type, **runner_kwargs)
  |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa/agents/main.py", line 124, in run_agent
  |     await get_directory_index(settings=settings, build=settings.agent.rebuild_index)
  |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa/agents/search.py", line 693, in get_directory_index
  |     async with anyio.create_task_group() as tg:
  |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/anyio/_backends/_asyncio.py", line 799, in __aexit__
  |     raise BaseExceptionGroup(
  | ExceptionGroup: unhandled errors in a TaskGroup (1 sub-exception)
  +-+---------------- 1 ----------------
    | Traceback (most recent call last):
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/pypdf/generic/_image_xobject.py", line 28, in <module>
    |     from PIL import Image, UnidentifiedImageError
    | ModuleNotFoundError: No module named 'PIL'
    | 
    | During handling of the above exception, another exception occurred:
    | 
    | Traceback (most recent call last):
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa/agents/search.py", line 522, in process_file
    |     await tmp_docs.aadd(
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa/docs.py", line 303, in aadd
    |     texts, metadata = await read_doc(
    |                       ^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa/readers.py", line 464, in read_doc
    |     parsed_text = cast(SyncPDFParserFn, parse_pdf)(path, **parser_kwargs)
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa_pypdf/reader.py", line 313, in parse_pdf_to_pages
    |     for img_idx, img_obj in enumerate(page.images):
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/pypdf/_page.py", line 481, in __iter__
    |     for i in range(len(self)):
    |                    ^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/pypdf/_page.py", line 443, in __len__
    |     return len(self.ids_function())
    |                ^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/pypdf/_page.py", line 611, in _get_ids_image
    |     self.inline_images = self._get_inline_images()
    |                          ^^^^^^^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/pypdf/_page.py", line 771, in _get_inline_images
    |     from .generic._image_xobject import _xobj_to_image  # noqa: PLC0415
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/pypdf/generic/_image_xobject.py", line 30, in <module>
    |     raise ImportError(
    | ImportError: pillow is required to do image extraction. It can be installed via 'pip install pypdf[image]'
    +------------------------------------
```
