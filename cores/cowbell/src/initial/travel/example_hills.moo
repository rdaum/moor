object EXAMPLE_HILLS [
  import_export_id -> "example_hills",
  import_export_hierarchy -> {"initial", "travel"}
]
  name: "The Example Hills"
  parent: AREA
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "A second area for the travel examples. An ordinary passage joins its overlook to the courtyard in Travel Examples.";
endobject
