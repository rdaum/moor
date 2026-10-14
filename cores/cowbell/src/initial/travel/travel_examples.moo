object TRAVEL_EXAMPLES [
  import_export_id -> "travel_examples",
  import_export_hierarchy -> {"initial", "travel"}
]
  name: "Travel Examples"
  parent: AREA
  owner: ARCH_WIZARD
  readable: true

  property initialized (owner: ARCH_WIZARD, flags: "") = false;
  property gallery (owner: ARCH_WIZARD, flags: "r") = EXAMPLE_GALLERY;
  property courtyard (owner: ARCH_WIZARD, flags: "r") = EXAMPLE_COURTYARD;
  property landing (owner: ARCH_WIZARD, flags: "r") = EXAMPLE_LANDING;
  property workshop (owner: ARCH_WIZARD, flags: "r") = EXAMPLE_WORKSHOP;
  property hills (owner: ARCH_WIZARD, flags: "r") = EXAMPLE_HILLS;
  property overlook (owner: ARCH_WIZARD, flags: "r") = EXAMPLE_OVERLOOK;
  property key (owner: ARCH_WIZARD, flags: "r") = EXAMPLE_KEY;
  override description (owner: ARCH_WIZARD, flags: "rc") = "A small world where visitors can try different ways of connecting rooms. Enter from the First Room with 'examples'.";

  method initialize owner: ARCH_WIZARD
    "Install the shipped examples once. Restarts preserve door state and later builder edits.";
    caller_perms().wizard || raise(E_PERM);
    this == $travel_examples || raise(E_INVARG);
    this.initialized && return;
    pass(@args);
    this.hills:initialize();
    const links = {
      {$first_room, "examples", this.gallery, "out"},
      {this.gallery, "east", this.courtyard, "west"},
      {this.gallery, "stairs", this.courtyard, "down"},
      {this.gallery, "chute", this.landing, ""},
      {this.landing, "ramp", this.courtyard, "alcove"},
      {this.workshop, "south", this.courtyard, "workshop"},
      {this.courtyard, "north", this.overlook, "south"}
    };
    for link in (links)
      const {source, outward, destination, inward} = link;
      this:create_passage(source, destination,
        $passage:mk(source, outward, {}, "", true, destination, inward, {}, "", true), true);
    endfor
    let door = $passage:mk(this.gallery, "door", {"brass door"}, "A brass door leads to the workshop.",
      true, this.workshop, "gallery", {"door", "brass door"}, "A brass door leads to the gallery.", true, false);
    door.is_door = true;
    door = door:with_locked(true):with_unlock_rule($rule_engine:parse_expression(
      "input.Key == " + toliteral(this.key), 'example_key, $rule_engine:passage_scope()));
    this:create_passage(this.gallery, this.workshop, door);
    $room:register_exit_provider(this.gallery, this.gallery);
    this.initialized = true;
  endmethod
endobject
