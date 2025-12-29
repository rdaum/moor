object GENDER_UTILS
  name: "gender utilities"
  parent: GENERIC_UTILS
  owner: HACKER
  readable: true

  property be (owner: HACKER, flags: "rc") = {"is", "is", "is", "is", "is", "is", "are", "am", "are", "are", "are"};
  property genders (owner: HACKER, flags: "rc") = {
    "neuter",
    "male",
    "female",
    "either",
    "Spivak",
    "splat",
    "plural",
    "egotistical",
    "royal",
    "2nd"
  };
  property have (owner: HACKER, flags: "rc") = {
    "has",
    "has",
    "has",
    "has",
    "has",
    "has",
    "have",
    "have",
    "have",
    "have",
    "have"
  };
  property is_plural (owner: HACKER, flags: "rc") = {0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1};
  property po (owner: HACKER, flags: "rc") = {"it", "him", "her", "him/her", "em", "h*", "them", "me", "us", "you"};
  property poc (owner: HACKER, flags: "rc") = {"It", "Him", "Her", "Him/Her", "Em", "H*", "Them", "Me", "Us", "You"};
  property pp (owner: HACKER, flags: "rc") = {"its", "his", "her", "his/her", "eir", "h*", "their", "my", "our", "your"};
  property ppc (owner: HACKER, flags: "rc") = {"Its", "His", "Her", "His/Her", "Eir", "H*", "Their", "My", "Our", "Your"};
  property pq (owner: HACKER, flags: "rc") = {
    "its",
    "his",
    "hers",
    "his/hers",
    "eirs",
    "h*s",
    "theirs",
    "mine",
    "ours",
    "yours"
  };
  property pqc (owner: HACKER, flags: "rc") = {
    "Its",
    "His",
    "Hers",
    "His/Hers",
    "Eirs",
    "H*s",
    "Theirs",
    "Mine",
    "Ours",
    "Yours"
  };
  property pr (owner: HACKER, flags: "rc") = {
    "itself",
    "himself",
    "herself",
    "(him/her)self",
    "emself",
    "h*self",
    "themselves",
    "myself",
    "ourselves",
    "yourself"
  };
  property prc (owner: HACKER, flags: "rc") = {
    "Itself",
    "Himself",
    "Herself",
    "(Him/Her)self",
    "Emself",
    "H*self",
    "Themselves",
    "Myself",
    "Ourselves",
    "Yourself"
  };
  property pronouns (owner: HACKER, flags: "rc") = {"ps", "po", "pp", "pq", "pr", "psc", "poc", "ppc", "pqc", "prc"};
  property ps (owner: HACKER, flags: "rc") = {"it", "he", "she", "s/he", "e", "*e", "they", "I", "we", "you"};
  property psc (owner: HACKER, flags: "rc") = {"It", "He", "She", "S/He", "E", "*E", "They", "I", "We", "You"};

  override aliases = {"Gender_Utilities"};
  override description = {
    "This is the gender utilities utility package.  See `help $gender_utils' for more details."
  };
  override help_msg = {
    "Defines the list of standard genders, the default pronouns for each, and routines for adding or setting pronoun properties on any gendered object.",
    "",
    "Properties:",
    "  .genders  -- list of standard genders",
    "  .pronouns -- list of pronoun properties",
    "  .ps .po .pp .pq .pr .psc .poc .ppc .pqc .prc ",
    "            -- lists of pronouns for each of the standard genders",
    "",
    "  If foo is of gender this.gender[n], ",
    "  then the default pronoun foo.p is this.p[n] ",
    "  (where p is one of ps/po/pp/pq...)",
    "",
    "Verbs:",
    "  :set(object,newgender) -- changes pronoun properties to match new gender.",
    "  :add(object[,perms[,owner]]) -- adds pronoun properties to object.",
    "",
    "  :get_pronoun     (which,object) -- return pronoun for a given object",
    "  :get_conj*ugation(verbspec,object) -- return appropriately conjugated verb"
  };
  override import_export_id = "gender_utils";
  override object_size = {12822, 1084848672};

  verb set (this none this) owner: #2 flags: "rxd"
    "$gender_utils:set(object,gender) --- sets the pronoun properties of object.";
    "gender is a string: one of the strings in $gender_utils.genders, the list of rcognized genders.  If the gender change is successful, the (full) name of the gender (e.g., \"male\") is returned.  E_NONE is returned if gender does not match any recognized gender.  Any other error encountered (e.g., E_PERM, E_PROPNF) is likewise returned and the object's pronoun properties are left unaltered.";
    set_task_perms(caller_perms());
    {object, gender} = args;
    if (this == object)
      return E_DIV;
    elseif (gnum = $string_utils:find_prefix(gender, this.genders))
      gender = this.genders[gnum];
    else
      return E_NONE;
    endif
    save = {};
    prons = this.pronouns;
    for p in (prons)
      save = {@save, e = `object.(p) ! ANY'};
      if (typeof(e) != TYPE_STR || typeof(e = `object.(p) = this.(p)[gnum] ! ANY') == TYPE_ERR)
        for i in [1..length(save) - 1]
          object.((prons[i])) = save[i];
        endfor
        return e;
      endif
    endfor
    return gender;
  endverb

  verb add (this none this) owner: #2 flags: "rxd"
    "$gender_utils:add(object[,perms[,owner]])";
    "--- adds pronoun properties to object if they're not already there.";
    "    perms default to \"rc\", owner defaults to the object owner.";
    set_task_perms(caller_perms());
    {object, ?perms = "rc", ?owner = object.owner} = args;
    prons = this.pronouns;
    e = 1;
    for p in (prons)
      if (!$object_utils:has_property(object, p))
        e = `add_property(object, p, "", {owner, perms}) ! ANY';
        if (typeof(e) == TYPE_ERR)
          player:tell("Couldn't add ", object, ".", p, ":  ", e);
          return;
        endif
      elseif (typeof(object.(p)) != TYPE_STR && typeof(e = `object.(p) = "" ! ANY') == TYPE_ERR)
        player:tell("Couldn't reset ", object, ".", p, ":  ", e);
        return;
      elseif (!object.(p))
        e = 0;
      endif
    endfor
    if (!e && TYPE_ERR == typeof(e = this:set(object, "neuter")))
      player:tell("Couldn't initialize pronouns:  ", e);
    endif
  endverb

  verb get_pronoun (this none this) owner: HACKER flags: "rxd"
    "get_pronoun(key,object) => pronoun corresponding to object.";
    "key can be one of s,o,p,q,r,S,O,P,Q,R to refer to the pronoun properties relatively directly or it can be something of the form \"he/she\" or \"He/She\".";
    "Next the object is checked for the desired pronoun property.  If that doesn't exist, we look at object.gender and infer the pronoun from the corresponding $gender_utils property.  If .gender doesn't exist or the object itself is invalid, we use the corresponding property on $player.";
    {key, ?object = player} = args;
    if (key[1] == ":")
      key = key[2..$];
    endif
    if (length(key) == 1 && (i = index("sopqrSOPQR", key, 1)))
      prop = this.pronouns[i];
    else
      search = "$1:he$s:she$1:he/she$2:him$2:him/her$3:his/her$4:hers$4:his/hers$5:himself$5:herself$5:himself/herself";
      i = index(search, ":" + key + "$");
      if (!i)
        return "";
      endif
      cap = strcmp("a", key) > 0 ? 1 | 0;
      prop = this.pronouns[toint(search[i - 1]) + 5 * cap];
    endif
    if (!valid(object))
      return $player.(prop);
    elseif (TYPE_STR == typeof(p = `object.(prop) ! ANY'))
      return p;
    elseif (TYPE_STR == typeof(g = `object.gender ! ANY') && (i = g in this.genders))
      return this.(prop)[i];
    else
      return $player.(prop);
    endif
  endverb

  verb "get_conj*ugation" (this none this) owner: HACKER flags: "rxd"
    "get_conj(verbspec,object) => verb conjugated according to object.";
    "verbspec can be one of \"singular/plural\", \"singular\", \"singular/\", or \"/plural\", e.g., \"is/are\", \"is\", \"is/\", or \"/are\".";
    "The object is checked to see whether it is singular or plural.  This is inferred from its .gender property.  If .gender doesn't exist or the object itself is invalid, we assume singular.";
    {spec, ?object = player} = args;
    i = index(spec + "/", "/");
    sing = spec[1..i - 1];
    if (i < length(spec))
      plur = spec[i + 1..$];
    else
      plur = "";
    endif
    cap = strcmp("a", i == 1 ? spec[2] | spec) > 0;
    if (valid(object) && TYPE_STR == typeof(g = `object.gender ! ANY') && (i = g in this.genders) && this.is_plural[i])
      vb = plur || this:_verb_plural(sing, i);
    else
      vb = sing || this:_verb_singular(plur, i);
    endif
    if (cap)
      return $string_utils:capitalize(vb);
    else
      return vb;
    endif
  endverb

  verb _verb_plural (this none this) owner: HACKER flags: "rxd"
    {st, idx} = args;
    if (typeof(st) != TYPE_STR)
      return E_INVARG;
    endif
    len = length(st);
    if (len >= 3 && rindex(st, "n't") == len - 2)
      return this:_verb_plural(st[1..len - 3], idx) + "n't";
    elseif (i = st in {"has", "is"})
      return this.(({"have", "be"}[i]))[idx];
    elseif (st == "was")
      return idx > 6 ? "were" | st;
    elseif (len <= 3 || st[len] != "s")
      return st;
    elseif (st[len - 1] != "e")
      return st[1..len - 1];
      "elseif ((r = (rindex(st, \"sses\") || rindex(st, \"zzes\"))) && (r == (len - 3)))";
    elseif ((r = rindex(st, "zzes")) && r == len - 3)
      return st[1..len - 3];
    elseif (st[len - 2] == "h" && index("cs", st[len - 3]) || index("ox", st[len - 2]) || st[len - 3..len - 2] == "ss")
      return st[1..len - 2];
      "washes => wash, belches => belch, boxes => box";
      "used to have || ((st[len - 2] == \"s\") && (!index(\"aeiouy\", st[len - 3])))";
      "so that <consonant>ses => <consonant>s";
      "known examples: none";
      "counterexample: browses => browse";
      "update of sorts--put in code to handle passes => pass";
    elseif (st[len - 2] == "i")
      return st[1..len - 3] + "y";
    else
      return st[1..len - 1];
    endif
  endverb

  verb _verb_singular (this none this) owner: HACKER flags: "rxd"
    {st, ?idx = 1} = args;
    if (typeof(st) != TYPE_STR)
      return E_INVARG;
    endif
    len = length(st);
    if (len >= 3 && rindex(st, "n't") == len - 2)
      return this:_verb_singular(st[1..len - 3], idx) + "n't";
    elseif (i = st in {"have", "are"})
      return this.(({"have", "be"}[i]))[idx];
    elseif (st[len] == "y" && !index("aeiou", st[len - 1]))
      return st[1..len - 1] + "ies";
    elseif (index("sz", st[len]) && index("aeiou", st[len - 1]))
      return st + st[len] + "es";
    elseif (index("osx", st[len]) || (len > 1 && index("chsh", st[len - 1..len]) % 2))
      return st + "es";
    else
      return st + "s";
    endif
  endverb

  verb _do (this none this) owner: HACKER flags: "rxd"
    "_do(cap,object,modifiers...)";
    {cap, object, modifiers} = args;
    if (!modifiers)
      if (typeof(object) != TYPE_OBJ)
        return tostr(object);
      elseif (!valid(object))
        return (cap ? "N" | "n") + "othing";
      else
        return cap ? object:titlec() | object:title();
      endif
    elseif (modifiers[1] == ".")
      if (i = index(modifiers[2..$], "."))
        i = i + 1;
      elseif (!(i = index(modifiers, ":") || index(modifiers, "#") || index(modifiers, "!")))
        i = length(modifiers) + 1;
      endif
      if (typeof(o = `object.(modifiers[2..i - 1]) ! ANY') == TYPE_ERR)
        return tostr("%(", o, ")");
      else
        return this:_do(cap || strcmp("a", modifiers[2]) > 0, o, modifiers[i..$]);
      endif
    elseif (modifiers[1] == ":")
      if (typeof(object) != TYPE_OBJ)
        return tostr("%(", E_TYPE, ")");
      elseif (p = this:get_pronoun(modifiers, object))
        return p;
      else
        return tostr("%(", modifiers, "??)");
      endif
    elseif (modifiers[1] == "#")
      return tostr(object);
    elseif (modifiers[1] == "!")
      return this:get_conj(modifiers[2..$], object);
    else
      i = index(modifiers, ".") || index(modifiers, ":") || index(modifiers, "#") || index(modifiers, "!") || length(modifiers) + 1;
      s = modifiers[1..i - 1];
      if (j = s in {"dobj", "iobj", "this"})
        return this:_do(cap, {dobj, iobj, callers()[2][1]}[j], modifiers[i..$]);
      else
        return tostr("%(", s, "??)");
      endif
    endif
  endverb

  verb pronoun_sub (this none this) owner: #2 flags: "rxd"
    "Experimental pronoun substitution. The official version is on $string_utils.";
    "syntax:  :pronoun_sub(text[,who])";
    "experimental version that accomodates Aladdin's style...";
    set_task_perms($no_one);
    {old, ?who = player} = args;
    if (typeof(old) == TYPE_LIST)
      plines = {};
      for line in (old)
        plines = {@plines, this:pronoun_sub(line, who)};
      endfor
      return plines;
    endif
    new = "";
    here = valid(who) ? who.location | $nothing;
    objspec = "nditl";
    objects = {who, dobj, iobj, caller, here};
    prnspec = "sopqrSOPQR";
    prprops = {"ps", "po", "pp", "pq", "pr", "Ps", "Po", "Pp", "Pq", "Pr"};
    oldlen = length(old);
    while ((prcnt = index(old, "%")) && prcnt < oldlen)
      cp_args = {};
      s = old[k = prcnt + 1];
      if (brace = index("([{", s))
        if (!(w = index(old[k + 1..oldlen], ")]}"[brace])))
          return new + old;
        elseif (brace == 3)
          s = this:_do(0, who, old[prcnt + 2..(k = k + w) - 1]);
        else
          p = old[prcnt + 2..(k = k + w) - 1];
          if (brace == 1)
            cp_args = {who, p};
          elseif (p[1] == "#")
            s = (o = index(objspec, p[2])) ? tostr(objects[o]) | "[" + p + "]";
          elseif (!(o = index(objspec, p[1])))
            s = "[" + p + "]";
          else
            cp_args = {objects[o], p[2..w - 1], strcmp(p[1], "a") < 0};
          endif
        endif
      elseif (o = index(objspec, s))
        cp_args = {objects[o], "", strcmp(s, "a") < 0};
      elseif (w = index(prnspec, s, 1))
        cp_args = {who, prprops[w]};
      elseif (s == "#")
        s = tostr(who);
      elseif (s != "%")
        s = "%" + s;
      endif
      new = new + old[1..prcnt - 1] + (!cp_args ? s | (typeof(sub = $string_utils:_cap_property(@cp_args)) != TYPE_ERR ? sub | "%(" + tostr(sub) + ")"));
      old = old[k + 1..oldlen];
      oldlen = oldlen - k;
    endwhile
    return new + old;
  endverb
endobject