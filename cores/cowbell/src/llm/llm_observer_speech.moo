object LLM_OBSERVER_SPEECH [
  import_export_id -> "llm_observer_speech",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Observer Speech"
  parent: ROOT
  owner: ARCH_WIZARD

  method mk owner: ARCH_WIZARD
    "Create an immutable speech value; empty text and the exact SILENT decision produce no speech.";
    const {response} = args;
    typeof(response) == TYPE_STR || raise(E_TYPE);
    value_bytes(response) <= 65536 || raise(E_QUOTA, "Observer response exceeds 64 KiB.");
    let text = response:trim();
    "Reasoning markup is not speech. An unfinished reasoning block suppresses its remaining text.";
    while (index(text, "<think>") > 0)
      const start = index(text, "<think>");
      const finish = index(text, "</think>");
      const before = start > 1 ? text[1..start - 1] | "";
      if (finish < start)
        text = before:trim();
        break;
      endif
      const after = finish + 8 <= length(text) ? text[finish + 8..$] | "";
      text = (before + after):trim();
    endwhile
    "Leaked tool markup cannot invoke actions or supply spoken parameters.";
    for marker in ({"<｜DSML｜", "<|DSML|", "<DSML", "<tool_call", "</think>"})
      index(text, marker) > 0 && return <$llm_observer_speech, .text = "">;
    endfor
    text == "SILENT" && (text = "");
    length(text) <= 8192 || raise(E_QUOTA, "Observer speech exceeds 8192 characters.");
    return <$llm_observer_speech, .text = text>;
  endmethod

  method announce owner: ARCH_WIZARD
    "Publish bounded speech as the NPC only while it remains in the captured room.";
    const principal = caller_perms();
    const {observer, destination} = args;
    typeof(observer) == TYPE_OBJ && valid(observer) && isa(observer, $llm_room_observer)
      || raise(E_TYPE);
    valid(principal) && (principal == observer || principal == observer.owner || principal.wizard)
      || raise(E_PERM);
    typeof(destination) == TYPE_OBJ || raise(E_TYPE);
    const speech = $llm_observer_speech:mk(this.text);
    if (length(speech.text) == 0 || !observer.enabled || !valid(destination)
        || observer.location != destination)
      return false;
    endif
    set_task_perms(observer);
    const event = $event:mk_say(observer, $sub:thec('actor), " ",
      $sub:agree('actor, 'name, "say", "says"), ", \"", speech.text, "\"");
    destination:announce(event);
    return true;
  endmethod
endobject
