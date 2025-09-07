object NEWT_LOG
  name: "Site-Locks"
  parent: MAIL_RECIPIENT
  location: MAIL_AGENT
  owner: BYTE_QUOTA_UTILS_WORKING

  override aliases (owner: HACKER, flags: "r") = {"Site-Locks"};
  override description = "Notes on annoying sites.";
  override mail_forward = {};
  override mail_notify = {BYTE_QUOTA_UTILS_WORKING};
  override moderated = 1;
  override object_size = {1042, 1084848672};

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.mail_notify = {player};
      player:set_current_message(this, 0, 0, 1);
      this.moderated = 1;
    else
      return E_PERM;
    endif
  endverb
endobject