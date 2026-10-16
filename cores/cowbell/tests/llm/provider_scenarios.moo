object #90032
  name: "LLM Provider Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD

  property effects (owner: ARCH_WIZARD, flags: "") = 0;
  property restart_job (owner: ARCH_WIZARD, flags: "") = #-1;
  property restart_agent (owner: ARCH_WIZARD, flags: "") = #-1;
  property restart_client (owner: ARCH_WIZARD, flags: "") = #-1;
  property restart_finished (owner: ARCH_WIZARD, flags: "") = false;
  property restart_used (owner: ARCH_WIZARD, flags: "") = 0;
  property restart_log_count (owner: ARCH_WIZARD, flags: "") = 0;
  property restart_reservation (owner: ARCH_WIZARD, flags: "") = "";
  property restart_amount (owner: ARCH_WIZARD, flags: "") = 0;
  property restart_model (owner: ARCH_WIZARD, flags: "") = "";

  method start_restart owner: ARCH_WIZARD
    "Start a real provider request in an anonymous job retained across a disposable server restart.";
    caller_perms().wizard || raise(E_PERM);
    const {endpoint, token, model} = args;
    const actor = this.owner;
    const client = create($llm_client, actor, 1);
    client.api_endpoint = endpoint;
    client.model = model;
    client:set_api_key(token);
    const agent = create($llm_agent, actor, 1);
    agent.client = client;
    agent.chat_opts = $llm_chat_opts:mk():with_max_tokens(768):with_temperature(0);
    const job = $llm_job:new_run(this, actor, connection());
    job:bind_agent(agent);
    this.restart_job = job;
    this.restart_agent = agent;
    this.restart_client = client;
    this.restart_model = model;
    this.restart_finished = false;
    this.restart_used = actor.llm_tokens_used;
    this.restart_log_count = length(actor.llm_usage_log);
    fork (0)
      this:_restart_work();
    endfork
    return true;
  endmethod

  method _restart_work owner: ARCH_WIZARD
    "Keep the host cleanup activation suspended while its tracked LLM job waits for real HTTP.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const job = this.restart_job;
    job:run("Return READY.", 20);
    job:release();
    this.restart_finished = true;
  endmethod

  method capture_restart owner: ARCH_WIZARD
    "Capture the actual reservation after the Python HTTP fixture confirms receipt.";
    caller_perms().wizard || raise(E_PERM);
    const job = this.restart_job;
    job.state == 'running && job.worker_task > 0 && job.parent_task > 0 || raise(E_ASSERT, "LLM job did not start");
    const reservations = this.owner.llm_reservations;
    length(reservations) == 1 || raise(E_ASSERT, "Expected one pending HTTP reservation");
    const id = mapkeys(reservations)[1];
    this.restart_reservation = id;
    this.restart_amount = reservations[id]['amount];
    this.restart_amount > 0 || raise(E_ASSERT, "The pending generation must reserve capacity");
    return true;
  endmethod

  method restart_ready owner: ARCH_WIZARD
    "Poll actual job completion and natural reservation expiry after persistent restart.";
    caller_perms().wizard || raise(E_PERM);
    $llm_account:reconcile(this.owner);
    return this.restart_finished && length(this.owner.llm_reservations) == 0;
  endmethod

  method verify_restart owner: ARCH_WIZARD
    "Verify persistent job release and exactly one conservative charge for the interrupted request.";
    caller_perms().wizard || raise(E_PERM);
    const actor = this.owner;
    $llm_account:reconcile(actor);
    $llm_account:reconcile(actor);
    this.restart_finished || raise(E_ASSERT, "Restarted job never reached host cleanup");
    const job = this.restart_job;
    job.state in {'failed, 'cancelled, 'timed_out} || raise(E_ASSERT, "Interrupted job reported completion");
    job.worker_task == 0 && job.parent_task == 0 || raise(E_ASSERT, "Job retained owned task identities");
    !valid(job.agent) && !valid(job.presenter) && !valid(job.host) && !valid(job.requester)
      || raise(E_ASSERT, "Job retained runtime references");
    job.reply_connection == 0 && job.context == {} || raise(E_ASSERT, "Job retained private output context");
    !valid(this.restart_agent.tool_callback) && !valid(this.restart_agent.compaction_callback)
      || raise(E_ASSERT, "Agent callback cycle survived cleanup");
    this.restart_client.model == this.restart_model && this.restart_client:is_configured()
      || raise(E_ASSERT, "Configured LLM client did not persist");
    actor.llm_tokens_used == this.restart_used + this.restart_amount
      || raise(E_ASSERT, "Interrupted request was charged zero or more than once");
    length(actor.llm_usage_log) == this.restart_log_count + 1 || raise(E_ASSERT, "Settlement count changed");
    const charge = actor.llm_usage_log[$];
    charge["id"] == this.restart_reservation && !charge["reported"]
      || raise(E_ASSERT, "Interrupted request lost its uncertain accounting identity");
    charge["tokens"] == this.restart_amount || raise(E_ASSERT, "Unknown usage did not retain the reservation amount");
    length(actor.llm_reservations) == 0 || raise(E_ASSERT, "Settled reservation remains live");
    return true;
  endmethod

  method request_after_restart owner: ARCH_WIZARD
    "Use the saved client after a second restart; new work receives one independent charge.";
    caller_perms().wizard || raise(E_PERM);
    this:verify_restart();
    const response = this.restart_client:chat("Return READY.", $llm_chat_opts:mk():with_max_tokens(768));
    $llm_response:mk(response):content() == "READY" || raise(E_ASSERT, "Saved client cannot serve fresh work");
    this.owner.llm_tokens_used == this.restart_used + this.restart_amount + 24
      || raise(E_ASSERT, "Fresh request accounting is not independent");
    return true;
  endmethod

  method mark owner: ARCH_WIZARD
    "Record the fixture tool effect under the supplied actor.";
    const {parameters, actor} = args;
    actor == this.owner && caller_perms() == actor || raise(E_PERM);
    typeof(parameters) == TYPE_MAP || raise(E_TYPE);
    this.effects = this.effects + 1;
    return "recorded";
  endmethod

  method run_case owner: ARCH_WIZARD
    "Exercise the actual agent and provider client in a disposable world.";
    caller_perms().wizard || raise(E_PERM);
    const {prototype, delegate, endpoint, token, model, scenario} = args;
    const agent = create(prototype, this.owner, 1);
    try
      agent.client = create($llm_client, this.owner, 1);
      agent.client.api_endpoint = endpoint;
      agent.client.model = model;
      agent.client:set_api_key(token);
      agent.chat_opts = $llm_chat_opts:mk():with_max_tokens(768):with_temperature(0);
      agent.max_iterations = 3;
      agent.system_prompt = "Follow the test request precisely. Keep responses short.";
      agent:reset_context();
      if (scenario == "completion")
        const result = agent:send_message("Reply with only the word READY.");
        typeof(result) == TYPE_STR && index(result, "READY") || raise(E_ASSERT, "Completion missing marker");
        agent.current_iteration == 0 || raise(E_ASSERT, "Completion left agent active");
        agent.total_tokens_used > 0 || raise(E_ASSERT, "Missing completion usage");
      elseif (scenario == "tool")
        this.effects = 0;
        const parameters = ["type" -> "object", "properties" -> [], "additionalProperties" -> false];
        agent:add_tool("mark", delegate:mk("mark", "Record the test effect once", parameters, this, "mark"));
        const result = agent:send_message("Call mark exactly once, then reply with only the word READY. Do not call mark again after its result.");
        this.effects == 1 || raise(E_ASSERT, "Expected one tool effect");
        typeof(result) == TYPE_STR && index(result, "READY") || raise(E_ASSERT, "Continuation missing marker");
        const replies = {m for m in (agent:get_context()) if (m["role"] == "tool")};
        length(replies) == 1 || raise(E_ASSERT, "Tool result missing from context");
        agent.current_iteration == 0 || raise(E_ASSERT, "Tool continuation left agent active");
      elseif (scenario == "compaction")
        agent.min_messages_to_keep = 2;
        for i in [1..6]
          agent:add_message("user", "Remember test fact " + tostr(i));
        endfor
        agent:compact_context();
        length(agent:get_context()) == 4 || raise(E_ASSERT, "Compaction did not replace the old prefix");
        agent:get_context()[$]["content"] == "Remember test fact 6" || raise(E_ASSERT, "Compaction lost recent context");
      else
        raise(E_INVARG, "Unknown provider scenario");
      endif
    finally
      "Both mutable fixtures are anonymous; drop references instead of recycling ordinary objects.";
      agent.client = #-1;
    endtry
    return true;
  endmethod
  method run_observer owner: ARCH_WIZARD
    "Verify dedicated-client triage, visible decisions, and actual NPC usage settlement.";
    caller_perms().wizard || raise(E_PERM);
    const {endpoint, token, model} = args;
    const observer = create($llm_room_observer, this.owner, 1);
    const agent = create($llm_agent, this.owner, 1);
    const client = create($llm_client, this.owner, 1);
    try
      observer.name = "ProviderGuide";
      observer.agent = agent;
      observer.triage_max_tokens = 768;
      agent.client = client;
      client.api_endpoint = endpoint;
      client.model = model;
      client.allowed_users = {observer};
      client:set_api_key(token);
      $llm_observer_policy:ready(observer) || raise(E_ASSERT, "Dedicated observer client not ready");
      for sample in ({
        {"Where am I? How do I check in?", true},
        {"Visitors discuss their weekend with each other.", false}})
        agent:set_context({["role" -> "user", "content" -> "OBSERVATION: " + sample[1]]});
        observer:triage() == sample[2] || raise(E_ASSERT, "Unexpected observer decision");
      endfor
      length(observer.llm_usage_log) == 2 || raise(E_ASSERT, "Missing NPC triage usage");
      for usage in (observer.llm_usage_log)
        usage["purpose"] == "triage" && usage["reported"] && usage["tokens"] > 0
          || raise(E_ASSERT, "Invalid NPC triage accounting");
      endfor
      length(observer.llm_reservations) == 0 || raise(E_ASSERT, "Triage reservation leaked");
    finally
      observer.agent = #-1;
      agent.client = #-1;
      client.allowed_users = {};
    endtry
    return true;
  endmethod

  method change_endpoint owner: ARCH_WIZARD
    "Change configuration during an HTTP wait, in a separate activation from the caller's cleanup.";
    caller == this || raise(E_PERM);
    const {client} = args;
    fork (0)
      suspend(0.1);
      client.api_endpoint = "http://127.0.0.1:1/revoked";
    endfork
  endmethod

  method run_fault owner: ARCH_WIZARD
    "Check actual HTTP errors, bounded retries, redaction, and account settlement with synthetic credentials.";
    caller_perms().wizard || raise(E_PERM);
    const {endpoint, token, model, scenario} = args;
    const client = create($llm_client, this.owner, 1);
    client.api_endpoint = endpoint;
    client.model = model;
    client:set_api_key(token);
    const actor = this.owner;
    const before = length(actor.llm_usage_log);
    const used = actor.llm_tokens_used;
    if (scenario == "configuration_change")
      this:change_endpoint(client);
    endif
    let failed = false;
    let response = [];
    try
      response = client:chat("Return READY.", $llm_chat_opts:mk():with_max_tokens(768));
    except error (E_PERM, E_INVARG, E_QUOTA)
      failed = true;
      !index(toliteral(error), "PRIVATE_PROVIDER_BODY") || raise(E_ASSERT, "Provider error body leaked");
    endtry
    const succeeds = scenario in {"retry429", "retry503", "missing_usage"};
    failed != succeeds || raise(E_ASSERT, "Unexpected fault outcome");
    if (succeeds)
      $llm_response:mk(response):content() == "READY" || raise(E_ASSERT, "Successful retry lost content");
    endif
    const attempts = scenario:starts_with("retry") ? 3 | 1;
    length(actor.llm_usage_log) == before + attempts || raise(E_ASSERT, "Attempt accounting mismatch");
    length(actor.llm_reservations) == 0 || raise(E_ASSERT, "Request left a live reservation");
    actor.llm_tokens_used > used || raise(E_ASSERT, "Unknown costs cannot become zero charges");
    if (!succeeds || scenario == "missing_usage")
      !actor.llm_usage_log[$]["reported"] || raise(E_ASSERT, "Unknown usage must be explicit");
    endif
    return true;
  endmethod
  method run_accounting owner: ARCH_WIZARD
    "Every purpose charges the captured NPC account; shared credentials do not choose the payer.";
    caller_perms().wizard || raise(E_PERM);
    const {endpoint, token, model} = args;
    const client = create($llm_client, this.owner, 1);
    client.api_endpoint = endpoint;
    client.model = model;
    client:set_api_key(token);
    client:set_access(true);
    const actor = create($thing, this.owner, 1);
    const owner_before = this.owner.llm_tokens_used;
    const opts = $llm_chat_opts:mk():with_max_tokens(768);
    for purpose in ({'conversation, 'compaction, 'triage, 'facts, 'suggestion, 'no_tools})
      const response = $llm_provider:chat_as(client, actor, purpose, "Return READY.", opts);
      $llm_response:mk(response):content() == "READY" || raise(E_ASSERT, "Purpose lost response");
      actor.llm_usage_log[$]["purpose"] == tostr(purpose) || raise(E_ASSERT, "Purpose was not accounted");
    endfor
    actor.llm_tokens_used == 144 || raise(E_ASSERT, "NPC usage bypassed accounting");
    length(actor.llm_usage_log) == 6 || raise(E_ASSERT, "Usage was charged twice");
    this.owner.llm_tokens_used == owner_before || raise(E_ASSERT, "Credential owner was charged for another actor");
    const before = this.owner.llm_tokens_used;
    const models = client:list_models();
    models["models"][1]["id"] == model || raise(E_ASSERT, "Model metadata lost");
    this.owner.llm_tokens_used == before || raise(E_ASSERT, "Metadata has no generation tokens");
    this.owner.llm_usage_log[$]["purpose"] == "models" || raise(E_ASSERT, "Metadata attempt was not logged");
    return true;
  endmethod
endobject
