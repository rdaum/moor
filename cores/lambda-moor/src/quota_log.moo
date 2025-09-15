object QUOTA_LOG
  name: "Quota-Log"
  parent: MAIL_RECIPIENT
  location: MAIL_AGENT
  owner: BYTE_QUOTA_UTILS_WORKING

  override aliases (owner: HACKER, flags: "r") = {"Quota-Log", "Quota_Log", "QL", "Quota"};
  override description = "Record of whose quota has been messed with and why.";
  override mail_forward = {};
  override mail_notify = {BYTE_QUOTA_UTILS_WORKING};
  override moderated = 1;
  override object_size = {1113, 1084848672};

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.mail_notify = {player};
      player:set_current_message(this, 0, 0, 1);
      this.moderated = 1;
    else
      raise(E_PERM);
    endif
  endverb
endobject