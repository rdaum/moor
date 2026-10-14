object DM [
  import_export_id -> "dm",
  import_export_hierarchy -> {"messaging"}
]
  name: "Direct Message"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Flyweight delegate for direct messages. DMs are lightweight, ephemeral player-to-player messages that follow the same protocol as letters (from, to, sent, text) but are stored as flyweights rather than full objects.";

  method mk owner: ARCH_WIZARD
    "Create a DM flyweight.";
    "Args: sender, recipient, text";
    const {sender, recipient, msg_text} = args;
    typeof(sender) == TYPE_OBJ && typeof(recipient) == TYPE_OBJ && typeof(msg_text) == TYPE_STR || raise(E_TYPE);
    const loc = valid(sender) ? sender.location | #-1;
    return toflyweight(this, ['from -> sender, 'to -> recipient, 'sent -> time(), 'text -> msg_text, 'location -> loc]);
  endmethod

  method from owner: ARCH_WIZARD
    "Return the sender of this DM.";
    return this.from;
  endmethod

  method to owner: ARCH_WIZARD
    "Return the recipient of this DM.";
    return this.to;
  endmethod

  method sent owner: ARCH_WIZARD
    "Return the timestamp when this DM was sent.";
    return this.sent;
  endmethod

  method text owner: ARCH_WIZARD
    "Return the text content of this DM.";
    return this.text;
  endmethod

  method location owner: ARCH_WIZARD
    "Return the location where the sender was when they sent this DM.";
    return this.location;
  endmethod

  method subject owner: ARCH_WIZARD
    "Return subject (empty for DMs, for protocol compatibility with letters).";
    return "";
  endmethod

  method _display_parts owner: ARCH_WIZARD
    "Capture attribution once for the recipient's displayed and spoken message.";
    set_task_perms(caller_perms());
    const {for_player} = args;
    const sender = this.from;
    const label = valid(sender) ? sender:noun_phrase('label, for_player) | "(unknown sender)";
    const name = valid(sender) ? sender:noun_phrase('definite, for_player) | "unknown";
    const source = valid(sender) ? $format.annotation:object(sender, label) | label;
    const loc = this.location;
    const location_name = valid(loc) ? loc:noun_phrase('definite, for_player) | "";
    const place = valid(loc) ? {" (in ", $format.annotation:object(loc, location_name), ")"} | {};
    const age = time() - this.sent;
    const age_text = age < 60 ? "just now" | age:format_time_seconds() + " ago";
    const header = $format.paragraph:mk(source, @place, " [", age_text, "]:");
    const spoken_place = valid(loc) ? " in " + location_name | "";
    const spoken = "Direct message from " + name + spoken_place + ", " + age_text + ": " + this.text;
    return {$format.block:mk(header, this.text), spoken};
  endmethod

  method display owner: ARCH_WIZARD
    "Construct the recipient's attribution and literal body paragraphs.";
    set_task_perms(caller_perms());
    const {?for_player = this.to} = args;
    return this:_display_parts(for_player)[1];
  endmethod

  method display_short owner: ARCH_WIZARD
    "Short display for listings.";
    sender = this.from;
    sender_name = valid(sender) ? sender.name | "???";
    age = time() - this.sent;
    age_str = age < 60 ? "now" | age:format_time_seconds();
    text = this.text;
    if (length(text) > 40)
      text = text[1..37] + "...";
    endif
    return sender_name + " (" + age_str + "): " + text;
  endmethod

  method display_tts owner: ARCH_WIZARD
    "Return spoken attribution and message text.";
    set_task_perms(caller_perms());
    const {?for_player = this.to} = args;
    return this:_display_parts(for_player)[2];
  endmethod

  method display_event owner: ARCH_WIZARD
    "Build the recipient's private message event with attribution, spoken text, and grouping.";
    set_task_perms(caller_perms());
    const {recipient} = args;
    const group = tosym("dm_" + $url_utils:to_curie_str(this.from));
    const {body, spoken} = this:_display_parts(recipient);
    return $event:mk_dm(recipient, body)
      :with_presentation_hint('inset):with_group(group):with_tts(spoken)
      :with_metadata('dm_from, this.from):with_metadata('dm_location, this.location)
      :with_metadata('dm_content, this.text):with_metadata('dm_timestamp, this.sent);
  endmethod

  method sender_echo_event owner: ARCH_WIZARD
    "Build a receipt with the captured location, recipient reference, and literal message paragraphs.";
    set_task_perms(caller_perms());
    const sender = this.from;
    const recipient = this.to;
    const loc = this.location;
    const recipient_name = valid(recipient) ? recipient:name() | "(unknown recipient)";
    const target = valid(recipient) ? $format.annotation:object(recipient, recipient_name) | recipient_name;
    const location_name = valid(loc) ? loc:noun_phrase('definite, sender) | "";
    const place = valid(loc) ? {" (in ", $format.annotation:object(loc, location_name), ")"} | {};
    const header = $format.paragraph:mk("You", @place, " → ", target, ":");
    const spoken_place = valid(loc) ? " in " + location_name | "";
    const spoken_name = valid(recipient) ? recipient:noun_phrase('definite, sender) | recipient_name;
    const spoken = "Message sent to " + spoken_name + spoken_place + ": " + this.text;
    const group = tosym("dm_" + $url_utils:to_curie_str(recipient));
    return $event:mk_dm(sender, $format.block:mk(header, this.text))
      :with_audience('utility):with_presentation_hint('inset):with_group(group):with_tts(spoken)
      :with_metadata('dm_to, recipient):with_metadata('dm_location, loc)
      :with_metadata('dm_content, this.text):with_metadata('dm_timestamp, this.sent);
  endmethod
endobject
