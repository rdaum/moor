object FIRST_ROOM [
  import_export_id -> "first_room",
  import_export_hierarchy -> {"initial"}
]
  name: "The First Room"
  parent: ROOM
  location: FIRST_AREA
  owner: ARCH_WIZARD
  readable: true

  property initialized (owner: ARCH_WIZARD, flags: "") = false;

  override description (owner: ARCH_WIZARD, flags: "rc") = "Welcome to Cowbell. Type 'examples' to visit rooms that demonstrate passages, doors, keys, and portals. Each room explains what to try and how to return. Type 'help' for commands.";

  method initialize owner: ARCH_WIZARD
    "Initialize the shipped room's objects once. Restarts preserve later builder edits.";
    caller_perms().wizard || raise(E_PERM);
    this == $first_room || raise(E_INVARG);
    this.initialized && return;
    const description = this.description;
    pass(@args);
    this.description = description;
    for object in ({$couch, $henri, $kibble_cupboard})
      object:initialize();
    endfor
    this.initialized = true;
  endmethod
endobject
