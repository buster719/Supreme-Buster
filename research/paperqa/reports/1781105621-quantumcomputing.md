# PaperQA Literature Report

- timestamp_secs: 1781105621
- domain: QuantumComputing
- task_id: ad-hoc
- status: failed
- command: `../tools/paperqa/.venv/bin/pqa --settings fast -i buster-papers ask What milestones does the local corpus identify for fault-tolerant quantum computing?`

## Question

What milestones does the local corpus identify for fault-tolerant quantum computing?

## Answer

[23:34:09] New file to index: QuantumComputing/1508.03695v1-66be1c2804c2.pdf... 
           Error parsing QuantumComputing/1508.03695v1-66be1c2804c2.pdf,        
           skipping index for this file.                                        
           ╭──────────────── Traceback (most recent call last) ────────────────╮
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/litellm/llms/openai/openai.py:904 in │
           │ acompletion                                                       │
           │                                                                   │
           │    901 │   │   │   2                                              │
           │    902 │   │   ):  # if call fails due to alternating messages, r │
           │    903 │   │   │   try:                                           │
           │ ❱  904 │   │   │   │   openai_aclient: AsyncOpenAI = self._get_op │
           │    905 │   │   │   │   │   is_async=True,                         │
           │    906 │   │   │   │   │   api_key=api_key,                       │
           │    907 │   │   │   │   │   api_base=api_base,                     │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/litellm/llms/openai/openai.py:386 in │
           │ _get_openai_client                                                │
           │                                                                   │
           │    383 │   │   │   │   ):                                         │
           │    384 │   │   │   │   │   return cached_client                   │
           │    385 │   │   │   if is_async:                                   │
           │ ❱  386 │   │   │   │   _new_client: Union[OpenAI, AsyncOpenAI] =  │
           │    387 │   │   │   │   │   api_key=api_key,                       │
           │    388 │   │   │   │   │   base_url=api_base,                     │
           │    389 │   │   │   │   │   http_client=OpenAIChatCompletion._get_ │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/openai/_client.py:496 in __init__    │
           │                                                                   │
           │    493 │   │   if api_key is None:                                │
           │    494 │   │   │   api_key = os.environ.get("OPENAI_API_KEY")     │
           │    495 │   │   if api_key is None:                                │
           │ ❱  496 │   │   │   raise OpenAIError(                             │
           │    497 │   │   │   │   "The api_key client option must be set eit │
           │        client or by setting the OPENAI_API_KEY environment variab │
           │    498 │   │   │   )                                              │
           │    499 │   │   if callable(api_key):                              │
           ╰───────────────────────────────────────────────────────────────────╯
           OpenAIError: The api_key client option must be set either by passing 
           api_key to the client or by setting the OPENAI_API_KEY environment   
           variable                                                             
                                                                                
           During handling of the above exception, another exception occurred:  
                                                                                
           ╭──────────────── Traceback (most recent call last) ────────────────╮
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/litellm/main.py:622 in acompletion   │
           │                                                                   │
           │    619 │   │   │   │   response = ModelResponse(**init_response)  │
           │    620 │   │   │   response = init_response                       │
           │    621 │   │   elif asyncio.iscoroutine(init_response):           │
           │ ❱  622 │   │   │   response = await init_response                 │
           │    623 │   │   else:                                              │
           │    624 │   │   │   response = init_response  # type: ignore       │
           │    625                                                            │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/litellm/llms/openai/openai.py:990 in │
           │ acompletion                                                       │
           │                                                                   │
           │    987 │   │   │   │   │   error_headers = getattr(exception_resp │
           │    988 │   │   │   │   message = getattr(e, "message", str(e))    │
           │    989 │   │   │   │                                              │
           │ ❱  990 │   │   │   │   raise OpenAIError(                         │
           │    991 │   │   │   │   │   status_code=status_code,               │
           │    992 │   │   │   │   │   message=message,                       │
           │    993 │   │   │   │   │   headers=error_headers,                 │
           ╰───────────────────────────────────────────────────────────────────╯
           OpenAIError: The api_key client option must be set either by passing 
           api_key to the client or by setting the OPENAI_API_KEY environment   
           variable                                                             
                                                                                
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
           │ lib/python3.12/site-packages/paperqa/docs.py:199 in aadd          │
           │                                                                   │
           │   196 │   │   │   )                                               │
           │   197 │   │   │   if not texts or not texts[0].text.strip():      │
           │   198 │   │   │   │   raise ValueError(f"Could not read document  │
           │ ❱ 199 │   │   │   result = await llm_model.call_single(           │
           │   200 │   │   │   │   messages=[                                  │
           │   201 │   │   │   │   │   Message(                                │
           │   202 │   │   │   │   │   │   content=parse_config.citation_promp │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/lmi/llms.py:725 in call_single       │
           │                                                                   │
           │    722 │   │   if isinstance(messages, str):                      │
           │    723 │   │   │   # convenience for single message               │
           │    724 │   │   │   messages = [Message(content=messages)]         │
           │ ❱  725 │   │   results = await self.call(                         │
           │    726 │   │   │   messages,                                      │
           │    727 │   │   │   callbacks,                                     │
           │    728 │   │   │   name,                                          │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/lmi/llms.py:672 in call              │
           │                                                                   │
           │    669 │   │   │   │   │   **chat_kwargs,                         │
           │    670 │   │   │   │   )                                          │
           │    671 │   │   elif callbacks is None:                            │
           │ ❱  672 │   │   │   results = await self.acompletion(messages, **c │
           │    673 │   │   else:                                              │
           │    674 │   │   │   if tools:                                      │
           │    675 │   │   │   │   raise NotImplementedError("Using tools wit │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/lmi/llms.py:850 in wrapper           │
           │                                                                   │
           │    847 │   │   │   │   │   yield item                             │
           │    848 │   │   │                                                  │
           │    849 │   │   │   return request_limited_generator()             │
           │ ❱  850 │   │   return await func(self, *args, **kwargs)           │
           │    851 │                                                          │
           │    852 │   return wrapper                                         │
           │    853                                                            │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/lmi/llms.py:805 in wrapper           │
           │                                                                   │
           │    802 │   │   │   return rate_limited_generator()                │
           │    803 │   │                                                      │
           │    804 │   │   # We checked isasyncgenfunction above, so this mus │
           │ ❱  805 │   │   result = await func(self, *args, **kwargs)         │
           │    806 │   │   if func.__name__ == "acompletion" and isinstance(r │
           │    807 │   │   │   await self.check_rate_limit(sum(r.completion_c │
           │    808 │   │   return result                                      │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/lmi/llms.py:1189 in acompletion      │
           │                                                                   │
           │   1186 │   │   │   )                                              │
           │   1187 │   │   │   kwargs["tool_choice"] = self.NO_TOOL_CHOICE    │
           │   1188 │   │                                                      │
           │ ❱ 1189 │   │   completions = await track_costs(router.acompletion │
           │   1190 │   │   │   self.name, prompts, **kwargs                   │
           │   1191 │   │   )                                                  │
           │   1192                                                            │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/lmi/cost_tracker.py:133 in           │
           │ wrapped_func                                                      │
           │                                                                   │
           │   130 │   │   model = args[0] if args else kwargs.get("model", "" │
           │   131 │   │   token = _requested_model_ctx.set(model)             │
           │   132 │   │   try:                                                │
           │ ❱ 133 │   │   │   response = await func(*args, **kwargs)          │
           │   134 │   │   │   if GLOBAL_COST_TRACKER.enabled.get():           │
           │   135 │   │   │   │   await GLOBAL_COST_TRACKER.record(response)  │
           │   136 │   │   │   return response                                 │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/litellm/router.py:1741 in            │
           │ acompletion                                                       │
           │                                                                   │
           │    1738 │   │   │   │   │   original_exception=e,                 │
           │    1739 │   │   │   │   )                                         │
           │    1740 │   │   │   )                                             │
           │ ❱  1741 │   │   │   raise e                                       │
           │    1742 │                                                         │
           │    1743 │   @staticmethod                                         │
           │    1744 │   def _combine_fallback_usage(                          │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/litellm/router.py:1717 in            │
           │ acompletion                                                       │
           │                                                                   │
           │    1714 │   │   │   if request_priority is not None and isinstanc │
           │    1715 │   │   │   │   response = await self.schedule_acompletio │
           │    1716 │   │   │   else:                                         │
           │ ❱  1717 │   │   │   │   response = await self.async_function_with │
           │    1718 │   │   │   end_time = time.time()                        │
           │    1719 │   │   │   _duration = end_time - start_time             │
           │    1720 │   │   │   asyncio.create_task(                          │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/litellm/router.py:5619 in            │
           │ async_function_with_fallbacks                                     │
           │                                                                   │
           │    5616 │   │   │   )                                             │
           │    5617 │   │   │   return response                               │
           │    5618 │   │   except Exception as e:                            │
           │ ❱  5619 │   │   │   return await self.async_function_with_fallbac │
           │    5620 │   │   │   │   e,                                        │
           │    5621 │   │   │   │   disable_fallbacks,                        │
           │    5622 │   │   │   │   fallbacks,                                │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/litellm/router.py:5576 in            │
           │ async_function_with_fallbacks_common_utils                        │
           │                                                                   │
           │    5573 │   │   │   │   │   )                                     │
           │    5574 │   │   │   │   )                                         │
           │    5575 │   │                                                     │
           │ ❱  5576 │   │   raise original_exception                          │
           │    5577 │                                                         │
           │    5578 │   @tracer.wrap()                                        │
           │    5579 │   async def async_function_with_fallbacks(self, *args,  │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/litellm/router.py:5610 in            │
           │ async_function_with_fallbacks                                     │
           │                                                                   │
           │    5607 │   │   │   │   │   *args, **kwargs, mock_timeout=mock_ti │
           │    5608 │   │   │   │   )                                         │
           │    5609 │   │   │   else:                                         │
           │ ❱  5610 │   │   │   │   response = await self.async_function_with │
           │    5611 │   │   │   if verbose_router_logger.isEnabledFor(logging │
           │    5612 │   │   │   │   verbose_router_logger.debug(f"Async Respo │
           │    5613 │   │   │   response = add_fallback_headers_to_response(  │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/litellm/router.py:5765 in            │
           │ async_function_with_retries                                       │
           │                                                                   │
           │    5762 │   │   │   # raises an exception if this error should no │
           │    5763 │   │   │   # Skip this check if retry policy applies (re │
           │    5764 │   │   │   if not _retry_policy_applies:                 │
           │ ❱  5765 │   │   │   │   self.should_retry_this_error(             │
           │    5766 │   │   │   │   │   error=e,                              │
           │    5767 │   │   │   │   │   healthy_deployments=_healthy_deployme │
           │    5768 │   │   │   │   │   all_deployments=_all_deployments,     │
           │                                                                   │
           │ /mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/ │
           │ lib/python3.12/site-packages/lmi/litellm_patches.py:68 in         │
           │ _patched_should_retry_this_error                                  │
           │                                                                   │
           │    65 │   │   │   if any(pattern in error_message for pattern in  │
           │    66 │   │   │   │   # Don't raise - allow fallback cascade to c │
           │    67 │   │   │   │   return None                                 │
           │ ❱  68 │   │   return original_should_retry_this_error(self, error │
           │    69 │                                                           │
           │    70 │   litellm.Router.should_retry_t
...[truncated]

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
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/litellm/llms/openai/openai.py", line 904, in acompletion
    |     openai_aclient: AsyncOpenAI = self._get_openai_client(  # type: ignore
    |                                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/litellm/llms/openai/openai.py", line 386, in _get_openai_client
    |     _new_client: Union[OpenAI, AsyncOpenAI] = AsyncOpenAI(
    |                                               ^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/openai/_client.py", line 496, in __init__
    |     raise OpenAIError(
    | openai.OpenAIError: The api_key client option must be set either by passing api_key to the client or by setting the OPENAI_API_KEY environment variable
    | 
    | During handling of the above exception, another exception occurred:
    | 
    | Traceback (most recent call last):
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/litellm/main.py", line 622, in acompletion
    |     response = await init_response
    |                ^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/litellm/llms/openai/openai.py", line 990, in acompletion
    |     raise OpenAIError(
    | litellm.llms.openai.common_utils.OpenAIError: The api_key client option must be set either by passing api_key to the client or by setting the OPENAI_API_KEY environment variable
    | 
    | During handling of the above exception, another exception occurred:
    | 
    | Traceback (most recent call last):
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa/agents/search.py", line 522, in process_file
    |     await tmp_docs.aadd(
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/paperqa/docs.py", line 199, in aadd
    |     result = await llm_model.call_single(
    |              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/lmi/llms.py", line 725, in call_single
    |     results = await self.call(
    |               ^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/lmi/llms.py", line 672, in call
    |     results = await self.acompletion(messages, **chat_kwargs)
    |               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/lmi/llms.py", line 850, in wrapper
    |     return await func(self, *args, **kwargs)
    |            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/lmi/llms.py", line 805, in wrapper
    |     result = await func(self, *args, **kwargs)
    |              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/lmi/llms.py", line 1189, in acompletion
    |     completions = await track_costs(router.acompletion)(
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/lmi/cost_tracker.py", line 133, in wrapped_func
    |     response = await func(*args, **kwargs)
    |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools/paperqa/.venv/lib/python3.12/site-packages/litellm/router.py", line 1741, in acompletion
    |     raise e
    |   File "/mnt/c/Users/buster/Documents/自驱动人工智能/tools
...[truncated]
```
