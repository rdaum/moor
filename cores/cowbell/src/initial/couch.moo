object COUCH [
  import_export_id -> "couch",
  import_export_hierarchy -> {"initial"}
]
  name: "ratty couch"
  parent: SITTABLE
  location: FIRST_ROOM
  owner: HACKER
  readable: true

  property henri_disturbed_reaction (owner: HACKER, flags: "r") = 0;

  override aliases (owner: HACKER, flags: "rc") = {"couch", "sofa", "ratty couch"};
  override description (owner: HACKER, flags: "rc") = "A well-worn brown couch that's seen better days. The cushions are slightly lumpy and there's a suspicious amount of black cat hair embedded in the fabric. Despite its shabby appearance, it looks comfortable enough for a quick rest - or for a grouchy cat to claim as his territory.";
  override get_denied_msg (owner: HACKER, flags: "rc") = {"The couch is far too heavy to pick up."};
  override get_rule (owner: HACKER, flags: "rc") = <RULE, .name = 'is_portable, .body = {{'is_portable, {'var, 'This}}}, .variables = {'This}, .head = 'is_portable>;
  override integrated_description (owner: HACKER, flags: "rc") = "A ratty brown couch, thoroughly colonized by cat hair, sits against one wall.";
  override seats (owner: HACKER, flags: "r") = 3;
  override sit_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .capitalize = false, .type = 'self_alt, .for_self = "sink", .for_others = "sinks">,
    " into ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'This>,
    ", stirring up a small cloud of cat hair."
  };
  override sitting_verb (owner: HACKER, flags: "rc") = "lounging";
  override squeeze (owner: HACKER, flags: "rc") = 1;
  override stand_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .capitalize = false, .type = 'self_alt, .for_self = "stand", .for_others = "stands">,
    " up from ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'This>,
    ", brushing off some cat hair."
  };

  method fact_is_portable owner: HACKER
    "Couches are not portable.";
    return false;
  endmethod

  method fact_is_sitting owner: HACKER
    "Rule predicate: Is target sitting on this furniture?";
    {furniture, who} = args;
    return who in furniture.sitting;
  endmethod

  method fact_not_is owner: HACKER
    "Rule predicate: Are two values not equal? First arg is this (for rule dispatch).";
    {_, a, b} = args;
    return a != b;
  endmethod

  method initialize owner: ARCH_WIZARD
    "Install this object's reactions through the same owner-authorized API used by builders.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    set_task_perms(principal);
    pass(@args);
    $reaction:install(this, "henri_disturbed_reaction", <$reaction, .enabled = true, .trigger = 'on_sit, .when = <$rule, .name = 'henri_sitting, .body = {
        {'is_sitting, {'var, 'This}, $henri},
        {'not_is, {'var, 'This}, {'var, 'Actor}, $henri}
      }, .variables = {'This, 'Actor}, .head = 'henri_sitting>, .effects = {{'trigger, $henri, 'on_couch_intruder}}, .fired_at = 0>);
  endmethod
endobject
