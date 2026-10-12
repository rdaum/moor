// Copyright (C) 2026 The mooR Authors
// SPDX-License-Identifier: GPL-3.0-or-later
object HEADLESS_MESSAGING_SCENARIOS
  name: "Messaging Boundary Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true
  override import_export_id = "headless_messaging_scenarios";
  override import_export_hierarchy = {"tests", "headless"};

  method call_as_programmer owner: RUNTIME_PROGRAMMER
    "Invoke a messaging entry point with a non-wizard principal.";
    const {target, method, @parameters} = args;
    return target:(method)(@parameters);
  endmethod

  method property_as_programmer owner: RUNTIME_PROGRAMMER
    "Read a property without wizard permissions.";
    const {target, property_name} = args;
    return target.(property_name);
  endmethod

  method test_sealed_letter_raw_text_is_private owner: ARCH_WIZARD
    "Public property reads must not bypass a sealed letter's reader policy.";
    const letter = create($letter, #90102, 2);
    try
      letter.text = {"private letter body"};
      letter.author = #90102;
      letter.addressee = #90100;
      letter.sealed = true;
      const result = `this:property_as_programmer(letter, "text") ! E_PERM => E_PERM';
      $test_utils:assert_eq(result, E_PERM, "foreign raw read denied; property info " + toliteral(property_info(letter, "text")));
    finally
      recycle(letter);
    endtry
  endmethod

  method test_letter_do_write_denial_does_not_claim_authorship owner: ARCH_WIZARD
    "A caught helper denial must not leave a forged author on a blank letter.";
    const letter = create($letter, #90102, 2);
    try
      const result = `this:call_as_programmer(letter, "do_write", #90101, "forged", true) ! E_PERM => E_PERM';
      $test_utils:assert_eq(result, E_PERM, "direct write helper denied");
      $test_utils:assert_eq(letter.author, #-1, "denial preserves author");
      $test_utils:assert_eq(letter.text, {}, "denial preserves body");
    finally
      recycle(letter);
    endtry
  endmethod

  method test_letter_edit_callback_requires_matching_target owner: ARCH_WIZARD
    "An editor session for another target cannot write this letter.";
    const letter = create($letter, #90102, 2);
    const session_id = player:start_edit_session($root, "receive_edit", {#-1});
    try
      let result = false;
      try
        result = this:call_as_programmer(letter, "receive_edit", session_id, "forged");
      except error (ANY)
        result = error[1];
      endtry
      $test_utils:assert_eq(result, E_PERM, "foreign callback denied");
      $test_utils:assert_eq(letter.text, {}, "denied callback preserves body");
      $test_utils:assert_eq(letter.author, #-1, "denied callback preserves author");
    finally
      player:end_edit_session(session_id);
      recycle(letter);
    endtry
  endmethod

  method test_letter_edit_without_connection owner: ARCH_WIZARD
    "HTTP editor callbacks save through an authenticated player without a connection view.";
    $test_utils:assert_eq(`connection() ! E_INVARG => E_INVARG', E_INVARG, "callback has no connection");
    const letter = create($letter, player, 2);
    const session_id = player:start_edit_session(letter, "receive_edit", {#-123});
    try
      letter:receive_edit(session_id, "first line\nsecond line");
      $test_utils:assert_eq(letter.text, {"first line", "second line"}, "saved body");
      $test_utils:assert_eq(letter.author, player, "authenticated author");
      letter:receive_edit(session_id, 'close);
      $test_utils:assert_eq(`player:get_edit_session(session_id) ! E_INVARG => E_INVARG', E_INVARG, "close ends session");
    finally
      `player:end_edit_session(session_id) ! E_INVARG';
      recycle(letter);
    endtry
  endmethod

  method test_dm_factory_validates_text_structure owner: ARCH_WIZARD
    "The value factory validates text structure without claiming message authenticity.";
    const result = `this:call_as_programmer($dm, "mk", #90102, #90100, 17) ! E_TYPE => E_TYPE';
    $test_utils:assert_eq(result, E_TYPE, "non-string text rejected");
  endmethod
  method test_dm_receipt_does_not_dispatch_untrusted_delegate owner: ARCH_WIZARD
    "Normalize caller-supplied DM values before callbacks or persistent storage.";
    const previous = #90102.direct_messages;
    const previous_sender = #90102.last_dm_from;
    try
      #90102.direct_messages = {};
      const forged = toflyweight($root, ['from -> #90101, 'to -> #90102, 'text -> "hello", 'sent -> time(), 'location -> #90103]);
      let result = false;
      try
        result = this:call_as_programmer(#90102, "receive_dm", forged);
      except error (ANY)
        result = error[1];
      endtry
      $test_utils:assert_eq(result, E_TYPE, "untrusted delegate rejected before callbacks");
      $test_utils:assert_eq(#90102.direct_messages, {}, "rejected delegate not stored");
    finally
      #90102.direct_messages = previous;
      #90102.last_dm_from = previous_sender;
    endtry
  endmethod

  method test_public_note_accessor_preserves_read_policy owner: ARCH_WIZARD
    "Public notes remain readable through the policy accessor; raw private storage stays closed.";
    const note = create($note, #90102, 2);
    try
      note.text = {"public body"};
      $test_utils:assert_eq(this:call_as_programmer(note, "text"), {"public body"}, "public getter succeeds");
      const public_look = this:call_as_programmer(note, "look_self");
      const public_document = $event:mk_info(#90101, public_look.description):resolve(#90101);
      $test_utils:assert_true(index($document:render(public_document, 'text_plain)['body], "writing") > 0,
        "public look preserves writing hint");
      const raw = `this:property_as_programmer(note, "text") ! E_PERM => E_PERM';
      $test_utils:assert_eq(raw, E_PERM, "raw storage requires owner");
      note.read_rule = $rule_engine:parse_expression("This owner_is(Accessor)?", 'private_note);
      const denied = `this:call_as_programmer(note, "text") ! E_PERM => E_PERM';
      $test_utils:assert_eq(denied, E_PERM, "restricted getter denied");
      const restricted_look = this:call_as_programmer(note, "look_self");
      const restricted_document = $event:mk_info(#90101, restricted_look.description):resolve(#90101);
      $test_utils:assert_eq(index($document:render(restricted_document, 'text_plain)['body], "writing"), 0,
        "restricted look omits writing hint");
      $test_utils:assert_eq(note:text(), {"public body"}, "wizard getter remains available");
    finally
      recycle(note);
    endtry
  endmethod

  method test_dm_acceptance_and_denial_leave_canonical_history owner: ARCH_WIZARD
    "Accept sender-bound messages, reject forged or misaddressed values without history changes.";
    const previous = #90102.direct_messages;
    const previous_sender = #90102.last_dm_from;
    try
      #90102.direct_messages = {};
      const accepted = toflyweight($dm, ['from -> #90101, 'to -> #90102, 'text -> "accepted", 'sent -> time(), 'location -> #90103, 'extra -> "discard"]);
      $test_utils:assert_eq(this:call_as_programmer(#90102, "receive_dm", accepted), true, "offline storage succeeds");
      $test_utils:assert_eq(length(#90102.direct_messages), 1, "one canonical receipt");
      $test_utils:assert_eq(maphaskey(flyslots(#90102.direct_messages[1]), 'extra), false, "caller extras discarded");
      const stored = #90102.direct_messages;
      const forged = toflyweight($dm, ['from -> #90100, 'to -> #90102, 'text -> "forged", 'sent -> time(), 'location -> #90103]);
      const denied = `this:call_as_programmer(#90102, "receive_dm", forged) ! E_PERM => E_PERM';
      $test_utils:assert_eq(denied, E_PERM, "foreign sender denied");
      const wrong_target = toflyweight($dm, ['from -> #90101, 'to -> #90100, 'text -> "wrong", 'sent -> time(), 'location -> #90103]);
      const misaddressed = `this:call_as_programmer(#90102, "receive_dm", wrong_target) ! E_INVARG => E_INVARG';
      $test_utils:assert_eq(misaddressed, E_INVARG, "wrong target denied");
      $test_utils:assert_eq(#90102.direct_messages, stored, "denials preserve accepted history");
      $test_utils:assert_eq(#90102.last_dm_from, #90101, "accepted sender retained");
    finally
      #90102.direct_messages = previous;
      #90102.last_dm_from = previous_sender;
    endtry
  endmethod

  method test_player_message_helpers_require_defining_principal owner: ARCH_WIZARD
    "Receiver-owned identity alone cannot grant a foreign verb access to private messages or edit sessions.";
    const receiver = create($player, #90102, 2);
    try
      add_verb(receiver, {#90101, "rxd", "foreign_messages"}, {"this", "none", "this"});
      set_verb_code(receiver, "foreign_messages", {"return this:all_messages();"});
      const messages = `this:call_as_programmer(receiver, "foreign_messages") ! E_PERM => E_PERM';
      $test_utils:assert_eq(messages, E_PERM, "foreign defining principal cannot read receiver history");
      add_verb(receiver, {#90101, "rxd", "foreign_editor"}, {"this", "none", "this"});
      set_verb_code(receiver, "foreign_editor", {"return this:start_edit_session($root, \"receive_edit\", {});"});
      const editor = `this:call_as_programmer(receiver, "foreign_editor") ! E_PERM => E_PERM';
      $test_utils:assert_eq(editor, E_PERM, "foreign defining principal cannot create receiver sessions");
      $test_utils:assert_eq(receiver.editing_sessions, [], "denial leaves session table unchanged");
    finally
      recycle(receiver);
    endtry
  endmethod

endobject

object #90106
  name: "Runtime Messaging Letter"
  parent: LETTER
  owner: RUNTIME_PROGRAMMER
  location: RUNTIME_PROGRAMMER
endobject

object #90107
  name: "Runtime Messaging Mailbox"
  parent: MAILBOX
  owner: RUNTIME_PROGRAMMER
  location: RUNTIME_ROOM
endobject
