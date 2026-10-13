object EXAMPLE_OVERLOOK [
  import_export_id -> "example_overlook",
  import_export_hierarchy -> {"initial", "travel"}
]
  name: "The Hillside Overlook"
  parent: ROOM
  location: EXAMPLE_HILLS
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "You are in the Example Hills, an area owned by a different builder. The ordinary passage south returns to the courtyard in Travel Examples. Area boundaries group rooms; they do not block travel. The blue portal also arrives here from the gallery. Take south, then west, to return without using a portal.";
endobject
