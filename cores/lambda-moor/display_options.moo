object DISPLAY_OPTIONS
  name: "Display Options"
  parent: GENERIC_OPTIONS
  owner: HACKER
  readable: true

  property show_blank_tnt (owner: HACKER, flags: "rc") = {
    "Treat `this none this' verbs like the others.",
    "Blank out the args on `this none this' verbs."
  };
  property show_shortprep (owner: HACKER, flags: "rc") = {"Display prepositions in full.", "Use short forms of prepositions."};
  property show_thisonly (owner: HACKER, flags: "rc") = {
    "./: will show ancestor properties/verbs if none on this.",
    "./: will not show ancestor properties/verbs."
  };

  override _namelist = "!blank_tnt!shortprep!thisonly!";
  override aliases = {"Display Options"};
  override names = {"blank_tnt", "shortprep", "thisonly"};
  override object_size = {809, 1084848672};
endobject