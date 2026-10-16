object #90030
  name: "Deterministic change-tool model fixture"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property phase (owner: ARCH_WIZARD, flags: "rc") = -1;
  property seen (owner: ARCH_WIZARD, flags: "rc") = {};
  property review (owner: ARCH_WIZARD, flags: "rc") = 0;
  property generation (owner: ARCH_WIZARD, flags: "rc") = 0;
  property member (owner: ARCH_WIZARD, flags: "rc") = "";
  property backend (owner: ARCH_WIZARD, flags: "rc") = "";
  property expect_denied (owner: ARCH_WIZARD, flags: "rc") = false;
  property revoke_before_apply (owner: ARCH_WIZARD, flags: "rc") = false;
  property recover_after_edit (owner: ARCH_WIZARD, flags: "rc") = false;
  property recovered (owner: ARCH_WIZARD, flags: "rc") = false;
  property inspect_only (owner: ARCH_WIZARD, flags: "rc") = false;
  property package (owner: ARCH_WIZARD, flags: "rc") = "";
  property source (owner: ARCH_WIZARD, flags: "rc") = {};

  method chat owner: ARCH_WIZARD
    "Return deterministic model messages. Drive all changes through the advertised tools.";
    const {context, ?opts = false, ?model = false, ?stream = false, ?tools = {}, ?purpose = 'conversation} = args;
    let last = [];
    for message in (context)
      if (message["role"] == "tool" && index(`message["name"] ! E_RANGE => ""', "changes_") == 1)
        last = parse_json(message["content"]);
      endif
    endfor
    if (last)
      this.seen = {@this.seen, last};
      if (this.expect_denied)
        !last["ok"] && last["error"]["code"] == "permission_denied" || raise(E_INVARG, "Unprivileged requester gained tool authority.");
        if (this.revoke_before_apply)
          player.wizard = true;
        endif
        this.phase = 9;
        last = ["ok" -> true, "result" -> []];
      endif
      last["ok"] || raise(E_INVARG, generate_json(last));
      const result = last["result"];
      if (maphaskey(result, "review_id"))
        this.review = result["review_id"];
        this.generation = result["generation"];
      endif
    endif
    let name = "";
    let input = ["review" -> this.review, "generation" -> this.generation];
    const phase = this.phase;
    if (phase == -1)
      name = "changes_packages";
      input = [];
    elseif (phase == 0)
      name = "changes_check";
      input = ["package" -> this.package, "sources" -> {["label" -> "agent.moo", "text" -> this.source]}];
    elseif (phase == 1)
      name = "changes_review";
    elseif (phase == 2 && !this.inspect_only)
      this.member = last["result"]["rows"][1]["id"];
      name = "changes_resolve";
      input["member"] = this.member;
      input["choice"] = "edited";
      input["program"] = "return (;";
    elseif (phase == 3)
      last["result"]["validation"][1]["valid"] == false || raise(E_INVARG, "Invalid draft was accepted.");
      name = "changes_resolve";
      input["member"] = this.member;
      input["choice"] = "edited";
      input["program"] = "const label = \"#9500\";\nreturn {label, 42};";
    elseif (phase == 4)
      last["result"]["validation"][1]["valid"] || raise(E_INVARG, "Multiline draft was lost.");
      if (this.recover_after_edit && !this.recovered)
        set_verb_code(#9500, "probe", {"return 99;"});
        suspend(0.01);
        name = "changes_refresh";
        this.phase = 9;
      else
        name = "changes_prepare_apply";
      endif
    elseif (phase == 5)
      last["result"]["can_apply"] || raise(E_INVARG, "Prepared apply was blocked.");
      name = "changes_apply";
      input["prepared"] = last["result"]["prepared"];
      if (this.revoke_before_apply)
        player.wizard = false;
        this.expect_denied = true;
        suspend(0.01);
      endif
    elseif (phase == 6)
      name = "changes_status";
      input = ["review" -> this.review];
      if (last["result"]["status"] == "applying")
        this.phase = phase - 1;
        suspend(0.01);
      endif
    elseif (phase == 7)
      last["result"]["status"] == "complete" || raise(E_INVARG, "Apply did not complete.");
      name = "changes_result";
    elseif (phase == 8)
      last["result"]["decision_count"] > 0 || raise(E_INVARG, "Missing committed decisions.");
      name = "changes_history";
      input = ["package" -> this.package];
    elseif (phase == 10)
      last["result"]["recovery_count"] == 1 || raise(E_INVARG, "Refresh lost the saved draft.");
      name = "changes_recovery";
    elseif (phase == 11)
      const items = last["result"]["items"];
      length(items) == 1 && items[1]["reason"] != "" || raise(E_INVARG, "Changed live code did not preserve a draft for review.");
      items[1]["id"] == this.member || raise(E_INVARG, "Recovery changed the member identity.");
      name = "changes_resolve";
      input["member"] = this.member;
      input["choice"] = "edited";
      input["program"] = items[1]["choice"]["program"];
      this.recovered = true;
      this.phase = 3;
    else
      return ["choices" -> {["message" -> ["role" -> "assistant", "content" -> "Change review finished."]]}];
    endif
    this.phase = this.phase + 1;
    return ["choices" -> {["message" -> ["role" -> "assistant", "tool_calls" -> {["id" -> tostr(phase), "type" -> "function", "function" -> ["name" -> name, "arguments" -> generate_json(input)]]}]]}];
  endmethod

  method run_scenario owner: ARCH_WIZARD
    "Exercise actual room orchestration with a deterministic model and no external credentials.";
    const {backend, inspect_only, ?revoke = false, ?recover = false} = args;
    player.wizard || raise(E_PERM);
    set_task_perms(player);
    const room = create(backend == "base" ? $agent_room | $llm_coding_room, player, 2);
    const model = create(this, player, 2);
    const package = "agent_fixture";
    const source = {"object #9500", tostr("owner: ", player), tostr("verb probe (this none this) owner: ", player, " flags: \"rxd\""), "return 1;", "endverb", "endobject"};
    try
      if (valid(#9500))
        recycle(#9500);
      endif
      load_object(source);
      $change_manager:configure(package, {#9500}, [], "", `$change_manager.packages[package]["generation"] ! E_RANGE => 0');
      const adoption = $change_manager:stage(package, {["label" -> "agent.moo", "text" -> source]}, "adopt");
      $change_manager:apply(adoption["review_id"], adoption["generation"]);
      while ($change_manager:status(adoption["review_id"])["status"] == "applying")
        suspend(0.01);
      endwhile
      model.backend = backend;
      model.inspect_only = inspect_only;
      model.revoke_before_apply = revoke;
      model.recover_after_edit = recover;
      model.package = package;
      model.source = source;
      model.source[4] = "return 2;";
      room.llm_client = model;
      add_verb(room, {player, "rxd", "execute_fixture"}, {"this", "none", "this"});
      set_verb_code(room, "execute_fixture", {"return this:_execute_task(args[1]);"});
      let task = ['player -> player, 'query -> inspect_only ? "Find out what is new." | "Find out what is new and merge it in. Use the edited program from the fixture."];
      task["authorization"] = $agent_room:_authorize_task(room, task);
      room:execute_fixture(task);
      !recover || model.recovered || raise(E_INVARG, "Room did not recover its saved draft.", {model.phase, model.seen[$]});
      const expected = (inspect_only || revoke) ? 1 | {"#9500", 42};
      #9500:probe() == expected || raise(E_INVARG, "Room did not preserve the requested effects.", room.history);
      length(model.seen) >= (inspect_only ? 2 | revoke ? 7 | 8) || raise(E_INVARG, "Room bypassed tool workflow.", room.history);
      if (inspect_only || revoke)
        const status = $change_manager:status(model.review);
        $change_manager:discard(model.review, status["generation"]);
      endif
      return true;
    finally
      if (revoke)
        this:restore_requester(player);
      endif
      valid(room) && recycle(room);
      valid(model) && recycle(model);
    endtry
  endmethod
  method restore_requester owner: ARCH_WIZARD
    "Restore the isolated fixture requester even if its room execution failed.";
    caller == this || raise(E_PERM);
    args[1].wizard = true;
  endmethod

  method run_denied owner: ARCH_WIZARD
    "Room and model ownership must not grant their requester administrator permissions.";
    const {backend} = args;
    const room = create(backend == "base" ? $agent_room | $llm_coding_room, $arch_wizard, 2);
    const model = create(this, $arch_wizard, 2);
    try
      model.backend = backend;
      model.expect_denied = true;
      room.llm_client = model;
      add_verb(room, {$arch_wizard, "rxd", "execute_fixture"}, {"this", "none", "this"});
      set_verb_code(room, "execute_fixture", {"return this:_execute_task(args[1]);"});
      let task = ['player -> player, 'query -> "Find updates and merge them in."];
      task["authorization"] = $agent_room:_authorize_task(room, task);
      room:execute_fixture(task);
      length(model.seen) > 0 || raise(E_INVARG, "Room did not call the tool.", room.history);
      for response in (model.seen)
        !response["ok"] && response["error"]["code"] == "permission_denied" || raise(E_INVARG, "Requester authority was not preserved.");
      endfor
      return true;
    finally
      valid(room) && recycle(room);
      valid(model) && recycle(model);
    endtry
  endmethod
endobject
