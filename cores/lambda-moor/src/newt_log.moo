object NEWT_LOG
  name: "Site-Locks"
  parent: MAIL_RECIPIENT
  location: MAIL_AGENT
  owner: #2

  override aliases (owner: HACKER, flags: "r") = {"Site-Locks"};
  override description = "Notes on annoying sites.";
  override import_export_id = "newt_log";
  override mail_forward = {};
  override mail_notify = {#2};
  override moderated = 1;
  override object_size = {1042, 1084848672};

  verb init_for_core (this none this) owner: #2 flags: "rxd"
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