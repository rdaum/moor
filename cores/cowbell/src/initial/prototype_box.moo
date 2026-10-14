object PROTOTYPE_BOX [
  import_export_id -> "prototype_box",
  import_export_hierarchy -> {"initial"}
]
  name: "prototype box"
  parent: CONTAINER
  location: FIRST_ROOM
  owner: ARCH_WIZARD
  readable: true

  override aliases = {"box"};
  override listing_form = 'definite;

  override description (owner: ARCH_WIZARD, flags: "rc") = "A container for storing generic prototype objects. These objects serve as templates and base classes for the system.";
endobject
