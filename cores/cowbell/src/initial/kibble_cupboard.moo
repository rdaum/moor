object KIBBLE_CUPBOARD [
  import_export_id -> "kibble_cupboard",
  import_export_hierarchy -> {"initial"}
]
  name: "a wooden cupboard"
  parent: CONTAINER
  location: FIRST_ROOM
  owner: ARCH_WIZARD
  readable: true

  property kibble_taken_reaction (owner: HACKER, flags: "r") = 0;
  property waft_reaction (owner: HACKER, flags: "r") = 0;

  override aliases (owner: ARCH_WIZARD, flags: "rc") = {"cupboard", "cabinet", "wooden cupboard"};
  override close_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "close", .for_others = "closes">,
    " ",
    <SUB, .capitalize = false, .type = 'dobj>,
    ", sealing away the kibble."
  };
  override description (owner: ARCH_WIZARD, flags: "rc") = "A sturdy wooden cupboard with a brass lock on the door. It has a faint aroma of cat food emanating from within.";
  override get_denied_msg (owner: ARCH_WIZARD, flags: "rc") = {"The cupboard is far too heavy and unwieldy to pick up."};
  override get_rule (owner: ARCH_WIZARD, flags: "rc") = <RULE,
    .name = 'is_portable, .condition = ['kind -> 'any, 'children -> {}],
    .scope = ['input -> [], 'parameter -> [], 'state -> [], 'binding -> []]>;
  override lock_denied_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'iobj>,
    " won't lock without the proper key."
  };
  override lock_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "lock", .for_others = "locks">,
    " ",
    <SUB, .capitalize = false, .type = 'iobj>,
    " with ",
    <SUB, .capitalize = false, .type = 'dobj>,
    " with a satisfying click."
  };
  override lock_rule (owner: ARCH_WIZARD, flags: "rc") = <RULE,
    .name = 'cupboard_lock_rule,
    .condition = ['kind -> 'compare, 'op -> 'eq,
      'left -> ['kind -> 'input, 'name -> 'Key],
      'right -> ['kind -> 'literal, 'value -> BRASS_KEY]],
    .scope = ['input -> ['Key -> 'object], 'parameter -> [], 'state -> [], 'binding -> []]>;
  override locked (owner: HACKER, flags: "r") = true;
  override open (owner: HACKER, flags: "r") = false;
  override open_locked_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'dobj>,
    " is locked tight. You'll need to unlock it first."
  };
  override open_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "open", .for_others = "opens">,
    " ",
    <SUB, .capitalize = false, .type = 'dobj>,
    ", revealing Henri's kibble storage."
  };
  override put_denied_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'iobj>,
    " is closed. You'll need to open it first."
  };
  override take_denied_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'iobj>,
    " is closed. You'll need to open it first."
  };
  override unlock_denied_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'iobj>,
    " won't budge. It needs the right key."
  };
  override unlock_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "unlock", .for_others = "unlocks">,
    " ",
    <SUB, .capitalize = false, .type = 'iobj>,
    " with ",
    <SUB, .capitalize = false, .type = 'dobj>,
    ". The brass lock clicks open."
  };
  override unlock_rule (owner: ARCH_WIZARD, flags: "rc") = <RULE,
    .name = 'cupboard_unlock_rule,
    .condition = ['kind -> 'compare, 'op -> 'eq,
      'left -> ['kind -> 'input, 'name -> 'Key],
      'right -> ['kind -> 'literal, 'value -> BRASS_KEY]],
    .scope = ['input -> ['Key -> 'object], 'parameter -> [], 'state -> [], 'binding -> []]>;

  method fact_is_portable owner: ARCH_WIZARD
    "Cupboards are not portable.";
    return false;
  endmethod

  method initialize owner: ARCH_WIZARD
    "Install this object's reactions through the same owner-authorized API used by builders.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    set_task_perms(principal);
    pass(@args);
    const condition = "isa(object: input.Item, prototype: " + tostr($cat_kibble) + ")";
    $reaction:install(this, "kibble_taken_reaction",
      $reaction:mk('on_take, condition, {{'trigger, $henri, 'on_kibble_taken}}));
    $reaction:install(this, "waft_reaction", <$reaction, .enabled = true, .trigger = 'on_open, .when = 0, .effects = {
      {'announce, "A waft of kibble-scented air escapes from the cupboard."},
      {'trigger, $henri, 'on_cupboard_open}
    }, .fired_at = 0>);
  endmethod
endobject
