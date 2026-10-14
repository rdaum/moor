object HOUSEKEEPING_SWEEP_MSGS [
  import_export_id -> "housekeeping_sweep_msgs",
  import_export_hierarchy -> {"initial"}
]
  name: "Housekeeping Sweep Messages"
  parent: MSG_BAG
  owner: ARCH_WIZARD

  override entries (owner: ARCH_WIZARD, flags: "rc") = {
    {
      "A member of housekeeping gently guides ",
      <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'actor>,
      " back to bed."
    },
    {
      "Housekeeping escorts ",
      <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'actor>,
      " away to ",
      <SUB, .capitalize = false, .type = 'pos_adj>,
      " room."
    },
    {
      <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
      " ", <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "are", .singular_form = "is">, " quietly led away by a tired-looking porter."
    },
    {
      "A woman with a broom nudges ",
      <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'actor>,
      " awake and points toward the back stairs."
    },
    {
      <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
      " ", <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "are", .singular_form = "is">, " gently but firmly ushered toward the dormitory."
    }
  };
endobject
