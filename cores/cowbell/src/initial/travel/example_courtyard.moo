object EXAMPLE_COURTYARD [
  import_export_id -> "example_courtyard",
  import_export_hierarchy -> {"initial", "travel"}
]
  name: "The Example Courtyard"
  parent: ROOM
  location: TRAVEL_EXAMPLES
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "A sign points west to the gallery. The stairs, called 'down' from this end, reach the same room by a separate passage. Try west or down. The workshop is also reachable here without a key, so the locked example cannot trap you. North leads into a different area; south from there returns here. The alcove leads to the chute landing.";
endobject
