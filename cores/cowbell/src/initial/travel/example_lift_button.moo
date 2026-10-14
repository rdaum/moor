object EXAMPLE_LIFT_BUTTON [
  import_export_id -> "example_lift_button",
  import_export_hierarchy -> {"initial", "travel"}
]
  name: "lift button"
  parent: THING
  location: EXAMPLE_GALLERY
  owner: ARCH_WIZARD
  readable: true

  override aliases (owner: ARCH_WIZARD, flags: "rc") = {"button"};
  override portable (owner: ARCH_WIZARD, flags: "rc") = false;
  override description (owner: ARCH_WIZARD, flags: "rc") = "Push this button to call the hillside lift. When it arrives, type 'ride lift'.";

  verb "push press" (this none none) owner: ARCH_WIZARD flags: "rd"
    "Call the lift from its button. Pressing again does not board the lift.";
    caller == player || raise(E_PERM);
    set_task_perms(player);
    this.location == $travel_examples.gallery && player.location == this.location
      || raise(E_INVARG, "The lift button is not here.");
    const action = this.location:exit_annotation("lift");
    if (action["command"] == "ride lift")
      return player:inform_current($event:mk_info(player, "The lift is already here. Use ",
        $format.annotation:exit(this.location, "lift", "ride lift"), " to board it."));
    endif
    return $room:use_exit_action(player, action["command"], action["exit"]);
  endverb
endobject
