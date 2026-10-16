object LLM_TOOL_PROVIDER [
  import_export_id -> "llm_tool_provider",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Tool Provider"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property bound_session (owner: ARCH_WIZARD, flags: "") = #-1;
  property session_bound (owner: ARCH_WIZARD, flags: "") = false;

  override description (owner: ARCH_WIZARD, flags: "rc") = "Shared actor validation and exact object resolution for domain tool providers.";

  method _require_tool_dispatch owner: ARCH_WIZARD
    "Validate the entry's captured principal before privileged tool effects. Caller ancestry does not grant authority.";
    const {actor, principal} = args;
    typeof(actor) == TYPE_OBJ && valid(actor) || raise(E_PERM, "Tool actor must be valid");
    principal == actor || principal.wizard || raise(E_PERM, "Tool caller cannot impersonate actor");
  endmethod

  method bind owner: ARCH_WIZARD
    "Create one private provider for an admitted wearable interaction.";
    caller_perms().wizard || raise(E_PERM);
    const {prototype, session, actor} = args;
    prototype in {$llm_programming_tools, $llm_building_tools, $llm_behavior_tools, $llm_project_tools} || raise(E_INVARG);
    $llm_wearable_session:check(session, actor);
    const provider = create(prototype, $arch_wizard, 1);
    provider.bound_session = session;
    provider.session_bound = true;
    return provider;
  endmethod

  method check owner: ARCH_WIZARD
    "Validate the incoming principal and reject released or stale interaction providers.";
    const {provider, actor, principal} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    $llm_tool_provider:_require_tool_dispatch(actor, principal);
    isa(provider, $llm_tool_provider) || raise(E_TYPE);
    if (provider.session_bound)
      valid(provider.bound_session) || raise(E_INVARG, "Tool interaction has closed.");
      $llm_wearable_session:check(provider.bound_session, actor);
    endif
    return true;
  endmethod

  method release owner: ARCH_WIZARD
    "Drop the interaction reference without making retained descriptors usable as unbound tools.";
    caller_perms().wizard || raise(E_PERM);
    const {provider} = args;
    isa(provider, $llm_tool_provider) && provider.session_bound || raise(E_INVARG);
    provider.bound_session = #-1;
  endmethod

  method _preview owner: ARCH_WIZARD
    "Capture precomputed proposal state only when this provider needs interactive approval.";
    const {actor, target, kind, member} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    this.session_bound || return false;
    return this:_snapshot(actor, target, kind, member);
  endmethod

  method _approve owner: ARCH_WIZARD
    "Confirm a stable proposal before effects; stale previews also discard cached approval.";
    const {actor, effect, target, kind, member, request, ?preview = false} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    this.session_bound || return true;
    const session = this.bound_session;
    const snapshot = this:_snapshot(actor, target, kind, member);
    if (typeof(preview) == TYPE_LIST
        && (preview != snapshot || strcmp(toliteral(preview), toliteral(snapshot)) != 0))
      session:forget_approval(actor, effect, target);
      return $llm_tool_result:mk('waiting, "Target changed before its preview.", "stale_confirmation");
    endif
    const proposal = effect in {'program_verb, 'eval} ? request["code"] | toliteral(request);
    const title = tostr(effect) + " on " + tostr(target);
    const rationale = `request["rationale"] ! E_RANGE => ""';
    const content = $format.block:mk($format.title:mk(title), rationale,
      $format.code:mk(proposal, 'moo));
    session:emit(actor, $event:mk_info(actor, content):as_inset():with_group('llm, session.host));
    const displayed = this:_snapshot(actor, target, kind, member);
    if (snapshot != displayed || strcmp(toliteral(snapshot), toliteral(displayed)) != 0)
      session:forget_approval(actor, effect, target);
      return $llm_tool_result:mk('waiting, "Target changed while displaying its preview.", "stale_confirmation");
    endif
    const approval = session:confirm(actor, effect, target, "Apply " + title + "?", effect != 'eval);
    typeof(approval) == TYPE_BOOL && approval || return approval;
    const current = this:_snapshot(actor, target, kind, member);
    if (snapshot != current || strcmp(toliteral(snapshot), toliteral(current)) != 0)
      session:forget_approval(actor, effect, target);
      return $llm_tool_result:mk('waiting, "Target changed while awaiting confirmation.", "stale_confirmation");
    endif
    $llm_wearable_session:check(session, actor);
    return true;
  endmethod

  method _snapshot owner: ARCH_WIZARD
    "Read the native state relevant to a confirmation with the actor's authority.";
    const {actor, target, kind, member} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    valid(target) || raise(E_INVARG, "Target no longer exists.");
    const identity = {target, target.owner};
    if (kind == 'verb)
      return {@identity, verb_info(target, member), verb_args(target, member), verb_code(target, member)};
    elseif (kind == 'property)
      return {@identity, property_info(target, member), target.(member)};
    elseif (kind == 'verbs)
      return {@identity, verbs(target)};
    elseif (kind == 'properties)
      return {@identity, properties(target)};
    elseif (kind == 'object)
      let contents = [];
      for object in (target.contents)
        contents[object] = true;
      endfor
      return {@identity, target.name, parent(target), target.location, contents, target.f};
    elseif (kind == 'eval)
      return identity;
    endif
    raise(E_INVARG, "Unsupported confirmation state.");
  endmethod

  method description_text owner: ARCH_WIZARD
    "Resolve a bounded object description as the authenticated viewer, preserving literal strings.";
    const {target, actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const content = target:description();
    value_bytes(content) <= 16384 || raise(E_QUOTA, "LLM description exceeds 16 KiB.");
    typeof(content) == TYPE_STR && return content;
    const parts = typeof(content) == TYPE_LIST ? content | {content};
    const event = $event:mk_info(actor, @parts):with_this(target):with_dobj(target);
    const rendered = $document:render(event:resolve(actor), 'text_plain)['body];
    value_bytes(rendered) <= 16384 || raise(E_QUOTA, "Rendered LLM description exceeds 16 KiB.");
    return rendered;
  endmethod

  method _resolve_object owner: ARCH_WIZARD
    "Resolve an object or string under the actor; an empty string selects an explicit default.";
    const {spec, actor, ?default = E_INVARG} = args;
    $llm_tool_provider:check(this, actor, caller_perms());
    set_task_perms(actor);
    typeof(spec) == TYPE_OBJ && return spec;
    if (!spec || spec == "")
      typeof(default) == TYPE_ERR && raise(E_INVARG, "Object reference required.");
      return default;
    endif
    typeof(spec) == TYPE_STR || raise(E_TYPE);
    return $match:match_object(spec, actor);
  endmethod
endobject
