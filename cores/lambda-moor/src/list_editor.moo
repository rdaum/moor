object LIST_EDITOR
  name: "List Editor"
  parent: GENERIC_EDITOR
  owner: #96
  fertile: true
  readable: true

  property objects (owner: #96, flags: "r") = {};
  property properties (owner: #96, flags: "r") = {};

  override aliases = {"List Editor"};
  override blessed_task = 917349705;
  override commands = {
    {"e*dit", "<object>.<prop>"},
    {"save", "[<object>.<prop>]"},
    {"expl*ode", "[<range>]"}
  };
  override commands2 = {
    {
      "say",
      "emote",
      "lis*t",
      "ins*ert",
      "n*ext,p*rev",
      "del*ete",
      "f*ind",
      "s*ubst",
      "m*ove,c*opy",
      "expl*ode"
    },
    {"w*hat", "abort", "q*uit,done,pause"}
  };
  override depart_msg = "%N heads off to edit some properties.";
  override import_export_id = "list_editor";
  override no_littering_msg = {
    "Partially edited list value will be here when you get back.",
    "To return, give the `@pedit' command with no arguments.",
    "Please come back and SAVE or ABORT if you don't intend to be working on this list value in the immediate future.  Keep Our MOO Clean!  No Littering!"
  };
  override object_size = {11877, 1084848672};
  override return_msg = "%N comes back from editing properties.";
  override stateprops = {
    {"properties", ""},
    {"objects", #-1},
    {"texts", 0},
    {"changes", 0},
    {"inserting", 1},
    {"readable", 0}
  };

  verb "e*dit" (any none none) owner: #96 flags: "rd"
    if (this:changed(who = player in this.active))
      player:tell("You are still editing ", this:working_on(who), ".  Please type ABORT or SAVE first.");
    elseif (spec = this:parse_invoke(dobjstr, verb))
      this:init_session(who, @spec);
    endif
  endverb

  verb save (any any any) owner: #96 flags: "rd"
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
      return;
    endif
    if (dobjstr)
      if (objprop = this:property_match_result(dobjstr))
        this.objects[who] = objprop[1];
        this.properties[who] = objprop[2];
      else
        return;
      endif
    else
      objprop = {this.objects[who], this.properties[who]};
    endif
    value_list = this:to_value(@this:text(who));
    if (value_list[1])
      player:tell("Error on line ", value_list[1], ":  ", value_list[2]);
      player:tell("Value not saved to ", this:working_on(who));
    elseif (result = this:set_property(@objprop, value_list[2]))
      player:tell("Value written to ", this:working_on(who), ".");
      this:set_changed(who, 0);
    else
      player:tell(result);
      player:tell("Value not saved to ", this:working_on(who));
    endif
  endverb

  verb "join* fill" (any any any) owner: #96 flags: "rd"
    player:tell("I don't understand that.");
  endverb

  verb "expl*ode" (any any any) owner: #96 flags: "rd"
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    elseif (typeof(range = this:parse_range(who, {"_", "1"}, @args)) != TYPE_LIST)
      player:tell(range);
    elseif (range[3])
      player:tell("Junk at end of cmd:  ", range[3]);
    else
      text = this.texts[who];
      newins = ins = this.inserting[who];
      start = range[1];
      if (typeof(debris = this:explode_line("", text[start])) == TYPE_STR)
        player:tell("Line ", start, ":  ", debris);
        return;
      endif
      if (!debris[1])
        debris = listdelete(debris, 1);
      endif
      newlines = {};
      for line in (text[i = start + 1..end = range[2]])
        dlen = length(debris);
        newlines = {@newlines, @debris[1..dlen - 1]};
        if (ins == i)
          newins = start + length(newlines) + 1;
        endif
        if (typeof(debris = this:explode_line(debris[dlen], line)) == TYPE_STR)
          player:tell("Line ", i, ":  ", debris);
          return;
        endif
        i = i + 1;
      endfor
      explen = length(newlines) + length(debris);
      if (ins > end)
        newins = ins - (end - start + 1) + explen;
      endif
      this.texts[who] = {@text[1..start - 1], @newlines, @debris, @text[end + 1..length(text)]};
      this.inserting[who] = newins;
      player:tell("--> ", start, "..", start + explen - 1);
    endif
  endverb

  verb value (this none this) owner: #96 flags: "rxd"
    if (!(e = this:readable(who = args ? args[1] | player in this.active) || this:ok(who)))
      return e;
    endif
    vlist = this:to_value(@this:text(who));
    if (vlist[1])
      player:tell("Error on line ", vlist[1], ":  ", vlist[2]);
      return E_INVARG;
    else
      return vlist[2];
    endif
  endverb

  verb working_on (this none this) owner: #96 flags: "rxd"
    if (!(who = args[1]))
      return "????";
    endif
    object = this.objects[who];
    prop = this.properties[who] || "(???)";
    return valid(object) ? tostr("\"", object.name, "\"(", object, ")", "." + prop) | tostr(".", prop, " on an invalid object (", object, ")");
  endverb

  verb init_session (this none this) owner: #96 flags: "rxd"
    if (this:ok(who = args[1]))
      this:load(who, args[4]);
      this.objects[who] = args[2];
      this.properties[who] = args[3];
      player:tell("Now editing ", this:working_on(who), ".");
    endif
  endverb

  verb property_match_result (this none this) owner: #96 flags: "rxd"
    if (!(pp = $code_utils:parse_propref(string = args[1])))
      player:tell("Property specification expected.");
      return 0;
    endif
    objstr = pp[1];
    prop = pp[2];
    if ($command_utils:object_match_failed(object = player:my_match_object(objstr, this:get_room(player)), objstr))
    elseif (!$object_utils:has_property(object, prop))
      player:tell(object.name, "(", object, ") has no \".", prop, "\" property.");
    else
      return {object, prop};
    endif
    return 0;
  endverb

  verb property (this none this) owner: #2 flags: "rx"
    "WIZARDLY";
    vl = $code_utils:verb_loc();
    if (caller != vl || caller_perms() != vl.owner)
      return E_PERM;
    endif
    set_task_perms(player);
    return args[1].((args[2]));
  endverb

  verb set_property (this none this) owner: #2 flags: "rx"
    "WIZARDLY";
    vl = $code_utils:verb_loc();
    if (caller != vl || caller_perms() != vl.owner)
      return E_PERM;
    endif
    {object, pname, value} = args;
    set_task_perms(player);
    if ($object_utils:has_callable_verb(object, "set_" + pname))
      if (typeof(attempt = object:(("set_" + pname))(value)) != TYPE_ERR)
        return attempt;
      endif
    endif
    return typeof(e = object.(pname) = value) == TYPE_ERR ? e | 1;
  endverb

  verb explode_line (this none this) owner: #96 flags: "rxd"
    su = $string_utils;
    prev = args[1];
    line = su:triml(args[2]);
    indent = length(args[2]) - length(line);
    if (line[1] == "@")
      if (!(splicee = $no_one:eval("{" + line[2..length(line)] + "}"))[1])
        return "Can't eval what's after the @.";
      endif
      newlines = this:explode_list(indent + 1, splicee[2]);
      return {prev, @newlines};
    elseif (line[1] == "}")
      if (this:is_delimiter(prev) && !index(prev, "{"))
        return {tostr((args[2])[1..indent], su:trim(prev), " ", line)};
      else
        return args;
      endif
    elseif (line[1] != "{")
      return args;
    elseif (!rindex(line, "}"))
      if (this:is_delimiter(prev))
        return {su:trimr(prev) + (rindex(prev, "{") ? " " | ", ") + line};
      else
        return args;
      endif
    elseif (!(v = $no_one:eval(line))[1])
      return "Can't eval this line.";
    else
      newlines = {@this:explode_list(indent + 2, v[2]), su:space(indent) + "}"};
      if (this:is_delimiter(prev))
        return {su:trimr(prev) + (rindex(prev, "{") ? " {" | ", {"), @newlines};
      else
        return {prev, su:space(indent) + "{", @newlines};
      endif
    endif
  endverb

  verb explode_list (this none this) owner: #96 flags: "rxd"
    ":explode_list(indent,list) => corresponding list of strings to use.";
    lines = {};
    indent = $string_utils:space(args[1]);
    for element in (args[2])
      if (typeof(element) == TYPE_STR)
        lines = {@lines, indent + "\"" + element};
      else
        lines = {@lines, indent + $string_utils:print(element)};
      endif
    endfor
    return lines;
  endverb

  verb is_delimiter (this none this) owner: #96 flags: "rxd"
    line = $string_utils:triml(args[1]);
    return line && (line[1] == "}" || (line[1] == "{" && !rindex(line, "}")));
  endverb

  verb to_value (this none this) owner: #96 flags: "rxd"
    ":to_value(@list_of_strings) => {line#, error_message} or {0,value}";
    "converts the given list of strings back into a value if possible";
    stack = {};
    curlist = {};
    curstr = 0;
    i = 0;
    for line in (args)
      i = i + 1;
      if (!(line = $string_utils:triml(line)))
        "skip blank lines";
      elseif ((char = line[1]) == "+")
        if (curstr == 0)
          return {i, "previous line is not a string"};
        endif
        curstr = curstr + line[2..length(line)];
      else
        if (curstr != 0)
          curlist = {@curlist, curstr};
          curstr = 0;
        endif
        if (char == "}" || (char == "{" && !rindex(line, "}")))
          comma = 0;
          for c in [1..length(line)]
            char = line[c];
            if (char == "}")
              if (comma)
                return {i, "unexpected `}'"};
              elseif (!stack)
                return {i, "too many }'s"};
              endif
              curlist = {@stack[1], curlist};
              stack = listdelete(stack, 1);
            elseif (char == "{")
              comma = 1;
              stack = {curlist, @stack};
              curlist = {};
            elseif (char == " ")
            elseif (!comma && char == ",")
              comma = 1;
            else
              return {i, tostr("unexpected `", char, "'")};
            endif
          endfor
        elseif (char == "\"")
          curstr = line[2..length(line)];
        elseif (char == "@")
          if (!(v = $no_one:eval("{" + line[2..length(line)] + "}"))[1])
            return {i, "Can't eval what's after the @"};
          endif
          curlist = {@curlist, @v[2]};
        else
          if (!(v = $no_one:eval(line))[1])
            return {i, "Can't eval this line"};
          endif
          curlist = {@curlist, v[2]};
        endif
      endif
    endfor
    if (stack)
      return {i, "missing }"};
    endif
    if (curstr != 0)
      return {0, {@curlist, curstr}};
    else
      return {0, curlist};
    endif
  endverb

  verb parse_invoke (this none this) owner: #96 flags: "rxd"
    if (caller != this)
      raise(E_PERM);
    elseif (!(string = args[1]))
      player:tell_lines({"Usage:  " + args[2] + " <object>.<property>", "        " + args[2] + "          (continues editing an unsaved property)"});
    elseif (!(objprop = this:property_match_result(string)))
    elseif (TYPE_ERR == typeof(value = this:property(@objprop)))
      player:tell("Couldn't get property value:  ", value);
    elseif (typeof(value) != TYPE_LIST)
      player:tell("Sorry... expecting a list-valued property.");
      if (typeof(value) == TYPE_STR)
        player:tell("Use @notedit to edit string-valued properties");
      else
        player:tell("Anyway, you don't need an editor to edit `", value, "'.");
      endif
    else
      return {@objprop, this:explode_list(0, value)};
    endif
    return 0;
  endverb
endobject