object EXAMPLE_GALLERY [
  import_export_id -> "example_gallery",
  import_export_hierarchy -> {"initial", "travel"}
]
  name: "The Travel Gallery"
  parent: ROOM
  location: TRAVEL_EXAMPLES
  owner: ARCH_WIZARD
  readable: true

  property lift_ready (owner: ARCH_WIZARD, flags: "r") = false;

  override description (owner: ARCH_WIZARD, flags: "rc") = "Try the exits here, then type 'exits' to see all the ways out. East and stairs are separate passages to the same courtyard. The chute goes only one way; return by the ramp and then west. For the brass door, get the example key and try 'open door with example key', then 'go door'. Call the lift with 'call lift', then use 'ride lift' when it arrives. A blue portal reaches another area. A misty arch chooses its destination when you use it. The ferry is visible but unavailable. Type out to return to the First Room.";

  method travel_options owner: ARCH_WIZARD
    "Describe the gallery's custom travel without moving or authorizing the viewer.";
    set_task_perms(caller_perms());
    const {source, viewer} = args;
    source == this && this == $travel_examples.gallery || return {};
    let lift = ['id -> "lift", 'label -> "lift", 'command -> this.lift_ready ? "ride lift" | "call lift",
      'description -> this.lift_ready ? "The lift stands ready. Use ride lift." | "A button calls the hillside lift.",
      'ambient -> true];
    if (this.lift_ready)
      lift['destination] = $travel_examples.overlook;
    endif
    return {lift,
      ['id -> "blue", 'label -> "blue portal", 'command -> "enter blue portal",
        'destination -> $travel_examples.overlook, 'description -> "A blue arch opens onto a distant hillside.",
        'ambient -> true],
      ['id -> "mist", 'label -> "misty arch", 'command -> "enter misty arch",
        'description -> "Mist hides the destination of a second arch.", 'ambient -> true],
      ['id -> "ferry", 'label -> "ferry", 'command -> "board ferry", 'available -> false,
        'reason -> "The ferry is out of service."]
    };
  endmethod

  method use_exit owner: ARCH_WIZARD
    "Choose a portal destination for the checked room action. The room performs the move.";
    caller == $room || raise(E_PERM);
    const {who, source, option} = args;
    (this == $travel_examples.gallery && source == this && who.location == source) || return false;
    if (option == "lift")
      const ready = this.lift_ready;
      this.lift_ready = !ready;
      $room:exit_options_changed(this, this);
      if (!ready)
        who:inform_current($event:mk_info(who, "The lift arrives. Use ride lift to reach the hillside."));
        return false;
      endif
      return $travel_examples.overlook;
    endif
    option in {"blue", "mist"} || return false;
    return option == "blue" ? $travel_examples.overlook
      | { $travel_examples.courtyard, $travel_examples.landing }[random(2)];
  endmethod
endobject
