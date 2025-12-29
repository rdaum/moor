object GENERIC_OPTIONS
  name: "Generic Option Package"
  parent: ROOT_CLASS
  owner: HACKER
  fertile: true
  readable: true

  property _namelist (owner: HACKER, flags: "r") = "!";
  property extras (owner: HACKER, flags: "r") = {};
  property names (owner: HACKER, flags: "r") = {};
  property namewidth (owner: HACKER, flags: "rc") = 15;

  override aliases = {"Generic Option Package"};
  override description = "an option package in need of a description.  See `help $generic_option'...";
  override import_export_id = "generic_options";
  override object_size = {12729, 1084848672};

  verb get (this none this) owner: HACKER flags: "rxd"
    ":get(options,name) => returns the value of the option specified by name";
    "i.e., if {name,value} is present in options, return value";
    "      if name is present, return 1";
    "      otherwise return 0";
    {options, name} = args;
    if (name in options)
      return 1;
    elseif (a = $list_utils:assoc(name, options))
      return a[2];
    else
      return 0;
    endif
  endverb

  verb set (this none this) owner: HACKER flags: "rxd"
    ":set(optionlist,oname,value) => revised optionlist or string error message.";
    "oname must be the full name of an option in .names or .extras.";
    "Note that values must not be of type ERR.  ";
    "FALSE (0, blank string, or empty list) is always a legal value.";
    "If a verb :check_foo is defined on this, it will be used to typecheck any";
    "non-false or object-type value supplied as a new value for option `foo'.";
    "";
    "   :check_foo(value) => string error message or {value to use}";
    "";
    "If instead there is a property .check_foo, that will give either the expected ";
    "type or a list of allowed types.";
    "Otherwise, the option is taken to be a boolean flag and all non-false, ";
    "non-object values map to 1.";
    "";
    {options, oname, value} = args;
    if (!(oname in this.names || oname in this.extras))
      return "Unknown option:  " + oname;
    elseif (typeof(value) == TYPE_ERR)
      "... no option should have an error value...";
      return "Error value";
    elseif (!value && typeof(value) != TYPE_OBJ)
      "... always accept FALSE (0, blankstring, emptylist)...";
    elseif ($object_utils:has_callable_verb(this, check = "check_" + oname))
      "... a :check_foo verb exists; use it to typecheck the value...";
      if (typeof(c = this:(check)(value)) == TYPE_STR)
        return c;
      endif
      value = c[1];
    elseif ($object_utils:has_property(this, tprop = "type_" + oname))
      "... a .type_foo property exists...";
      "... property value should be a type or list of types...";
      if (!this:istype(value, t = this.(tprop)))
        return $string_utils:capitalize(this:desc_type(t) + " value expected.");
      endif
    elseif ($object_utils:has_property(this, cprop = "choices_" + oname))
      "... a .choices_foo property exists...";
      "... property value should be a list of {value,docstring} pairs...";
      if (!$list_utils:assoc(value, c = this.(cprop)))
        return tostr("Allowed values: ", $string_utils:english_list($list_utils:slice(c, 1), "(??)", " or "));
      endif
    else
      "... value is considered to be boolean...";
      if (!value)
        "... must be an object.  oops.";
        return tostr("Non-object value expected.");
      endif
      value = 1;
    endif
    "... We now have oname and a value.  However, if oname is one of the extras,";
    "... then we need to call :actual to see what it really means.";
    if (oname in this.names)
      nvlist = {{oname, value}};
    elseif (typeof(nvlist = this:actual(oname, value)) != TYPE_LIST || !nvlist)
      return nvlist || "Not implemented.";
    endif
    "... :actual returns a list of pairs...";
    for nv in (nvlist)
      {oname, value} = nv;
      if (i = oname in options || $list_utils:iassoc(oname, options))
        if (!value && typeof(value) != TYPE_OBJ)
          "value == 0, blank string, empty list";
          options[i..i] = {};
        elseif (value == 1)
          options[i] = oname;
        else
          options[i] = {oname, value};
        endif
      elseif (value || typeof(value) == TYPE_OBJ)
        options[1..0] = {value == 1 ? oname | {oname, value}};
      endif
    endfor
    return options;
  endverb

  verb parse (this none this) owner: HACKER flags: "rxd"
    ":parse(args[,...]) => {oname [,value]} or string error message";
    "additional arguments are fed straight through to :parse_* routines.";
    " <option> <value>     => {option, value}";
    " <option>=<value>     => {option, value}";
    " <option> is <value>  => {option, value}";
    " +<option>            => {option, 1}";
    " -<option>            => {option, 0}";
    " !<option>            => {option, 0}";
    " <option>             => {option}";
    if (!(words = args[1]))
      return "";
    endif
    option = words[1];
    words[1..1] = {};
    if (flag = option && index("-+!", option[1]))
      option[1..1] = "";
    endif
    if (i = index(option, "="))
      rawval = option[i + 1..$];
      option = option[1..i - 1];
      if (i == 1)
        "... =bar ...";
        return "Blank option name?";
      elseif (flag)
        "... +foo=bar";
        return "Don't give a value if you use +, -, or !";
      elseif (words)
        "... foo=bar junk";
        return $string_utils:from_list(words, " ") + "??";
      endif
    elseif (!option)
      return "Blank option name?";
    elseif (flag)
      if (words)
        "... +foo junk";
        return "Don't give a value if you use +, -, or !";
      endif
      rawval = (flag - 1) % 2;
    else
      words && (words[1] == "is" && (words[1..1] = {}));
      rawval = words;
    endif
    "... do we know about this option?...";
    if (!(oname = this:_name(strsub(option, "-", "_"))))
      return tostr(oname == $failed_match ? "Unknown" | "Ambiguous", " option:  ", option);
    endif
    "... determine new value...";
    if (!rawval)
      "... `@option foo is' or `@option foo=' ...";
      return rawval == {} ? {oname} | {oname, 0};
    elseif ($object_utils:has_callable_verb(this, pverb = "parse_" + oname))
      return this:(pverb)(oname, rawval, args[2..$]);
    elseif ($object_utils:has_property(this, cprop = "choices_" + oname))
      return this:parsechoice(oname, rawval, this.(cprop));
    elseif (rawval in {0, "0", {"0"}})
      return {oname, 0};
    elseif (rawval in {1, "1", {"1"}})
      return {oname, 1};
    else
      return tostr("Option is a flag, use `+", option, "' or `-", option, "' (or `!", option, "')");
    endif
  endverb

  verb _name (this none this) owner: HACKER flags: "rxd"
    ":_name(string) => full option name corresponding to string ";
    "               => $failed_match or $ambiguous_match as appropriate.";
    if ((string = args[1]) in this.names || string in this.extras)
      return string;
    endif
    char = (namestr = this._namelist)[1];
    if (!(i = index(namestr, char + string)))
      return $failed_match;
    elseif (i != rindex(namestr, char + string))
      return $ambiguous_match;
    else
      j = index(namestr[i + 1..$], char);
      return namestr[i + 1..i + j - 1];
    endif
  endverb

  verb add_name (this none this) owner: HACKER flags: "rxd"
    ":add_name(name[,isextra]) adds name to the list of options recognized.";
    "name must be a nonempty string and must not contain spaces, -, +, !, or =.";
    "isextra true means that name isn't an actual option (recognized by :get) but merely a name that the option setting command should recognize to set a particular combination of options.  Actual options go in .names; others go in .extras";
    {name, ?isextra = 0} = args;
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    elseif (!name || match(name, "[-!+= ]"))
      "...name is blank or contains a forbidden character";
      return E_INVARG;
    elseif (name in this.names)
      "...name is already in option list";
      if (isextra)
        this.names = setremove(this.names, name);
        this.extras = setadd(this.extras, name);
        return 1;
      else
        return 0;
      endif
    elseif (name in this.extras)
      if (isextra)
        return 0;
      else
        this.names = setadd(this.names, name);
        this.extras = setremove(this.extras, name);
        return 1;
      endif
    else
      char = this._namelist[1];
      if (isextra)
        this.extras = setadd(this.extras, name);
      else
        this.names = setadd(this.names, name);
      endif
      if (!index(this._namelist, char + name + char))
        this._namelist = tostr(this._namelist, name, char);
      endif
      return 1;
    endif
  endverb

  verb remove_name (this none this) owner: HACKER flags: "rxd"
    ":remove_name(name) removes name from the list of options recognized.";
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    elseif (!((name = args[1]) in this.names || name in this.extras))
      "...hmm... already gone...";
      return 0;
    else
      char = this._namelist[1];
      this._namelist = strsub(this._namelist, char + name + char, char);
      this.names = setremove(this.names, name);
      this.extras = setremove(this.extras, name);
      return 1;
    endif
  endverb

  verb show (this none this) owner: HACKER flags: "rxd"
    ":show(options,name or list of names)";
    " => text describing current value of option and what it means";
    name = args[2];
    if (typeof(name) == TYPE_LIST)
      text = {};
      for n in (name)
        text = {@text, @this:show(@listset(args, n, 2))};
      endfor
      return text;
    elseif (!(name in this.names || name in this.extras))
      return {"Unknown option:  " + name};
    elseif ($object_utils:has_callable_verb(this, sverb = "show_" + name))
      r = this:(sverb)(@args);
      value = r[1];
      desc = r[2];
    elseif ($object_utils:has_property(this, sverb) && (value = this:get(args[1], name)) in {0, 1})
      desc = this.(sverb)[value + 1];
      if (typeof(desc) == TYPE_STR)
        desc = {desc};
      endif
    elseif ($object_utils:has_property(this, cprop = "choices_" + name))
      if (!(value = this:get(args[1], name)))
        desc = this.(cprop)[1][2];
      elseif (!(a = $list_utils:assoc(value, this.(cprop))))
        return {name + " has unexpected value " + toliteral(value)};
      else
        desc = a[2];
      endif
    elseif (name in this.extras)
      return {name + " not documented (complain)"};
    else
      value = this:get(args[1], name);
      desc = {"not documented (complain)"};
      if (typeof(value) in {TYPE_LIST, TYPE_STR})
        desc[1..0] = toliteral(value);
        value = "";
      endif
    endif
    if (value in {0, 1})
      which = "-+"[value + 1] + name;
    elseif (typeof(value) in {TYPE_OBJ, TYPE_STR, TYPE_INT} && value != "")
      which = tostr(" ", name, "=", value);
    else
      which = " " + name;
    endif
    show = {$string_utils:left(which + "  ", this.namewidth) + desc[1]};
    for i in [2..length(desc)]
      show = {@show, $string_utils:space(this.namewidth) + desc[i]};
    endfor
    return show;
  endverb

  verb actual (this none this) owner: HACKER flags: "rxd"
    ":actual(<name>,<value>) => list of {<name>,<value>} pairs or string errormsg";
    " corresponding to what setting option <name> to <value> actually means";
    " e.g., :actual(\"unfoo\",1) => {{\"foo\",0}}";
    " e.g., :actual(\"g7mode\",1) => {{\"splat\",37},{\"baz\",#3}}";
    return "Not implemented.";
  endverb

  verb istype (this none this) owner: HACKER flags: "rxd"
    ":istype(value,types) => whether value is one of the given types";
    if ((vtype = typeof(value = args[1])) in (types = args[2]))
      return 1;
    elseif (vtype != TYPE_LIST)
      return 0;
    else
      for t in (types)
        if (typeof(t) == TYPE_LIST && this:islistof(value, t))
          return 1;
        endif
      endfor
    endif
    return 0;
  endverb

  verb islistof (this none this) owner: HACKER flags: "rxd"
    ":islistof(value,types) => whether value (a list) has each element being one of the given types";
    types = args[2];
    for v in (value = args[1])
      if (!this:istype(v, types))
        return 0;
      endif
    endfor
    return 1;
  endverb

  verb desc_type (this none this) owner: HACKER flags: "rxd"
    ":desc_type(types) => string description of types";
    nlist = {};
    for t in (types = args[1])
      if (typeof(t) == TYPE_LIST)
        if (length(t) > 1)
          nlist = {@nlist, tostr("(", this:desc_type(t), ")-list")};
        else
          nlist = {@nlist, tostr(this:desc_type(t), "-list")};
        endif
      elseif (t in {TYPE_INT, TYPE_OBJ, TYPE_STR, TYPE_LIST})
        nlist = {@nlist, {"number", "object", "string", "?", "list"}[t + 1]};
      else
        return "Bad type list";
      endif
    endfor
    return $string_utils:english_list(nlist, "nothing", " or ");
  endverb

  verb parsechoice (this none this) owner: HACKER flags: "rxd"
    ":parsechoice(oname,rawval,assoclist)";
    which = {};
    oname = args[1];
    rawval = args[2];
    choices = $list_utils:slice(args[3], 1);
    errmsg = tostr("Allowed values for this flag: ", $string_utils:english_list(choices, "(??)", " or "));
    if (typeof(rawval) == TYPE_LIST)
      if (length(rawval) > 1)
        return errmsg;
      endif
      rawval = rawval[1];
    elseif (typeof(rawval) != TYPE_STR)
      return errmsg;
    endif
    for c in (choices)
      if (index(c, rawval) == 1)
        which = {@which, c};
      endif
    endfor
    if (!which)
      return errmsg;
    elseif (length(which) > 1)
      return tostr(rawval, " is ambiguous.");
    else
      return {oname, which[1]};
    endif
  endverb
endobject