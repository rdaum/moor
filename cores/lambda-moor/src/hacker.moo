object HACKER
  name: "Hacker"
  parent: PROG
  owner: HACKER
  player: true
  programmer: true
  readable: true

  override aliases (owner: #2, flags: "r") = {"Hacker"};
  override description = "A system character used to own non-wizardly system verbs , properties, and objects in the core.";
  override features = {PASTING_FEATURE, STAGE_TALK};
  override home = #-1;
  override import_export_id = "hacker";
  override last_disconnect_time = 2147483647;
  override mail_forward = {#2};
  override object_size = {2102, 1084848672};
  override owned_objects = 0;
  override ownership_quota = 37331;
  override size_quota = {100000008, -27508461, 1008125633, 1510455};

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.mail_forward = {$owner};
    endif
  endverb
endobject