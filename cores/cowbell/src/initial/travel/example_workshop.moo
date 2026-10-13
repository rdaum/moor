object EXAMPLE_WORKSHOP [
  import_export_id -> "example_workshop",
  import_export_hierarchy -> {"initial", "travel"}
]
  name: "The Example Workshop"
  parent: ROOM
  location: TRAVEL_EXAMPLES
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "The brass door connects this workshop to the gallery. Its open and locked state is shared by both sides. Try close gallery, then open gallery. If someone locks the door, south always leads to the courtyard. From there, west returns to the gallery. Leave the example key in the gallery for the next visitor.";
endobject
