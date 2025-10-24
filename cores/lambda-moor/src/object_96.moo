object #96
  name: "Editor_Owner"
  parent: BUILDER
  owner: #96
  player: true

  override aliases (owner: #2, flags: "r") = {"Editor_Owner"};
  override description = "This player owns all editor-related verbs.";
  override features = {PASTING_FEATURE, STAGE_TALK};
  override home = LOCAL;
  override last_disconnect_time = 2147483647;
  override mail_forward = {#2};
  override object_size = {2277, 1084848672};
  override owned_objects = {NOTE_EDITOR, VERB_EDITOR, GENERIC_EDITOR, LIST_EDITOR, #96};
  override ownership_quota = -10000;
  override size_quota = {0, -6548, 0, 0};
endobject