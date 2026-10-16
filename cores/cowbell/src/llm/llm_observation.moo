object LLM_OBSERVATION [
  import_export_id -> "llm_observation",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Observation"
  parent: ROOT
  owner: ARCH_WIZARD

  method check_observer owner: ARCH_WIZARD
    "Authenticate observation capture and rendering without invoking observer code.";
    const {observer, principal} = args;
    typeof(observer) == TYPE_OBJ && valid(observer) && isa(observer, $llm_room_observer)
      || raise(E_TYPE);
    valid(principal) && (principal == observer || principal == observer.owner || principal.wizard)
      || raise(E_PERM);
  endmethod

  method event owner: ARCH_WIZARD
    "Capture a bounded canonical event without rendering it or calling the selected client.";
    const {observer, event} = args;
    $llm_observation:check_observer(observer, caller_perms());
    observer.enabled && valid(observer.location) || return false;
    typeof(event) == TYPE_FLYWEIGHT && event.delegate == $event || return false;
    value_bytes(event) <= 16384 && event:validate() || return false;
    const actor = event.actor;
    typeof(actor) == TYPE_OBJ && valid(actor) && actor != observer || return false;
    isa(actor, $llm_room_observer) && return false;
    event.location == observer.location || return false;
    typeof(event.verb) == TYPE_STR && typeof(observer.significant_events) == TYPE_LIST
      && event.verb in observer.significant_events || return false;
    const target = event.iobj;
    if (typeof(target) == TYPE_OBJ && valid(target) && target != observer)
      return false;
    endif
    const value = <$llm_observation, .kind = 'event, .observer = observer,
      .controller = observer.owner, .room = observer.location, .input = event>;
    value_bytes(value) <= 16384 || return false;
    return value;
  endmethod

  method transfer owner: ARCH_WIZARD
    "Capture a real native gift or removal through the canonical Mr. Welcome callback.";
    const {observer, item, callback} = args;
    $llm_observation:check_observer(observer, caller_perms());
    callback in {"enterfunc", "exitfunc"} || return false;
    const frames = callers();
    length(frames) >= 2 || return false;
    const source = frames[1];
    const native = frames[2];
    source[1] == observer && source[2] == callback && source[4] == $mr_welcome
      && native[4] == #-1 && native[2] == "move" || return false;
    observer.enabled && valid(observer.location) || return false;
    typeof(item) == TYPE_OBJ && valid(item) || return false;
    (callback == "enterfunc" ? item.location == observer | item.location != observer) || return false;
    const name = item.name;
    typeof(name) == TYPE_STR && value_bytes(name) <= 4096 || return false;
    const input = ["action" -> callback == "enterfunc" ? "received" | "removed",
      "object" -> tostr(item), "name" -> name];
    return <$llm_observation, .kind = 'transfer, .observer = observer,
      .controller = observer.owner, .room = observer.location, .input = input>;
  endmethod

  method current owner: ARCH_WIZARD
    "Check the captured observer, controller, and room before consuming an observation.";
    typeof(this) == TYPE_FLYWEIGHT && this.delegate == $llm_observation || raise(E_TYPE);
    $llm_observation:check_observer(this.observer, caller_perms());
    return this.observer.enabled && this.observer.owner == this.controller
      && valid(this.room) && this.observer.location == this.room;
  endmethod

  method submit owner: ARCH_WIZARD
    "Queue one captured input; intake errors cannot escape into room delivery or native movement.";
    $llm_observation:check_observer(this.observer, caller_perms());
    try
      this:current() || return false;
      return $llm_observer_runtime:for_observer(this.observer):submit('observation, this);
    except error (ANY)
      $llm_observation:record_error(this.observer, error[1]);
    endtry
    return false;
  endmethod

  method record_error owner: ARCH_WIZARD
    "Retain only a bounded failure count and the most recent error code for owner diagnostics.";
    const {observer, code} = args;
    $llm_observation:check_observer(observer, caller_perms());
    typeof(code) == TYPE_ERR || raise(E_TYPE);
    const count = is_clear_property(observer, "intake_failures") ? 0 | observer.intake_failures;
    observer.intake_failures = min(count + 1, 1000000);
    observer.last_intake_error = ['code -> code, 'at -> time()];
  endmethod

  method render owner: ARCH_WIZARD
    "Render captured input only inside its observer worker; return false for stale or irrelevant input.";
    const principal = caller_perms();
    $llm_observation:check_observer(this.observer, principal);
    principal.wizard && valid(caller) && isa(caller, $llm_observer_job)
      && caller.requester == this.observer && caller.interaction_task == task_id() || raise(E_PERM);
    $llm_observer_job:check(caller);
    this:current() || return false;
    const observer = this.observer;
    if (this.kind == 'transfer)
      const result = ["type" -> "observation", "addressed" -> true, "content" -> generate_json(this.input)];
      value_bytes(result) <= 16384 || return false;
      return result;
    endif
    this.kind == 'event || raise(E_INVARG);
    const event = this.input;
    valid(event.actor) || return false;
    const destination = this.room;
    let addressed = event.iobj == observer;
    set_task_perms(observer);
    const content = $document:render(event:resolve(observer), 'text_plain)['body];
    value_bytes(content) <= 16384 || return false;
    if (event.verb in {"say", "directed_say", "emote"})
      const own_name = observer.name;
      const mentions_self = length(own_name) > 0 && index(content, own_name) > 0;
      if (!addressed && !mentions_self)
        for other in (destination.contents)
          if (other != observer && valid(other) && isa(other, $llm_room_observer)
              && length(other.name) > 0 && index(content, other.name) > 0)
            return false;
          endif
        endfor
      endif
      addressed = addressed || mentions_self;
    endif
    this:current() || return false;
    const result = ["type" -> "observation", "addressed" -> addressed,
      "content" -> generate_json(["actor" -> tostr(event.actor), "action" -> event.verb, "text" -> content])];
    value_bytes(result) <= 16384 || return false;
    return result;
  endmethod
endobject
