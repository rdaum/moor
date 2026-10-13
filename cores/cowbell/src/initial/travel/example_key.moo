object EXAMPLE_KEY [
  import_export_id -> "example_key",
  import_export_hierarchy -> {"initial", "travel"}
]
  name: "an example key"
  parent: THING
  location: EXAMPLE_GALLERY
  owner: ARCH_WIZARD
  readable: true

  override aliases (owner: ARCH_WIZARD, flags: "rc") = {"example key", "key"};
  override description (owner: ARCH_WIZARD, flags: "rc") = "This key opens the brass door in the example gallery. Carry it and try 'open door with example key'. Leave it in the gallery afterward. The workshop also has an unlocked route through the courtyard.";
endobject
