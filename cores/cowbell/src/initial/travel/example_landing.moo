object EXAMPLE_LANDING [
  import_export_id -> "example_landing",
  import_export_hierarchy -> {"initial", "travel"}
]
  name: "The Chute Landing"
  parent: ROOM
  location: TRAVEL_EXAMPLES
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "A padded mat catches visitors from the gallery's chute. That passage works in only one direction: there is no way back up the chute. Take the ramp to the courtyard, then west to the gallery.";
endobject
