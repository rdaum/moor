object SUB [
  import_export_id -> "sub",
  import_export_hierarchy -> {"events"}
]
  name: "Substitutions Delegate"
  parent: ROOT
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Flyweight delegate and factory for template substitution in events.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Message substitutions for programmers",
    "",
    "A substitution is an unresolved value inside an event. It selects words for each viewer.",
    "The event captures participant identities. Names and pronouns are read when it resolves.",
    "Rendering an already resolved document does not read these objects again.",
    "",
    "For the complete template syntax, see the object documentation on $sub_utils.",
    "",
    "## Constructing a message",
    "",
    "```moo",
    "const event = $event:mk_info(player,",
    "  $sub:nc(), \" \", $sub:agree('actor, 'name, \"wave\", \"waves\"), \" to \", $sub:d(), \".\")",
    "  :with_dobj(recipient);",
    "const document = event:resolve(viewer);",
    "const text = $document:render(document, 'text_plain)['body];",
    "```",
    "",
    "recipient and viewer are objects supplied by your program. No message is sent by this example.",
    "Resolution runs under the caller's authority. Choosing a viewer does not grant their permissions.",
    "",
    "## Names and pronouns",
    "",
    "n, d, i, t, and l select the actor, direct object, indirect object, this object, and location.",
    "Append c to capitalize: nc, dc, ic, tc, lc. Names become you when the object is the viewer.",
    "Names do not add articles. Use a('d), ac('d), the('d), or thec('d) for a name with an article.",
    "Articles need object bindings, and include the object's name exactly once.",
    "Only the abbreviations nc/dc/ic/tc/lc capitalize a name after its article.",
    "",
    "s, o, p, q, and r select subject, object, possessive adjective, possessive noun, and reflexive forms.",
    "They use the actor's chosen pronouns. Add c to capitalize, and _dobj or _iobj to select another role.",
    "Examples: sc(), p_dobj(), qc_iobj(). Pronouns become you/you/your/yours/yourself for the viewer.",
    "pronoun('recipient, 'subject) selects pronouns for any named object binding or standard role.",
    "Forms are 'subject, 'object, 'pos_adj, 'pos_noun, and 'reflexive. capitalize() works on these too.",
    "",
    "## Agreement and alternatives",
    "",
    "agree(binding, subject_form, plural_form, singular_form) selects literal wording for a subject.",
    "Use 'name when the sentence names its subject, or 'pronoun when it uses a subject pronoun.",
    "For example, agree('actor, 'name, \"are\", \"is\") gives Alex is and You are.",
    "With 'pronoun, a singular person using they/them takes are: They are.",
    "The binding can be any object role or named object binding, including 'this or 'companion.",
    "Noun number comes from noun_grammar(viewer)['number]; pronoun agreement comes from pronouns.is_plural.",
    "",
    "verb_be(), verb_have(), and verb_look() are pronoun-agreement shortcuts.",
    "Their _dobj and _iobj versions select those participants. Pair them with pronouns, not names.",
    "self_alt(for_self, for_others) selects by actor/viewer identity, without checking grammar.",
    "Branches can hold strings, lists, or structured inline content. Only the selected branch resolves.",
    "self_altc capitalizes its selected branch. capitalize(content) capitalizes any resolved inline content.",
    "",
    "## Named bindings and literal content",
    "",
    "binding(name) reads an object name or literal text; bindingc(name) capitalizes its result.",
    "content_binding(name) reads explicitly supplied inline content.",
    "Supply these through event:with_object_binding, with_text_binding, or with_content_binding.",
    "Missing named bindings raise E_INVARG. Missing optional object roles show a readable placeholder.",
    "Text bindings remain literal, including braces, HTML, and Djot. They are never compiled again.",
    "",
    "## Article selection",
    "",
    "Complete names and you omit articles. Indefinite plurals and mass nouns use some.",
    "Objects control phrases through noun_phrase() and agreement through noun_grammar().",
    "The string indefinite_article() helper handles spelling and exceptions such as hour and user.",
    "A thing's indefinite_article property can be \"a\" or \"an\" to override pronunciation.",
    "An empty property uses automatic selection. Pronunciation can depend on dialect.",
    "",
    "## Template editing",
    "",
    "$sub_utils:compile(text) converts authored template text to content.",
    "$sub_utils:decompile(content) preserves supported content for editing, including nested substitutions.",
    "It supports strings, $sub values, and lists. Other structured content raises an error.",
    "Use content_binding for formatted or computed content that must appear in an editable template."
  };

  method resolve_inline owner: ARCH_WIZARD
    "Resolve a standalone substitution with a capture local to this call.";
    set_task_perms(caller_perms());
    const {context} = args;
    return this:resolve_with_state(context, ['subjects -> []])[1];
  endmethod

  method resolve_with_state owner: ARCH_WIZARD
    "Resolve one perspective substitution to inline content under the caller's authority.";
    "Text bindings stay literal. Content bindings and self alternatives expand only their selected value.";
    set_task_perms(caller_perms());
    const {context, initial_state} = args;
    let state = initial_state;
    context['principal] == caller_perms() || raise(E_PERM, "Resolution principal mismatch");
    const viewer = context['viewer];
    const facts = context['event];
    const kind = this.type;
    const capitalize = `this.capitalize ! E_PROPNF => false';
    if (kind in {'binding, 'content_binding})
      const binding = $document_resolver:binding(context, this.binding_name);
      if (kind == 'content_binding)
        binding['kind] == 'content || raise(E_INVARG, "Binding is not inline content");
        return {$format:inlines({binding['value]}), state};
      elseif (binding['kind] == 'text)
        const text = binding['value];
        typeof(text) == TYPE_STR || raise(E_TYPE, "Text binding must contain a string");
        return {$format:literal_inline(capitalize ? text:capitalize() | text), state};
      endif
      binding['kind] == 'object || raise(E_INVARG, "Use content_binding for structured content");
    endif
    if (kind == 'capitalize)
      return {{$document_template:mk('capitalize, $format:inlines({this.content}))}, state};
    endif
    typeof(viewer) == TYPE_OBJ && valid(viewer)
      || raise(E_INVARG, "This substitution needs a viewer");
    if (kind == 'agree)
      const binding = $document_resolver:binding(context, this.binding_name);
      binding['kind] == 'object || raise(E_INVARG, "Agreement requires an object binding.");
      const target = binding['value];
      if (typeof(target) != TYPE_OBJ || !valid(target))
        return {$format:literal_inline("<no-" + tostr(this.binding_name) + ">"), state};
      endif
      let plural = target == viewer;
      if (!plural)
        const request = this.subject_form == 'name ? 'grammar | 'pronouns;
        const {answer, subjects} = $document_resolver:capture_subject(
          target, viewer, request, state['subjects]);
        state['subjects] = subjects;
        plural = this.subject_form == 'name ? answer['number] == 'plural | answer.is_plural;
      endif
      return {$format:literal_inline(plural ? this.plural_form | this.singular_form), state};
    endif
    if (kind == 'self_alt)
      const selected = facts['actor] == viewer ? this.for_self | this.for_others;
      const parts = $format:inlines({selected});
      return {capitalize ? {$document_template:mk('capitalize, parts)} | parts, state};
    endif
    let binding_name = kind;
    kind == 'this && (binding_name = 'this_obj);
    if (kind in {'binding, 'article_a, 'article_the, 'listing})
      binding_name = this.binding_name;
    endif
    if (kind in {'actor, 'location, 'this, 'dobj, 'iobj, 'binding, 'article_a, 'article_the, 'listing, 'reference})
      let target;
      if (kind == 'reference)
        target = this.object;
      else
        const binding = $document_resolver:binding(context, binding_name);
        binding['kind] == 'object || raise(E_INVARG, "Name substitution requires an object binding");
        target = binding['value];
      endif
      if (typeof(target) != TYPE_OBJ || !valid(target))
        return {$format:literal_inline("<no-" + tostr(binding_name) + ">"), state};
      endif
      const forms = ['article_a -> 'indefinite, 'article_the -> 'definite, 'listing -> 'listing];
      const form = kind == 'reference ? this.form | (maphaskey(forms, kind) ? forms[kind] | 'bare);
      const perspective = kind == 'reference && (`this.third_person ! E_PROPNF => false') ? $nothing | viewer;
      const {phrase, subjects} = $document_resolver:capture_subject(
        target, perspective, form, state['subjects]);
      state['subjects] = subjects;
      let label = phrase;
      if (`this.capitalize_binding ! E_PROPNF => false')
        const space = index(label, " ");
        if (space > 1 && label[1..space - 1] in {"a", "an", "the", "some"})
          label = label[1..space] + label[space + 1..$]:capitalize();
        else
          label = label:capitalize();
        endif
      endif
      capitalize && (label = label:capitalize());
      return {{$format.annotation:object(target, label)}, state};
    endif
    let subject = 'actor;
    let form = kind;
    const name = tostr(kind);
    if (kind == 'pronoun)
      subject = this.binding_name;
      form = this.form;
    elseif (index(name, "dobj_", 1) == 1)
      subject = 'dobj;
      form = tosym(name[6..$]);
    elseif (index(name, "iobj_", 1) == 1)
      subject = 'iobj;
      form = tosym(name[6..$]);
    endif
    const binding = $document_resolver:binding(context, subject);
    binding['kind] == 'object || raise(E_INVARG, "Pronouns require an object binding.");
    const target = binding['value];
    if (typeof(target) != TYPE_OBJ || !valid(target))
      return {$format:literal_inline("<no-" + tostr(subject) + ">"), state};
    endif
    const self_forms = ['subject -> "you", 'object -> "you", 'pos_adj -> "your",
      'pos_noun -> "yours", 'reflexive -> "yourself", 'verb_be -> "are",
      'verb_have -> "have", 'verb_look -> "look"];
    maphaskey(self_forms, form) || raise(E_INVARG, "Unknown substitution: " + name);
    let text = "";
    if (target == viewer)
      text = self_forms[form];
    else
      const {pronouns, subjects} = $document_resolver:capture_subject(
        target, viewer, 'pronouns, state['subjects]);
      state['subjects] = subjects;
      const fields = ['subject -> 'ps, 'object -> 'po, 'pos_adj -> 'pp,
        'pos_noun -> 'pq, 'reflexive -> 'pr, 'verb_be -> 'verb_be, 'verb_have -> 'verb_have];
      if (form == 'verb_look)
        text = pronouns.is_plural ? "look" | "looks";
      else
        text = pronouns.(fields[form]);
      endif
    endif
    return {$format:literal_inline(capitalize ? text:capitalize() | text), state};
  endmethod

  method reference owner: HACKER
    "Refer to an explicit object using its phrase verb when the document resolves.";
    const {object, ?form = 'definite} = args;
    typeof(object) == TYPE_OBJ || raise(E_TYPE, "A reference requires an object.");
    form in {'label, 'bare, 'definite, 'indefinite, 'listing}
      || raise(E_INVARG, "Unknown noun phrase form.");
    return <$sub, .type = 'reference, .object = object, .form = form>;
  endmethod

  method attribution owner: HACKER
    "Name an emote's subject in third person so its authored continuation keeps its meaning.";
    const {object} = args;
    typeof(object) == TYPE_OBJ || raise(E_TYPE);
    return <$sub, .type = 'reference, .object = object, .form = 'definite,
      .third_person = true, .capitalize = true>;
  endmethod

  method pronoun owner: HACKER
    "Select one pronoun form for any object role or named object binding.";
    const {binding_name, form} = args;
    (typeof(binding_name) == TYPE_SYM && typeof(form) == TYPE_SYM) || raise(E_TYPE);
    form in {'subject, 'object, 'pos_adj, 'pos_noun, 'reflexive}
      || raise(E_INVARG, "Pronoun form must be subject, object, pos_adj, pos_noun, or reflexive.");
    return <$sub, .type = 'pronoun, .binding_name = binding_name, .form = form>;
  endmethod

  method agree owner: HACKER
    "Choose plural or singular wording for a binding's name or subject pronoun.";
    "The viewer always takes the plural form, as in you are or you wave.";
    const {binding_name, subject_form, plural_form, singular_form} = args;
    (typeof(binding_name) == TYPE_SYM && typeof(subject_form) == TYPE_SYM
      && typeof(plural_form) == TYPE_STR && typeof(singular_form) == TYPE_STR) || raise(E_TYPE);
    subject_form in {'name, 'pronoun} || raise(E_INVARG, "Agreement needs 'name or 'pronoun.");
    (length(plural_form) && length(singular_form)) || raise(E_INVARG, "Both verb forms are required.");
    return <$sub, .type = 'agree, .binding_name = binding_name, .subject_form = subject_form,
      .plural_form = plural_form, .singular_form = singular_form>;
  endmethod

  method capitalize owner: HACKER
    "Capitalize the first character of resolved content while preserving its document structure.";
    const {content} = args;
    return <$sub, .type = 'capitalize, .content = content>;
  endmethod

  method content_binding owner: HACKER
    "Insert explicitly bound inline content. Text bindings cannot use this operation.";
    const {name} = args;
    typeof(name) == TYPE_SYM || raise(E_TYPE);
    return <$sub, .type = 'content_binding, .binding_name = name, .capitalize = false>;
  endmethod

  method phrase owner: HACKER
    {text, ?options = []} = args;
    typeof(text) != TYPE_STR && return "";
    strip_period = typeof(options) == TYPE_LIST && 'strip_period in options;
    initial_lowercase = typeof(options) == TYPE_LIST && 'initial_lowercase in options;
    strip_period && length(text) && text[$] == "." && (text = text[1..$ - 1]);
    initial_lowercase && length(text) && (text = text:initial_lowercase());
    return text;
  endmethod

  method "self_alt self_altc" owner: HACKER
    capitalize = verb[length(verb)] == "c";
    {for_self, for_alt} = args;
    return <this, .type = 'self_alt, .capitalize = capitalize, .for_self = for_self, .for_others = for_alt>;
  endmethod

  method "d*c" owner: HACKER
    capitalize = index(verb, "c") != 0;
    return <this, .type = 'dobj, .capitalize = capitalize>;
  endmethod

  method "i*c" owner: HACKER
    capitalize = index(verb, "c") != 0;
    return <this, .type = 'iobj, .capitalize = capitalize>;
  endmethod

  method "l*c" owner: HACKER
    capitalize = index(verb, "c") != 0;
    return <this, .type = 'location, .capitalize = capitalize>;
  endmethod

  method "n*c" owner: HACKER
    capitalize = index(verb, "c") != 0;
    return <this, .type = 'actor, .capitalize = capitalize>;
  endmethod

  method "o*c o*_dobj o*_iobj oc*_dobj oc*_iobj" owner: HACKER
    capitalize = index(verb, "c") != 0;
    type = verb:ends_with("_dobj") ? 'dobj_object | verb:ends_with("_iobj") ? 'iobj_object | 'object;
    return <this, .type = type, .capitalize = capitalize>;
  endmethod

  method "p*c p*_dobj p*_iobj pc*_dobj pc*_iobj" owner: HACKER
    capitalize = index(verb, "c") != 0;
    type = verb:ends_with("_dobj") ? 'dobj_pos_adj | verb:ends_with("_iobj") ? 'iobj_pos_adj | 'pos_adj;
    return <this, .type = type, .capitalize = capitalize>;
  endmethod

  method "q*c q*_dobj q*_iobj qc*_dobj qc*_iobj" owner: HACKER
    capitalize = index(verb, "c") != 0;
    type = verb:ends_with("_dobj") ? 'dobj_pos_noun | verb:ends_with("_iobj") ? 'iobj_pos_noun | 'pos_noun;
    return <this, .type = type, .capitalize = capitalize>;
  endmethod

  method "r*c r*_dobj r*_iobj rc*_dobj rc*_iobj" owner: HACKER
    capitalize = index(verb, "c") != 0;
    type = verb:ends_with("_dobj") ? 'dobj_reflexive | verb:ends_with("_iobj") ? 'iobj_reflexive | 'reflexive;
    return <this, .type = type, .capitalize = capitalize>;
  endmethod

  method "s*c s*_dobj s*_iobj sc*_dobj sc*_iobj" owner: HACKER
    capitalize = index(verb, "c") != 0;
    type = verb:ends_with("_dobj") ? 'dobj_subject | verb:ends_with("_iobj") ? 'iobj_subject | 'subject;
    return <this, .type = type, .capitalize = capitalize>;
  endmethod

  method "t*c*" owner: HACKER
    capitalize = index(verb, "c") != 0;
    return <this, .type = 'this, .capitalize = capitalize>;
  endmethod

  method "verb_be verb_be_dobj verb_be_iobj" owner: HACKER
    "Verb conjugation for 'be' (is/are).";
    type = verb:ends_with("_dobj") ? 'dobj_verb_be | verb:ends_with("_iobj") ? 'iobj_verb_be | 'verb_be;
    return <this, .type = type>;
  endmethod

  method "verb_have verb_have_dobj verb_have_iobj" owner: HACKER
    "Verb conjugation for 'have' (has/have).";
    type = verb:ends_with("_dobj") ? 'dobj_verb_have | verb:ends_with("_iobj") ? 'iobj_verb_have | 'verb_have;
    return <this, .type = type>;
  endmethod

  method "verb_look verb_look_dobj verb_look_iobj" owner: HACKER
    "Verb conjugation for 'look' (look/looks).";
    type = verb:ends_with("_dobj") ? 'dobj_verb_look | verb:ends_with("_iobj") ? 'iobj_verb_look | 'verb_look;
    return <this, .type = type>;
  endmethod

  method "binding*" owner: HACKER
    "Read a captured text or object binding from the event.";
    capitalize = index(verb, "c") != 0;
    {binding_name} = args;
    return <this, .type = 'binding, .binding_name = binding_name, .capitalize = capitalize>;
  endmethod

  method "a ac the thec" owner: HACKER
    "Insert an object name with an article; only nc/dc/ic/tc/lc capitalize the binding.";
    const {name} = args;
    typeof(name) == TYPE_SYM || raise(E_TYPE, "Article binding must be a symbol.");
    const capitalize_article = verb in {"ac", "thec"};
    const capitalize_binding = name in {'nc, 'dc, 'ic, 'tc, 'lc};
    const binding_name = capitalize_binding ? tosym(tostr(name)[1]) | name;
    const type = verb in {"a", "ac"} ? 'article_a | 'article_the;
    return <this, .type = type, .binding_name = binding_name,
      .capitalize = capitalize_article, .capitalize_binding = capitalize_binding>;
  endmethod

  method "listing listingc" owner: HACKER
    "Request an object's preferred listing phrase for a named binding.";
    const {name, ?capitalize = verb == "listingc"} = args;
    return <$sub, .type = 'listing, .binding_name = name, .capitalize = capitalize>;
  endmethod

  method test_articles_and_named_bindings owner: HACKER
    "Article tokens retain exact capitalization and complete custom binding names.";
    const item = $thing:create(true);
    item.name = "apple";
    for example in ({{"{a d}", "an apple"}, {"{A d}", "An apple"},
      {"{an d}", "an apple"}, {"{An d}", "An apple"}, {"{The d}", "The apple"},
      {"{the dc}", "the Apple"}, {"{the relic}", "the apple"}, {"{The relic}", "The apple"}})
      const content = $sub_utils:compile(example[1]);
      const event = $event:mk_test(player, @content):with_dobj(item):with_object_binding('relic, item);
      const actual = $document:render(event:resolve(player), 'text_plain)['body];
      $test_utils:assert_eq(strcmp(actual, example[2]), 0, example[1]);
    endfor
    return true;
  endmethod

  method test_article_noun_grammar owner: HACKER
    "Mass nouns, plurals, proper names, and pronunciation overrides affect articles.";
    const item = $thing:create(true);
    const event = $event:mk_test(player, this:a('d), " / ", this:the('d)):with_dobj(item);
    for example in ({{"water", false, false, false, "", "some water / the water"},
      {"boots", true, true, false, "", "some boots / the boots"},
      {"Henri", true, false, true, "", "Henri / Henri"},
      {"MRI scanner", true, false, false, "an", "an MRI scanner / the MRI scanner"},
      {"herb", true, false, false, "a", "a herb / the herb"}})
      item.name = example[1];
      item.noun_kind = example[4] ? 'name | (example[2] ? 'count | 'mass);
      item.noun_number = example[3] ? 'plural | 'singular;
      item.indefinite_article = example[5];
      const actual = $document:render(event:resolve(player), 'text_plain)['body];
      $test_utils:assert_eq(strcmp(actual, example[6]), 0, item.name);
      $test_utils:assert_eq($document:render(event:resolve(item), 'text_plain)['body],
        "you / you", "viewer has no articles");
    endfor
    for example in ({{"hour", "an"}, {"honest answer", "an"}, {"user", "a"},
      {"unidentified object", "an"}, {"unusual object", "an"}, {"unicycle", "a"}})
      $test_utils:assert_eq(example[1]:indefinite_article(), example[2], example[1]);
    endfor
    item.indefinite_article = "some";
    $test_utils:assert_raises(E_INVARG, event, "resolve", {player}, "invalid override");
    return true;
  endmethod

  method test_a_or_an_vowels owner: HACKER
    "Test a_or_an() returns 'an' for vowel starters.";
    $test_utils:assert_eq("apple":indefinite_article(), "an", "apple article");
    $test_utils:assert_eq("egg":indefinite_article(), "an", "egg article");
    $test_utils:assert_eq("igloo":indefinite_article(), "an", "igloo article");
    $test_utils:assert_eq("orange":indefinite_article(), "an", "orange article");
    $test_utils:assert_eq("umbrella":indefinite_article(), "an", "umbrella article");
    return true;
  endmethod

  method test_a_or_an_consonants owner: HACKER
    "Test a_or_an() returns 'a' for consonant starters.";
    $test_utils:assert_eq("banana":indefinite_article(), "a", "banana article");
    $test_utils:assert_eq("cat":indefinite_article(), "a", "cat article");
    $test_utils:assert_eq("dog":indefinite_article(), "a", "dog article");
    return true;
  endmethod

  method test_a_or_an_u_silent owner: HACKER
    "Test a_or_an() with 'u' words that have silent 'u' sound.";
    $test_utils:assert_eq("unicycle":indefinite_article(), "a", "unicycle article");
    $test_utils:assert_eq("union":indefinite_article(), "a", "union article");
    $test_utils:assert_eq("university":indefinite_article(), "a", "university article");
    $test_utils:assert_eq("unit":indefinite_article(), "a", "unit article");
    return true;
  endmethod

  method test_a_or_an_u_pronounced owner: HACKER
    "Test a_or_an() with 'u' words that have pronounced 'u' sound.";
    $test_utils:assert_eq("ukulele":indefinite_article(), "an", "ukulele article");
    return true;
  endmethod

  method test_a_or_an_edge_cases owner: HACKER
    "Test a_or_an() with edge cases.";
    $test_utils:assert_eq("":indefinite_article(), "a", "empty article");
    $test_utils:assert_eq("x":indefinite_article(), "a", "single consonant article");
    $test_utils:assert_eq("a":indefinite_article(), "an", "single vowel article");
    return true;
  endmethod

  method test_article_a_creation owner: HACKER
    "Test a() article flyweight creation.";
    fw = this:a('test_binding);
    $test_utils:assert_type(fw, TYPE_FLYWEIGHT, "a() should return flyweight");
    $test_utils:assert_eq(fw.type, 'article_a, "a() type");
    $test_utils:assert_eq(fw.binding_name, 'test_binding, "a() binding");
    $test_utils:assert_false(fw.capitalize, "a() should not capitalize");
    return true;
  endmethod

  method test_article_ac_creation owner: HACKER
    "Test ac() capitalized article flyweight.";
    fwc = this:ac('test_binding);
    $test_utils:assert_eq(fwc.type, 'article_a, "ac() type");
    $test_utils:assert_true(fwc.capitalize, "ac() should capitalize");
    return true;
  endmethod

  method test_article_the_creation owner: HACKER
    "Test the() article flyweight creation.";
    fw = this:the('test_binding);
    $test_utils:assert_type(fw, TYPE_FLYWEIGHT, "the() should return flyweight");
    $test_utils:assert_eq(fw.type, 'article_the, "the() type");
    $test_utils:assert_eq(fw.binding_name, 'test_binding, "the() binding");
    $test_utils:assert_false(fw.capitalize, "the() should not capitalize");
    $test_utils:assert_false(`fw.capitalize_binding ! E_PROPNF => false', "the() should not set capitalize_binding");
    return true;
  endmethod

  method test_article_thec_creation owner: HACKER
    "Test thec() capitalized article flyweight.";
    fwc = this:thec('test_binding);
    $test_utils:assert_eq(fwc.type, 'article_the, "thec() type");
    $test_utils:assert_true(fwc.capitalize, "thec() should capitalize");
    $test_utils:assert_false(`fwc.capitalize_binding ! E_PROPNF => false', "thec() should not set capitalize_binding");
    return true;
  endmethod

  method test_article_the_eval owner: HACKER
    "Resolve definite articles through the document pipeline.";
    const event = $event:mk_test(this, this:the('d)):with_dobj(this);
    const result = $document:render(event:resolve(#0), 'text_plain)['body];
    $test_utils:assert_eq(result, this.name, "Complete name has no added article");
    return true;
  endmethod

  method test_article_a_eval owner: HACKER
    "Resolve indefinite articles through the document pipeline.";
    const item = $thing:create(true);
    item.name = "apple";
    const event = $event:mk_test(player, this:a('d)):with_dobj(item);
    const result = $document:render(event:resolve(player), 'text_plain)['body];
    $test_utils:assert_eq(result, "an apple", "Indefinite article");
    return true;
  endmethod

  method test_article_with_capitalized_binding owner: HACKER
    "A capitalized binding changes the noun without capitalizing its article.";
    const event = $event:mk_test(this, this:the('dc)):with_dobj(this);
    const result = $document:render(event:resolve(#0), 'text_plain)['body];
    $test_utils:assert_eq(result, this.name:capitalize(), "Capitalized complete name");
    return true;
  endmethod

  method test_unicode_capitalized_substitutions owner: HACKER
    "Name, text binding, and selected-branch capitalization share Unicode case conversion.";
    const actor = $thing:create(true);
    actor.name = "éloïse";
    const event = $event:mk_test(actor, this:nc(), " / ", this:bindingc('word), " / ",
      this:self_altc("you", {this:n()})):with_text_binding('word, "σάρα");
    const rendered = $document:render(event:resolve(player), 'text_plain)['body];
    $test_utils:assert_eq(strcmp(rendered, "Éloïse / Σάρα / Éloïse"), 0, "Unicode substitutions");
    return true;
  endmethod

  method test_binding_creation owner: HACKER
    "Test binding() flyweight creation.";
    fw = this:binding('test_name);
    $test_utils:assert_type(fw, TYPE_FLYWEIGHT, "binding() should return flyweight");
    $test_utils:assert_eq(fw.type, 'binding, "binding() type");
    $test_utils:assert_eq(fw.binding_name, 'test_name, "binding() name");
    return true;
  endmethod

  method test_bindingc_creation owner: HACKER
    "Test bindingc() capitalized binding flyweight.";
    fwc = this:bindingc('test_name);
    $test_utils:assert_eq(fwc.type, 'binding, "bindingc() type");
    $test_utils:assert_true(fwc.capitalize, "bindingc() should capitalize");
    return true;
  endmethod

  method test_self_alt_creation owner: HACKER
    "Test self_alt() flyweight creation.";
    fw = this:self_alt("for_self", "for_others");
    $test_utils:assert_type(fw, TYPE_FLYWEIGHT, "self_alt() should return flyweight");
    $test_utils:assert_eq(fw.type, 'self_alt, "self_alt() type");
    $test_utils:assert_eq(fw.for_self, "for_self", "self_alt() self text");
    $test_utils:assert_eq(fw.for_others, "for_others", "self_alt() others text");
    $test_utils:assert_false(fw.capitalize, "self_alt() should not capitalize");
    return true;
  endmethod

  method test_self_altc_creation owner: HACKER
    "Test self_altc() capitalized self-alt flyweight.";
    fwc = this:self_altc("For_self", "For_others");
    $test_utils:assert_eq(fwc.type, 'self_alt, "self_altc() type");
    $test_utils:assert_true(fwc.capitalize, "self_altc() should capitalize");
    return true;
  endmethod

  method test_name_subs_actor owner: HACKER
    "Test n() and nc() actor name substitutions.";
    fw = this:n();
    $test_utils:assert_eq(fw.type, 'actor, "n() type");
    $test_utils:assert_false(fw.capitalize, "n() should not capitalize");
    fwc = this:nc();
    $test_utils:assert_eq(fwc.type, 'actor, "nc() type");
    $test_utils:assert_true(fwc.capitalize, "nc() should capitalize");
    return true;
  endmethod

  method test_name_subs_objects owner: HACKER
    "Test d/i/l/t object name substitutions.";
    fwd = this:d();
    $test_utils:assert_eq(fwd.type, 'dobj, "d() type");
    fwi = this:i();
    $test_utils:assert_eq(fwi.type, 'iobj, "i() type");
    fwl = this:l();
    $test_utils:assert_eq(fwl.type, 'location, "l() type");
    fwt = this:t();
    $test_utils:assert_eq(fwt.type, 'this, "t() type");
    return true;
  endmethod

  method test_pronouns_actor owner: HACKER
    "Test actor pronoun substitutions.";
    fw_s = this:s();
    $test_utils:assert_eq(fw_s.type, 'subject, "s() type");
    fw_sc = this:sc();
    $test_utils:assert_eq(fw_sc.type, 'subject, "sc() type");
    $test_utils:assert_true(fw_sc.capitalize, "sc() should capitalize");
    fw_o = this:o();
    $test_utils:assert_eq(fw_o.type, 'object, "o() type");
    fw_p = this:p();
    $test_utils:assert_eq(fw_p.type, 'pos_adj, "p() type");
    fw_q = this:q();
    $test_utils:assert_eq(fw_q.type, 'pos_noun, "q() type");
    fw_r = this:r();
    $test_utils:assert_eq(fw_r.type, 'reflexive, "r() type");
    return true;
  endmethod

  method test_pronouns_dobj owner: HACKER
    "Test direct object pronoun substitutions.";
    fw_s = this:s_dobj();
    $test_utils:assert_eq(fw_s.type, 'dobj_subject, "s_dobj() type");
    fw_o = this:o_dobj();
    $test_utils:assert_eq(fw_o.type, 'dobj_object, "o_dobj() type");
    fw_p = this:p_dobj();
    $test_utils:assert_eq(fw_p.type, 'dobj_pos_adj, "p_dobj() type");
    fw_q = this:q_dobj();
    $test_utils:assert_eq(fw_q.type, 'dobj_pos_noun, "q_dobj() type");
    fw_r = this:r_dobj();
    $test_utils:assert_eq(fw_r.type, 'dobj_reflexive, "r_dobj() type");
    return true;
  endmethod

  method test_pronouns_iobj owner: HACKER
    "Test indirect object pronoun substitutions.";
    fw_s = this:s_iobj();
    $test_utils:assert_eq(fw_s.type, 'iobj_subject, "s_iobj() type");
    fw_o = this:o_iobj();
    $test_utils:assert_eq(fw_o.type, 'iobj_object, "o_iobj() type");
    fw_p = this:p_iobj();
    $test_utils:assert_eq(fw_p.type, 'iobj_pos_adj, "p_iobj() type");
    fw_q = this:q_iobj();
    $test_utils:assert_eq(fw_q.type, 'iobj_pos_noun, "q_iobj() type");
    fw_r = this:r_iobj();
    $test_utils:assert_eq(fw_r.type, 'iobj_reflexive, "r_iobj() type");
    return true;
  endmethod

  method test_explicit_subject_agreement owner: HACKER
    "Singular they, noun plurality, and the viewer select agreement independently.";
    const actor = $thing:create(true);
    actor.name = "alex";
    actor.pronouns = $pronouns.they_them;
    for example in ({{"{nc} {agree actor name:are|is} ready.", "Alex is ready.", "You are ready."},
      {"{sc} {agree actor pronoun:wave|waves}.", "They wave.", "You wave."},
      {"{sc} {be} ready.", "They are ready.", "You are ready."}})
      const content = $sub_utils:compile(example[1]);
      for version in ({content, $sub_utils:compile($sub_utils:decompile(content))})
        const event = $event:mk_test(actor, @version);
        $test_utils:assert_eq(strcmp($document:render(event:resolve(player), 'text_plain)['body],
          example[2]), 0, "observer: " + example[1]);
        $test_utils:assert_eq(strcmp($document:render(event:resolve(actor), 'text_plain)['body],
          example[3]), 0, "actor: " + example[1]);
      endfor
    endfor
    actor.pronouns = $pronouns.he_him;
    const singular = $event:mk_test(actor, this:sc(), " ", this:agree('actor, 'pronoun, "wave", "waves"));
    $test_utils:assert_eq($document:render(singular:resolve(player), 'text_plain)['body],
      "He waves", "singular pronoun agreement");
    const boots = $thing:create(true);
    boots.name = "boots";
    boots.noun_number = 'plural;
    for role in ({'dobj, 'iobj, 'this, 'location, 'companion})
      const content = {this:bindingc(role), " ", this:agree(role, 'name, "are", "is"), " / ",
        this:agree(role, 'pronoun, "are", "is")};
      const event = $event:mk_test(actor, @content):with_dobj(boots):with_iobj(boots)
        :with_this(boots):with_location(boots):with_object_binding('companion, boots);
      $test_utils:assert_eq($document:render(event:resolve(player), 'text_plain)['body],
        "Boots are / is", "name number differs from it/its for " + tostr(role));
      $test_utils:assert_eq($document:render(event:resolve(boots), 'text_plain)['body],
        "You are / are", "agreement follows subject viewer for " + tostr(role));
    endfor
    return true;
  endmethod

  method test_named_pronouns owner: HACKER
    "Named participants use their own pronouns and the reader's perspective after editing.";
    const actor = $thing:create(true);
    actor.pronouns = $pronouns.he_him;
    const recipient = $thing:create(true);
    recipient.pronouns = $pronouns:mk("ze", "zir", "zir", "zirs", "zirself");
    for role in ({'recipient, 'this, 'location, 'dobj, 'iobj})
      let content = {};
      for form in ({'subject, 'object, 'pos_adj, 'pos_noun, 'reflexive})
        content = {@content, this:pronoun(role, form), " / "};
      endfor
      content = {this:capitalize(content[1..$ - 1])};
      for version in ({content, $sub_utils:compile($sub_utils:decompile(content))})
        const event = $event:mk_test(actor, @version):with_object_binding('recipient, recipient)
          :with_this(recipient):with_location(recipient):with_dobj(recipient):with_iobj(recipient);
        for viewer in ({actor, recipient, player})
          const expected = viewer == recipient ? "You / you / your / yours / yourself"
            | "Ze / zir / zir / zirs / zirself";
          const document = event:resolve(viewer);
          $test_utils:assert_true(equal($document:render(document, 'text_plain)['body], expected),
            "named pronouns: " + tostr(role));
          $test_utils:assert_true(equal($document:render(document, 'text_html)['body],
            "<p>" + expected + "</p>"), "HTML uses the same resolved pronouns");
        endfor
      endfor
    endfor
    const missing = $event:mk_test(actor, this:pronoun('missing, 'subject));
    $test_utils:assert_raises(E_INVARG, missing, "resolve", {actor}, "missing named participant");
    const text = missing:with_text_binding('missing, "Alex");
    $test_utils:assert_raises(E_INVARG, text, "resolve", {actor}, "text has no pronouns");
    for template in ({"{pronoun recipient}", "{pronoun recipient invalid}",
      "{pronoun recipient subject extra}"})
      $test_utils:assert_raises(E_INVARG, $sub_utils, "compile", {template}, template);
    endfor
    return true;
  endmethod

  method test_agreement_errors owner: HACKER
    "Invalid subject forms and non-object bindings produce authoring errors.";
    $test_utils:assert_raises(E_INVARG, this, "agree", {'actor, 'bad, "are", "is"}, "unknown subject form");
    $test_utils:assert_raises(E_INVARG, this, "agree", {'actor, 'name, "", "is"}, "empty verb form");
    const text = $event:mk_test(player, this:agree('word, 'name, "are", "is"))
      :with_text_binding('word, "boots");
    $test_utils:assert_raises(E_INVARG, text, "resolve", {player}, "agreement needs object grammar");
    const missing = $event:mk_test(player, this:agree('missing, 'name, "are", "is"));
    $test_utils:assert_raises(E_INVARG, missing, "resolve", {player}, "unknown binding");
    return true;
  endmethod

  method test_furniture_subject_agreement owner: HACKER
    "Furniture messages agree with the displaced occupant's name, independently of the actor.";
    const actor = $thing:create(true);
    actor.pronouns = $pronouns.they_them;
    const occupant = $thing:create(true);
    occupant.name = "Sam";
    occupant.noun_kind = 'name;
    occupant.pronouns = $pronouns.they_them;
    const chair = $sittable:create(true);
    chair.name = "chair";
    for example in ({{chair.dumped_msg, "dumped"}, {chair.squeezed_msg, "squeezed"}})
      const event = $event:mk_test(actor, @example[1]):with_dobj(occupant):with_this(chair);
      $test_utils:assert_eq($document:render(event:resolve(player), 'text_plain)['body],
        "Sam is " + example[2] + " off the chair.", "observer sees name agreement");
      $test_utils:assert_eq($document:render(event:resolve(occupant), 'text_plain)['body],
        "You are " + example[2] + " off the chair.", "occupant sees own agreement");
    endfor
    return true;
  endmethod

  method test_verb_conjugation_be owner: HACKER
    "Test verb_be() conjugation flyweights.";
    fw_be = this:verb_be();
    $test_utils:assert_eq(fw_be.type, 'verb_be, "verb_be() type");
    fw_be_d = this:verb_be_dobj();
    $test_utils:assert_eq(fw_be_d.type, 'dobj_verb_be, "verb_be_dobj() type");
    fw_be_i = this:verb_be_iobj();
    $test_utils:assert_eq(fw_be_i.type, 'iobj_verb_be, "verb_be_iobj() type");
    return true;
  endmethod

  method test_verb_conjugation_have owner: HACKER
    "Test verb_have() conjugation flyweights.";
    fw_have = this:verb_have();
    $test_utils:assert_eq(fw_have.type, 'verb_have, "verb_have() type");
    fw_have_d = this:verb_have_dobj();
    $test_utils:assert_eq(fw_have_d.type, 'dobj_verb_have, "verb_have_dobj() type");
    fw_have_i = this:verb_have_iobj();
    $test_utils:assert_eq(fw_have_i.type, 'iobj_verb_have, "verb_have_iobj() type");
    return true;
  endmethod

  method test_verb_conjugation_look owner: HACKER
    "Test verb_look() conjugation flyweights.";
    fw_look = this:verb_look();
    $test_utils:assert_eq(fw_look.type, 'verb_look, "verb_look() type");
    fw_look_d = this:verb_look_dobj();
    $test_utils:assert_eq(fw_look_d.type, 'dobj_verb_look, "verb_look_dobj() type");
    fw_look_i = this:verb_look_iobj();
    $test_utils:assert_eq(fw_look_i.type, 'iobj_verb_look, "verb_look_iobj() type");
    return true;
  endmethod

  method test_render_actor_perspective owner: HACKER
    "Resolve actor and observer wording before either document is serialized.";
    const event = $event:mk_social(player, this:nc(), " ", this:self_alt("wave", "waves"),
      " to ", this:the('d), "."):with_dobj(this);
    const own = $document:render(event:resolve(player), 'text_plain)['body];
    const observer = $document:render(event:resolve(#0), 'text_plain)['body];
    $test_utils:assert_eq(own, "You wave to " + this.name + ".", "Actor perspective");
    $test_utils:assert_eq(observer, player:name() + " waves to " + this.name + ".", "Observer perspective");
    return true;
  endmethod

  method test_eval_missing_object_bindings owner: HACKER
    "Missing optional participants produce readable placeholders without interaction descriptors.";
    for example in ({{this:s_dobj(), "<no-dobj>"}, {this:o_iobj(), "<no-iobj>"},
      {this:verb_have_dobj(), "<no-dobj>"}, {this:verb_look_iobj(), "<no-iobj>"}})
      const event = $event:mk_test(player, example[1]);
      $test_utils:assert_eq($document:render(event:resolve(player), 'text_plain)['body],
        example[2], "Absent participant");
    endfor
    return true;
  endmethod

  method test_binding_eval_and_missing_binding owner: HACKER
    "Object bindings preserve perspective, while an unknown binding is an explicit authoring error.";
    const event = $event:mk_test(player, this:binding('actor), " / ", this:binding('d)):with_dobj(this);
    $test_utils:assert_eq($document:render(event:resolve(player), 'text_plain)['body],
      "you / " + this:name(), "Object binding labels");
    const invalid = $event:mk_test(player, this:binding('missing));
    $test_utils:assert_raises(E_INVARG, invalid, "resolve", {player}, "Unknown binding");
    return true;
  endmethod

  method test_article_eval_self_and_proper_noun owner: HACKER
    "Articles disappear for the viewer and for proper nouns; anonymous objects retain readable labels.";
    const target = $test_utils:anonymous($thing);
    target.name = "test relic";
    const self_event = $event:mk_test(player, this:a('d)):with_dobj(target);
    $test_utils:assert_eq($document:render(self_event:resolve(target), 'text_plain)['body],
      "you", "Self has no article");
    target.noun_kind = 'name;
    for substitution in ({this:the('d), this:a('d)})
      const event = $event:mk_test(player, substitution):with_dobj(target);
      const result = $document:render(event:resolve(player), 'text_html);
      $test_utils:assert_eq(result['body], "<p>test relic</p>", "Proper noun has no article");
      $test_utils:assert_eq(result['annotations], [], "Anonymous identity has no client address");
    endfor
    return true;
  endmethod

  method test_phrase_strip_period owner: HACKER
    "Test phrase() with strip_period option.";
    result = this:phrase("Hello world.", {'strip_period});
    $test_utils:assert_eq(result, "Hello world", "strip_period phrase");
    return true;
  endmethod

  method test_phrase_initial_lowercase owner: HACKER
    "Test phrase() with initial_lowercase option.";
    result = this:phrase("Hello world", {'initial_lowercase});
    $test_utils:assert_eq(result, "hello world", "initial_lowercase phrase");
    return true;
  endmethod

  method test_phrase_both_options owner: HACKER
    "Test phrase() with both strip_period and initial_lowercase.";
    result = this:phrase("Hello world.", {'strip_period, 'initial_lowercase});
    $test_utils:assert_eq(result, "hello world", "combined phrase options");
    return true;
  endmethod

  method test_phrase_no_options owner: HACKER
    "Test phrase() with no options returns unchanged text.";
    result = this:phrase("Hello world.");
    $test_utils:assert_eq(result, "Hello world.", "phrase without options");
    return true;
  endmethod

  method test_phrase_empty_string owner: HACKER
    "Test phrase() with empty string returns empty.";
    result = this:phrase("");
    $test_utils:assert_eq(result, "", "empty phrase");
    return true;
  endmethod
endobject
