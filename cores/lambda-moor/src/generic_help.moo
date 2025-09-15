object GENERIC_HELP
  name: "Generic Help Database"
  parent: ROOT_CLASS
  owner: HACKER
  fertile: true
  readable: true

  property index (owner: HACKER, flags: "rc") = {};
  property index_cache (owner: HACKER, flags: "r") = {};

  override aliases = {"Generic Help Database"};
  override description = "A help database of the standard form in need of a description. See `help $generic_help'...";
  override object_size = {9501, 1084848672};

  verb find_topics (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "WIZARDLY";
    if (args)
      "...check for an exact match first...";
      search = args[1];
      if (`$object_utils:has_property(parent(this), search) ! ANY')
        if ($object_utils:has_property(this, " " + search))
          return {search};
        endif
      elseif ($object_utils:has_property(this, search))
        return {search};
      endif
      "...search for partial matches, allowing for";
      "...confusion between topics that do and don't start with @, and";
      ".. confusion between - and _ characters.";
      props = properties(this);
      topics = {};
      if (search[1] == "@")
        search = search[2..$];
      endif
      search = strsub(search, "-", "_");
      if (!search)
        "...don't try searching for partial matches if the string is empty or @";
        "...we'd get *everything*...";
        return {};
      endif
      for prop in (props)
        if ((i = index(strsub(prop, "-", "_"), search)) == 1 || (i == 2 && index(" @", prop[1])))
          topics = {@topics, prop[1] == " " ? prop[2..$] | prop};
        endif
      endfor
      return topics;
    else
      "...return list of all topics...";
      props = setremove(properties(this), "");
      for p in (`$object_utils:all_properties(parent(this)) ! ANY => {}')
        if (i = " " + p in props)
          props = {p, @listdelete(props, i)};
        endif
      endfor
      return props;
    endif
  endverb

  verb get_topic (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "WIZARDLY";
    {topic, ?dblist = {}} = args;
    if (`$object_utils:has_property(parent(this), topic) ! ANY')
      text = `this.((" " + topic)) ! ANY';
    else
      text = `this.(topic) || this.((" " + topic)) ! ANY';
    endif
    if (typeof(text) == LIST)
      if (text && text[1] == "*" + (vb = strsub(text[1], "*", "")) + "*")
        text = `this:(vb)(listdelete(text, 1), dblist) ! ANY';
      endif
    endif
    return text;
  endverb

  verb sort_topics (this none this) owner: HACKER flags: "rxd"
    ":sort_topics(list_of_topics) -- sorts the given list of strings, assuming that they're help-system topic names";
    buckets = "abcdefghijklmnopqrstuvwxyz";
    keys = names = $list_utils:make(length(buckets) + 1, {});
    for name in (setremove(args[1], ""))
      key = index(".@", name[1]) ? name[2..$] + " " | name;
      k = index(buckets, key[1]) + 1;
      bucket = keys[k];
      i = $list_utils:find_insert(bucket, key);
      keys[k] = listinsert(bucket, key, i);
      names[k] = listinsert(names[k], name, i);
      $command_utils:suspend_if_needed(0);
    endfor
    return $list_utils:append(@names);
  endverb

  verb columnize (this none this) owner: HACKER flags: "rxd"
    ":columnize(@list_of_strings) -- prints the given list in a number of columns wide enough to accomodate longest entry. But no more than 4 columns.";
    longest = $list_utils:longest(args);
    for d in ({4, 3, 2, 1})
      if (79 / d >= length(longest))
        return $string_utils:columnize_suspended(0, args, d);
      endif
    endfor
  endverb

  verb "forward pass" (this none this) owner: HACKER flags: "rxd"
    "{\"*forward*\", topic, @rest}  => text for topic from this help db.";
    "{\"*pass*\",    topic, @rest}  => text for topic from next help db.";
    "In both cases the text of @rest is appended.  ";
    "@rest may in turn begin with a *<verb>*";
    {text, ?dblist = {}} = args;
    if (verb == "forward")
      first = this:get_topic(text[1], dblist);
    elseif ((result = $code_utils:help_db_search(text[1], dblist)) && (db = result[1]) != $ambiguous_match)
      first = db:get_topic(result[2], dblist[(db in dblist) + 1..$]);
    else
      first = {};
    endif
    if (2 <= length(text))
      if (text[2] == "*" + (vb = strsub(text[2], "*", "")) + "*")
        return {@first, @`this:(vb)(text[3..$], dblist) ! ANY => {}'};
      else
        return {@first, @text[2..$]};
      endif
    else
      return first;
    endif
  endverb

  verb subst (this none this) owner: HACKER flags: "rxd"
    "{\"*subst*\", @text} => text with the following substitutions:";
    "  \"...%[expr]....\" => \"...\"+value of expr (assumed to be a string)+\"....\"";
    "  \"%;expr\"         => @(value of expr (assumed to be a list of strings))";
    newlines = {};
    for old in (args[1])
      new = "";
      bomb = 0;
      while ((prcnt = index(old, "%")) && prcnt < length(old))
        new = new + old[1..prcnt - 1];
        code = old[prcnt + 1];
        old = old[prcnt + 2..$];
        if (code == "[")
          prog = "";
          while ((b = index(old + "]", "]")) > (p = index(old + "%", "%")))
            prog = prog + old[1..p - 1] + old[p + 1];
            old = old[p + 2..$];
          endwhile
          prog = prog + old[1..b - 1];
          old = old[b + 1..$];
          value = $no_one:eval_d(prog);
          if (value[1])
            new = tostr(new, value[2]);
          else
            new = tostr(new, toliteral(value[2]));
            bomb = 1;
          endif
        elseif (code != ";" || new)
          new = new + "%" + code;
        else
          value = $no_one:eval_d(old);
          if (value[1] && typeof(r = value[2]) == LIST)
            newlines = {@newlines, @r[1..$ - 1]};
            new = tostr(r[$]);
          else
            new = tostr(new, toliteral(value[2]));
            bomb = 1;
          endif
          old = "";
        endif
      endwhile
      if (bomb)
        newlines = {@newlines, new + old, tostr("@@@ Helpfile alert:  Previous line is messed up; notify ", this.owner.wizard ? "" | tostr(this.owner.name, " (", this.owner, ") or "), "a wizard. @@@")};
      else
        newlines = {@newlines, new + old};
      endif
    endfor
    return newlines;
  endverb

  verb index (this none this) owner: HACKER flags: "rxd"
    "{\"*index*\" [, title]}";
    "This produces a columnated list of topics in this help db, headed by title.";
    $command_utils:suspend_if_needed(0);
    title = args[1] ? args[1][1] | tostr(this.name, " (", this, ")");
    su = $string_utils;
    return {"", title, su:from_list($list_utils:map_arg(su, "space", su:explode(title), "-"), " "), @this:columnize(@this:sort_topics(this:find_topics()))};
  endverb

  verb initialize (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    pass(@args);
    if ($perm_utils:controls(caller_perms(), this))
      this.r = 1;
      this.f = 0;
    endif
  endverb

  verb verbdoc (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "{\"*verbdoc*\", \"object\", \"verbname\"}  use documentation for this verb";
    set_task_perms(this.owner);
    if (!valid(object = $string_utils:match_object(args[1][1], player.location)))
      return E_INVARG;
    elseif (!(hv = $object_utils:has_verb(object, vname = args[1][2])))
      return E_VERBNF;
    else
      return $code_utils:verb_documentation(hv[1], vname);
    endif
  endverb

  verb dump_topic (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    try
      text = this.((fulltopic = args[1]));
      return {tostr(";;", $code_utils:corify_object(this), ".(", toliteral(fulltopic), ") = $command_utils:read_lines()"), @$command_utils:dump_lines(text)};
    except error (ANY)
      return error[1];
    endtry
  endverb

  verb objectdoc (this none this) owner: HACKER flags: "rxd"
    "{\"*objectdoc*\", \"object\"} => text for topic from object:help_msg";
    if (!valid(object = $string_utils:literal_object(args[1][1])))
      return E_INVARG;
    elseif (!($object_utils:has_verb(object, "help_msg") || $object_utils:has_property(object, "help_msg")))
      return E_VERBNF;
    else
      return $code_utils:verb_or_property(object, "help_msg");
    endif
  endverb

  verb find_index_topics (this none this) owner: HACKER flags: "rxd"
    ":find_index_topic([search])";
    "Return the list of index topics of this help DB";
    "(i.e., those which contain an index (list of topics)";
    "this DB, return it, otherwise return false.";
    "If search argument is given and true,";
    "we first remove any cached information concerning index topics.";
    {?search = 0} = args;
    if (this.index_cache && !search)
      "...make sure every topic listed in .index_cache really is an index topic";
      for p in (this.index_cache)
        if (!("*index*" in `this.(p) ! ANY => {}'))
          search = 1;
        endif
      endfor
      if (!search)
        return this.index_cache;
      endif
    elseif ($generic_help == this)
      return {};
    endif
    itopics = {};
    for p in (properties(this))
      if ((h = `this.(p) ! ANY') && "*index*" in h)
        itopics = {@itopics, p};
      endif
    endfor
    this.index_cache = itopics;
    return itopics;
  endverb
endobject