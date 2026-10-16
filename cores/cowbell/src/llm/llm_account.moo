object LLM_ACCOUNT [
  import_export_id -> "llm_account",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Request Accounting"
  parent: ROOT
  owner: ARCH_WIZARD

  method usage owner: ARCH_WIZARD
    "Return validated provider token counts, or false when usage is absent or malformed.";
    const {raw} = args;
    set_task_perms(caller_perms());
    typeof(raw) == TYPE_MAP || return false;
    let result = [];
    for key in ({"prompt_tokens", "completion_tokens", "total_tokens"})
      if (maphaskey(raw, key))
        const value = raw[key];
        typeof(value) == TYPE_INT && value >= 0 && value <= 1000000000 || return false;
        result[key] = value;
      endif
    endfor
    if (!maphaskey(result, "total_tokens"))
      maphaskey(result, "prompt_tokens") && maphaskey(result, "completion_tokens") || return false;
      result["total_tokens"] = result["prompt_tokens"] + result["completion_tokens"];
    endif
    if (maphaskey(result, "prompt_tokens") && maphaskey(result, "completion_tokens"))
      result["total_tokens"] >= result["prompt_tokens"] + result["completion_tokens"] || return false;
    endif
    return result;
  endmethod

  method reserve owner: ARCH_WIZARD
    "Reserve capacity before HTTP can suspend. Only the canonical provider can open a charge.";
    caller == $llm_provider && caller_perms().wizard || raise(E_PERM);
    const {actor, amount, deadline, purpose, model} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM);
    typeof(amount) == TYPE_INT && amount >= 0 && amount <= 10000000 || raise(E_INVARG);
    $llm_account:_reconcile(actor);
    const held = actor.llm_reservations;
    length(held) < 8 || raise(E_QUOTA, "Too many concurrent model requests.");
    let reserved = 0;
    for record in (mapvalues(held))
      reserved = reserved + record['amount];
    endfor
    actor.llm_tokens_used + reserved + amount <= actor.llm_token_budget
      || raise(E_QUOTA, "LLM token budget cannot cover this request.");
    const id = uuid();
    actor.llm_reservations[id] = ['amount -> amount, 'deadline -> deadline,
      'purpose -> purpose, 'model -> model, 'started -> ftime()];
    return id;
  endmethod

  method finish owner: ARCH_WIZARD
    "Settle a reservation once; missing usage retains a conservative charge, never an assumed zero.";
    caller == $llm_provider && caller_perms().wizard || raise(E_PERM);
    return $llm_account:_finish(@args);
  endmethod

  method _finish owner: ARCH_WIZARD
    "Commit one settlement and retain bounded metadata without prompts, keys, or response bodies.";
    caller == $llm_account && caller_perms().wizard || raise(E_PERM);
    const {actor, id, raw_usage, status} = args;
    maphaskey(actor.llm_reservations, id) || return false;
    const record = actor.llm_reservations[id];
    const usage = $llm_account:usage(raw_usage);
    const reported = typeof(usage) == TYPE_MAP;
    const tokens = reported ? usage["total_tokens"] | record['amount];
    actor.llm_tokens_used = actor.llm_tokens_used + tokens;
    actor.llm_reservations = mapdelete(actor.llm_reservations, id);
    let log = actor.llm_usage_log;
    if (length(log) >= 100)
      log = log[length(log) - 98..$];
    endif
    actor.llm_usage_log = {@log, ["id" -> id, "timestamp" -> time(), "tokens" -> tokens,
      "usage" -> reported ? usage | [], "reported" -> reported, "status" -> tostr(status),
      "purpose" -> tostr(record['purpose]), "model" -> record['model],
      "seconds" -> ftime() - record['started]]};
    return true;
  endmethod

  method reconcile owner: ARCH_WIZARD
    "Settle expired requests after cancellation or restart; only the actor or a wizard may request it.";
    const principal = caller_perms();
    const {actor} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    return $llm_account:_reconcile(actor);
  endmethod

  method available owner: ARCH_WIZARD
    "Return unreserved capacity for the authenticated actor, reconciling expired attempts first.";
    const principal = caller_perms();
    const {actor} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) && (actor == principal || principal.wizard) || raise(E_PERM);
    $llm_account:_reconcile(actor);
    let reserved = 0;
    for record in (mapvalues(actor.llm_reservations))
      reserved = reserved + record['amount];
    endfor
    return actor.llm_token_budget - actor.llm_tokens_used - reserved;
  endmethod

  method _reconcile owner: ARCH_WIZARD
    "An expired reservation has an uncertain provider cost and remains charged at its reserved amount.";
    caller == $llm_account && caller_perms().wizard || raise(E_PERM);
    const {actor} = args;
    for field in ({"llm_tokens_used", "llm_usage_log", "llm_reservations"})
      if (is_clear_property(actor, field))
        actor.(field) = field == "llm_tokens_used" ? 0 | field == "llm_usage_log" ? {} | [];
      endif
    endfor
    for id in (mapkeys(actor.llm_reservations))
      if (actor.llm_reservations[id]['deadline] <= time())
        $llm_account:_finish(actor, id, false, 'expired);
      endif
    endfor
  endmethod
endobject
