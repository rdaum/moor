object GARBAGE
  name: "Generic Garbage Object"
  owner: HACKER
  readable: true

  property aliases (owner: HACKER, flags: "r") = {"garbage"};
  property import_export_id (owner: HACKER, flags: "r") = "garbage";

  verb description (this none this) owner: #2 flags: "rxd"
    return "Garbage object " + tostr(this) + ".";
  endverb

  verb look_self (this none this) owner: #2 flags: "rxd"
    player:tell(this:description());
  endverb

  verb "title titlec" (this none this) owner: #2 flags: "rxd"
    return tostr("Recyclable ", this);
  endverb

  verb tell (this none this) owner: #2 flags: "rxd"
    return;
  endverb

  verb do_examine (none none none) owner: #2 flags: "rxd"
    args[1]:notify(tostr(this, " is a garbage object, ready for reuse."));
  endverb
endobject