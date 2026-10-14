object PRONOUNS [
  import_export_id -> "pronouns"
]
  name: "Pronoun System"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  readable: true

  property he_him (owner: HACKER, flags: "r") = <PRONOUNS, .verb_be = "is", .verb_have = "has", .is_plural = false, .display = "he/him", .ps = "he", .po = "him", .pp = "his", .pq = "his", .pr = "himself">;
  property it_its (owner: HACKER, flags: "r") = <PRONOUNS, .verb_be = "is", .verb_have = "has", .is_plural = false, .display = "it/its", .ps = "it", .po = "it", .pp = "its", .pq = "its", .pr = "itself">;
  property she_her (owner: HACKER, flags: "r") = <PRONOUNS, .verb_be = "is", .verb_have = "has", .is_plural = false, .display = "she/her", .ps = "she", .po = "her", .pp = "her", .pq = "hers", .pr = "herself">;
  property spivak (owner: HACKER, flags: "r") = <PRONOUNS, .verb_be = "is", .verb_have = "has", .is_plural = false, .display = "e/em", .ps = "e", .po = "em", .pp = "eir", .pq = "eirs", .pr = "emself">;
  property they_them (owner: HACKER, flags: "r") = <PRONOUNS, .verb_be = "are", .verb_have = "have", .is_plural = true, .display = "they/them", .ps = "they", .po = "them", .pp = "their", .pq = "theirs", .pr = "themselves">;

  override description (owner: HACKER, flags: "rc") = "Pronoun system providing preset and custom pronoun sets for objects and players.";

  method from_input owner: HACKER
    "Validate a preset label or the six named fields supplied by commands and clients.";
    const {input} = args;
    if (typeof(input) == TYPE_STR)
      const preset = this:lookup(input);
      typeof(preset) == TYPE_FLYWEIGHT || raise(E_INVARG,
        "Unknown pronoun preset. Choose " + this:list_presets():join(", ")
        + ", or enter custom pronouns.");
      return this:validate(preset);
    endif
    typeof(input) == TYPE_MAP || raise(E_TYPE, "Use a preset name or custom pronoun fields.");
    const fields = {"subject", "object", "possessive_adjective", "possessive_pronoun", "reflexive", "agreement"};
    let words = {};
    for field in (fields)
      maphaskey(input, field) || raise(E_INVARG, "Missing pronoun field: " + field);
      typeof(input[field]) == TYPE_STR || raise(E_TYPE, "Pronoun fields must be text.");
      const word = input[field]:trim();
      (length(word) > 0 && length(word) <= 64 && !index(word, "\n") && !index(word, "\r"))
        || raise(E_INVARG, "Enter one line of 1 to 64 characters for " + field + ".");
      words = {@words, word};
    endfor
    length(input) == length(fields) || raise(E_INVARG, "Unknown custom pronoun field.");
    words[6] in {"is", "are"} || raise(E_INVARG, "Choose is or are for verb agreement.");
    return this:mk(@words[1..5], words[6] == "are");
  endmethod

  method to_input owner: HACKER
    "Return the fields used to edit an existing pronoun set.";
    const {value} = args;
    const checked = this:validate(value);
    return ["subject" -> checked.ps, "object" -> checked.po,
      "possessive_adjective" -> checked.pp, "possessive_pronoun" -> checked.pq,
      "reflexive" -> checked.pr, "agreement" -> checked.verb_be];
  endmethod

  method examples owner: HACKER
    "Preview every form and verb agreement before a user applies a pronoun choice.";
    const {input} = args;
    const pronouns = this:from_input(input);
    const subject = pronouns.ps:capitalize();
    return {subject + " " + pronouns.verb_be + " ready.", "I met " + pronouns.po + ".",
      "This is " + pronouns.pp + " coat.", "The coat is " + pronouns.pq + ".",
      subject + " did it " + pronouns.pr + ".", subject + " " + pronouns.verb_have + " a map."};
  endmethod

  method prompt_custom owner: ARCH_WIZARD
    "Collect custom fields and preview them. Return confirmed input, or false on cancellation.";
    "Args: {editor[, confirmation]}. Each prompt commits; the caller must recheck permission on save.";
    set_task_perms(caller_perms());
    const {editor, ?confirmation = "Use these pronouns?"} = args;
    editor == caller_perms() || raise(E_PERM);
    let input = [];
    for field in ({{"subject", "___ is ready.", "ze"}, {"object", "I met ___.", "zir"},
      {"possessive_adjective", "This is ___ coat.", "zir"},
      {"possessive_pronoun", "The coat is ___.", "zirs"}, {"reflexive", "Ze did it ___.", "zirself"}})
      const answer = editor:prompt("Fill the blank: " + field[2] + " Enter @abort to cancel.", field[3]);
      typeof(answer) == TYPE_STR || return false;
      input[field[1]] = answer;
    endfor
    let agreement = "";
    while (!(agreement in {"is", "are"}))
      const answer = editor:prompt("Choose: " + input["subject"] + " is ready, or "
        + input["subject"] + " are ready. Enter is or are, or @abort to cancel.", "is or are");
      typeof(answer) == TYPE_STR || return false;
      agreement = answer:trim();
    endwhile
    input["agreement"] = agreement;
    const examples = this:examples(input);
    editor:inform_current($event:mk_info(editor,
      @{$format.paragraph:mk(line) for line in (examples)}):with_audience('utility));
    return equal(editor:confirm(confirmation), true) ? input | false;
  endmethod

  method test_custom_input owner: HACKER
    "Custom editor fields and presets share validation, previews, and lossless editing.";
    let input = ["subject" -> " ze ", "object" -> "zir", "possessive_adjective" -> "zir",
      "possessive_pronoun" -> "zirs", "reflexive" -> "zirself", "agreement" -> "is"];
    const set = this:from_input(input);
    $test_utils:assert_true(equal(this:examples(input), {"Ze is ready.", "I met zir.",
      "This is zir coat.", "The coat is zirs.", "Ze did it zirself.", "Ze has a map."}),
      "preview covers all forms and agreement");
    $test_utils:assert_true(equal(this:from_input(this:to_input(set)), set), "custom edit roundtrip");
    for preset in (this:list_presets())
      $test_utils:assert_true(equal(this:from_input(preset), this:lookup(preset)), "preset uses same input path");
    endfor
    input["agreement"] = "are";
    $test_utils:assert_eq(this:examples(input)[1], "Ze are ready.", "explicit plural agreement");
    for invalid in ({"", " ", "two\nlines", "x":repeat(65)})
      input["subject"] = invalid;
      $test_utils:assert_raises(E_INVARG, this, "from_input", {input}, "reject invalid form");
    endfor
    $test_utils:assert_raises(E_INVARG, this, "from_input", {[]}, "all fields required");
    return true;
  endmethod

  method mk owner: HACKER
    "Create a custom set from five nonempty strings and an optional agreement boolean.";
    "The boolean selects plural verbs, including for singular they. It defaults to false.";
    const {ps, po, pp, pq, pr, ?is_plural = false} = args;
    typeof(is_plural) == TYPE_BOOL || raise(E_TYPE, "Pronoun agreement must be a boolean.");
    const verb_be = is_plural ? "are" | "is";
    const verb_have = is_plural ? "have" | "has";
    for word in ({ps, po, pp, pq, pr})
      typeof(word) == TYPE_STR || raise(E_TYPE, "Pronoun forms must be strings.");
    endfor
    const pronouns = <this, .display = ps + "/" + po, .ps = ps, .po = po, .pp = pp,
      .pq = pq, .pr = pr, .is_plural = is_plural, .verb_be = verb_be, .verb_have = verb_have>;
    return this:validate(pronouns);
  endmethod

  method validate owner: HACKER
    "Return a complete pronoun set, or reject malformed fields with E_TYPE or E_INVARG.";
    const {pronouns} = args;
    typeof(pronouns) == TYPE_FLYWEIGHT || raise(E_TYPE, "Expected a pronoun set.");
    pronouns.delegate == $pronouns || raise(E_INVARG, "Expected a $pronouns flyweight.");
    try
      for field in ({'ps, 'po, 'pp, 'pq, 'pr, 'display, 'verb_be, 'verb_have})
        const word = pronouns.(field);
        typeof(word) == TYPE_STR || raise(E_TYPE, "Pronoun " + tostr(field) + " must be a string.");
        length(word:trim()) || raise(E_INVARG, "Pronoun " + tostr(field) + " must not be empty.");
      endfor
      typeof(pronouns.is_plural) == TYPE_BOOL
        || raise(E_TYPE, "Pronoun agreement must be a boolean.");
      const be = pronouns.is_plural ? "are" | "is";
      const have = pronouns.is_plural ? "have" | "has";
      (strcmp(pronouns.verb_be, be) == 0 && strcmp(pronouns.verb_have, have) == 0)
        || raise(E_INVARG, "Pronoun verb forms disagree with is_plural.");
    except (E_PROPNF)
      raise(E_INVARG, "Pronoun set is missing a required field.");
    endtry
    return pronouns;
  endmethod

  method form owner: HACKER
    "Read subject, object, reflexive, or possessive ('adj or 'noun) from a pronoun set.";
    const {pronouns, form, @options} = args;
    const checked = this:validate(pronouns);
    if (form == 'possessive)
      length(options) == 1 || raise(E_INVARG, "Specify 'adj or 'noun for possessive pronouns.");
      options[1] in {'adj, 'noun} || raise(E_INVARG, "Expected possessive 'adj or 'noun.");
      return options[1] == 'adj ? checked.pp | checked.pq;
    endif
    !options || raise(E_INVARG, "This pronoun form takes no additional arguments.");
    const fields = ['subject -> 'ps, 'object -> 'po, 'reflexive -> 'pr];
    maphaskey(fields, form) || raise(E_INVARG, "Unknown pronoun form.");
    return checked.(fields[form]);
  endmethod

  method display owner: HACKER
    "Return the label of a validated preset or custom pronoun set.";
    const {pronouns} = args;
    return this:validate(pronouns).display;
  endmethod

  method presets owner: HACKER
    "Return the available preset sets in menu order.";
    return {this.he_him, this.she_her, this.they_them, this.it_its, this.spivak};
  endmethod

  method lookup owner: HACKER
    "Find a preset by its label, ignoring case and surrounding whitespace; otherwise return false.";
    const {search} = args;
    typeof(search) == TYPE_STR || raise(E_TYPE, "Pronoun preset name must be a string.");
    const label = search:trim();
    for preset in (this:presets())
      this:display(preset) == label && return preset;
    endfor
    return false;
  endmethod

  method list_presets owner: HACKER
    "Return the labels available to pronoun selectors.";
    return {this:display(preset) for preset in (this:presets())};
  endmethod

  method test_custom_sets owner: HACKER
    "Custom words remain literal and agreement is an explicit boolean.";
    const custom = this:mk("ze", "zir", "zir", "zirs", "zirself");
    $test_utils:assert_eq(this:display(custom), "ze/zir", "custom label");
    $test_utils:assert_eq({custom.verb_be, custom.verb_have}, {"is", "has"}, "singular agreement");
    const plural = this:mk("they", "them", "their", "theirs", "themself", true);
    $test_utils:assert_eq({plural.verb_be, plural.verb_have}, {"are", "have"}, "plural agreement");
    const literal = this:mk("{n}", "<them>", "their", "theirs", "themself");
    $test_utils:assert_eq(literal.ps, "{n}", "pronouns are literal words");
    for preset in (this:presets())
      $test_utils:assert_eq(this:lookup(this:display(preset)), preset, "preset roundtrip");
    endfor
    $test_utils:assert_eq(this:lookup(" THEY/THEM "), this.they_them, "preset normalization");
    return true;
  endmethod

  method test_invalid_sets owner: HACKER
    "Reject incomplete records, empty forms, and inconsistent agreement.";
    const words = {"ze", "zir", "zir", "zirs", "zirself"};
    for bad_flag in ({0, 1, "false", {}, #0})
      $test_utils:assert_raises(E_TYPE, this, "mk", {@words, bad_flag}, "boolean agreement required");
    endfor
    for index in [1..5]
      let invalid = words;
      invalid[index] = " ";
      $test_utils:assert_raises(E_INVARG, this, "mk", invalid, "nonempty forms required");
      invalid[index] = 'word;
      $test_utils:assert_raises(E_TYPE, this, "mk", invalid, "string forms required");
    endfor
    $test_utils:assert_raises(E_INVARG, this, "display", {<$pronouns, .ps = "ze">}, "incomplete set");
    $test_utils:assert_raises(E_INVARG, this, "display", {<$sub, .display = "ze/zir">}, "wrong delegate");
    const inconsistent = <this, .ps = "ze", .po = "zir", .pp = "zir", .pq = "zirs",
      .pr = "zirself", .display = "ze/zir", .is_plural = true, .verb_be = "is", .verb_have = "has">;
    $test_utils:assert_raises(E_INVARG, this, "validate", {inconsistent}, "inconsistent verbs");
    return true;
  endmethod

  method test_object_accessors owner: HACKER
    "Actors and things accept the same one-argument possessive selectors.";
    for parent in ({$actor, $thing})
      const object = parent:create(true);
      object.pronouns = this.she_her;
      $test_utils:assert_eq(object:pronoun_subject(), "she", "subject");
      $test_utils:assert_eq(object:pronoun_object(), "her", "object");
      $test_utils:assert_eq(object:pronoun_possessive('adj), "her", "possessive adjective");
      $test_utils:assert_eq(object:pronoun_possessive('noun), "hers", "possessive noun");
      $test_utils:assert_eq(object:pronoun_reflexive(), "herself", "reflexive");
      $test_utils:assert_raises(E_INVARG, object, "pronoun_possessive", {}, "missing selector");
      $test_utils:assert_raises(E_INVARG, object, "pronoun_possessive", {'bad}, "unknown selector");
      object.pronouns = <$pronouns, .ps = "she">;
      $test_utils:assert_raises(E_INVARG, object, "pronouns", {}, "bad stored set");
    endfor
    return true;
  endmethod
endobject
