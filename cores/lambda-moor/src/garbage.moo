object GARBAGE
  name: "Generic Garbage Object"
  owner: HACKER
  readable: true

  property aliases (owner: HACKER, flags: "r") = {"garbage"};

  verb description (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return "Garbage object " + tostr(this) + ".";
  endverb

  verb look_self (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    player:tell(this:description());
  endverb

  verb "title titlec" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return tostr("Recyclable ", this);
  endverb

  verb tell (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return;
  endverb

  verb do_examine (none none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    args[1]:notify(tostr(this, " is a garbage object, ready for reuse."));
  endverb
endobject