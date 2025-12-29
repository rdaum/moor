object GOPHER
  name: "Gopher utilities"
  parent: ROOT_CLASS
  owner: #2
  readable: true

  property cache_requests (owner: #2, flags: "r") = {};
  property cache_timeout (owner: #2, flags: "r") = 900;
  property cache_times (owner: #2, flags: "r") = {};
  property cache_values (owner: #2, flags: "r") = {};
  property frozen (owner: #2, flags: "rc") = 0;
  property limit (owner: #2, flags: "rc") = 2000;

  override aliases = {"Gopher utilities"};
  override description = {
    "An interface to Gopher internet services.",
    "Copyright (c) 1992,1993 Grump,JoeFeedback@LambdaMOO.",
    "",
    "This object contains just the raw verbs for getting data from gopher servers and parsing the results. Look at #50122 (Generic Gopher Slate) for one example of a user interface. ",
    "",
    ":get(site, port, selection)",
    "  Get data from gopher server: returns a list of strings, or an error if it couldn't connect. Results are cached.",
    "",
    ":get_now(site, port, selection)",
    "  Used by $gopher:get. Arguments are the same: this actually gets the ",
    "  data without checking the cache. (Don't call this, since the",
    "  caching is important to reduce lag.)",
    "  ",
    ":show_text(who, start, end, site, port, selection)",
    "  Requires wiz-perms to call.",
    "  like who:notify_lines($gopher:get(..node..)[start..end])",
    "",
    ":clear_cache()",
    "  Erase the gopher cache.",
    "",
    ":parse(string)",
    "  Takes a directory line as returned by $gopher:get, and return a list",
    "  {host, port, selector, label}",
    "   host, port, and selector are what you send to :get.",
    "  label is a string, where the first character is the type code.",
    "",
    ":type(char)",
    "   returns the name of the gopher type indicated by the character, e.g.",
    "   $gopher:type(\"I\") => \"image\"",
    ""
  };
  override import_export_id = "gopher";
  override object_size = {15578, 1084848672};

  verb get_now (this none this) owner: #2 flags: "rxd"
    "Usage:  get_now(site, port, message)";
    "Returns a list of strings, or an error if we couldn't connect.";
    {host, port, message, ?extra = {0}} = args;
    if (!this:trusted(caller_perms()))
      return E_PERM;
    elseif (!match(host, $network.valid_host_regexp) && !match(host, "[0-9]+%.[0-9]+%.[0-9]+%.[0-9]+"))
      "allow either welformed internet hosts or explicit IP addresses.";
      return E_INVARG;
    elseif (port < 100 && !(port in {13, 70, 80, 81, 79}))
      "I added port 13, which is used for atomic clock servers. -Krate";
      "disallow connections to low number ports; necessary?";
      return E_INVARG;
    endif
    opentime = time();
    con = $network:open(host, port);
    opentime = time() - opentime;
    if (typeof(con) == TYPE_ERR)
      return con;
    endif
    notify(con, message);
    results = {};
    count = this.limit;
    "perhaps this isn't necessary, but if a gopher source is slowly spewing things, perhaps we don't want to hang forever -- perhaps this should just fork a process to close the connection instead?";
    now = time();
    timeout = 30;
    end = "^%.$";
    if (extra[1] == "2")
      end = "^[2-9]";
    endif
    while (typeof(string = `read(con) ! ANY') == TYPE_STR && !match(string, end) && (count = count - 1) > 0 && now + timeout > (now = time()))
      if (string && string[1] == ".")
        string = string[2..$];
      endif
      results = {@results, string};
    endwhile
    $network:close(con);
    if (opentime > 0)
      "This is to keep repeated calls to $network:open to 'slow responding hosts' from totally spamming.";
      suspend(0);
    endif
    return results;
  endverb

  verb parse (this none this) owner: #2 flags: "rxd"
    "parse gopher result line:";
    "return {host, port, tag, label}";
    "host/port/tag are what you send to the gopher server to get that line";
    "label is <type>/human readable entry";
    {string} = args;
    tab = index(string, "\t");
    label = string[1..tab - 1];
    string = string[tab + 1..$];
    tab = index(string, "\t");
    tag = string[1..tab - 1];
    string = string[tab + 1..$];
    tab = index(string, "\t");
    host = string[1..tab - 1];
    if (host[$] == ".")
      host = host[1..$ - 1];
    endif
    string = string[tab + 1..$];
    tab = index(string, "\t");
    port = toint(tab ? string[1..tab - 1] | string);
    return {host, port, tag, label};
    "ignore extra material after port, if any";
  endverb

  verb show_text (this none this) owner: #2 flags: "rxd"
    "$gopher:show_text(who, start, end, ..node..)";
    "like who:notify_lines($gopher:get(..node..)[start..end]), but pipelined";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {who, start, end, @args} = args;
    con = $network:open(who, start);
    if (typeof(con) == TYPE_ERR)
      player:tell("Sorry, can't get this information now.");
      return;
    endif
    notify(con, end);
    line = 0;
    sent = 0;
    end = end || this.limit;
    while ((string = `read(con) ! ANY') != "." && typeof(string) == TYPE_STR)
      line = line + 1;
      if (line >= start && (!end || line <= end))
        sent = sent + 1;
        if (valid(who))
          if (string && string[1] == ".")
            string = string[2..$];
          endif
          who:notify(string);
        else
          notify(who, string);
        endif
      endif
    endwhile
    $network:close(con);
    return sent;
  endverb

  verb type (this none this) owner: #2 flags: "rxd"
    type = args[1];
    if (type == "1")
      return "menu";
    elseif (type == "?")
      return "menu?";
    elseif (type == "0")
      return "text";
    elseif (type == "7")
      return "search";
    elseif (type == "9")
      return "binary";
    elseif (type == "2")
      return "phone directory";
    elseif (type == "4")
      return "binhex";
    elseif (type == "8")
      return "telnet";
    elseif (type == "I")
      return "image";
    elseif (type == " ")
      "not actually gopher protocol: used by 'goto'";
      return "";
    else
      return "unknown";
    endif
    "not done, need to fill out";
  endverb

  verb summary (this none this) owner: #2 flags: "rxd"
    "return a 'nice' string showing the information in a gopher node";
    if (typeof(parse = args[1]) == TYPE_STR)
      parse = this:parse(parse);
    endif
    if (parse[1] == "!")
      return {"[remembered set]", "", ""};
    endif
    if (length(parse) > 3)
      label = parse[4];
      if (label)
        type = $gopher:type(label[1]);
        label = label[2..$];
        if (type == "menu")
        elseif (type == "search")
          label = "<" + (parse[3])[rindex(parse[3], "\t") + 1..$] + "> " + label;
        else
          label = type + ": " + label;
        endif
      else
        label = "(top)";
      endif
    else
      label = parse[3] + " (top)";
    endif
    port = "";
    if (parse[2] != 70)
      port = tostr(" ", parse[2]);
    endif
    return {tostr("[", parse[1], port, "]"), label, parse[3]};
  endverb

  verb get (this none this) owner: #2 flags: "rxd"
    "Usage: get(site, port, selection)";
    "returns a list of strings, or an error if it couldn't connect. Results are cached.";
    if (this.frozen)
      return E_QUOTA;
    endif
    request = args[1..3];
    while ((index = request in this.cache_requests) && this.cache_times[index] > time())
      if (typeof(result = this.cache_values[index]) != TYPE_INT)
        return result;
      endif
      if ($code_utils:task_valid(result))
        "spin, let other process getting same data win, or timeout";
        suspend(1);
      else
        "well, other process crashed, or terminated, or whatever.";
        this.cache_times[index] = 0;
      endif
    endwhile
    if (!this:trusted(caller_perms()))
      return E_PERM;
    endif
    while (this.cache_times && this.cache_times[1] < time())
      $command_utils:suspend_if_needed(0);
      this.cache_times = listdelete(this.cache_times, 1);
      this.cache_values = listdelete(this.cache_values, 1);
      this.cache_requests = listdelete(this.cache_requests, 1);
      "caution: don't want to suspend between test and removal";
    endwhile
    $command_utils:suspend_if_needed(0);
    this:cache_entry(@request);
    value = this:get_now(@args);
    $command_utils:suspend_if_needed(0);
    index = this:cache_entry(@request);
    this.cache_times[index] = time() + (typeof(value) == TYPE_ERR ? 120 | 1800);
    this.cache_values[index] = value;
    return value;
  endverb

  verb clear_cache (this none this) owner: #2 flags: "rxd"
    if (!this:trusted(caller_perms()))
      return E_PERM;
    endif
    if (!args)
      this.cache_values = this.cache_times = this.cache_requests = {};
    elseif (index = args[1..3] in this.cache_requests)
      this.cache_requests = listdelete(this.cache_requests, index);
      this.cache_times = listdelete(this.cache_times, index);
      this.cache_values = listdelete(this.cache_values, index);
    endif
  endverb

  verb unparse (this none this) owner: #2 flags: "rxd"
    "unparse(host, port, tag, label) => string";
    {host, port, tag, label} = args;
    if (tab = index(tag, "\t"))
      "remove search terms from search nodes";
      tag = tag[1..tab - 1];
    endif
    return tostr(label, "\t", tag, "\t", host, "\t", port);
  endverb

  verb interpret_error (this none this) owner: #2 flags: "rxd"
    "return an explanation for a 'false' $gopher:get result";
    value = args[1];
    if (value == E_INVARG)
      return "That gopher server is not reachable or is not responding.";
    elseif (value == E_QUOTA)
      return "Gopher connections cannot be made at this time because of system resource limitations!";
    elseif (typeof(value) == TYPE_ERR)
      return tostr("The gopher request results in an error: ", value);
    else
      return "The gopher request has no results.";
    endif
  endverb

  verb trusted (this none this) owner: #2 flags: "rxd"
    "default -- gopher trusts everybody";
    return 1;
  endverb

  verb _textp (this none this) owner: #2 flags: "rxd"
    "_textp(parsed node)";
    "Return true iff the parsed info points to a text node.";
    return index("02", args[1][4][1]);
  endverb

  verb _mail_text (this none this) owner: #2 flags: "rxd"
    "_mail_text(parsed node)";
    "Return the text to be mailed out for the given node.";
    where = args[1];
    if (this:_textp(where))
      return $gopher:get(@where);
    else
      text = {};
      for x in ($gopher:get(@where))
        parse = $gopher:parse(x);
        sel = parse[4];
        text = {@text, "Type=" + sel[1], "Name=" + sel[2..$], "Path=" + parse[3], "Host=" + parse[1], "Port=" + tostr(parse[2]), "#"};
      endfor
      return text;
    endif
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      this:clear_cache();
      pass(@args);
    endif
  endverb

  verb display_cache (this none none) owner: #2 flags: "rxd"
    "Just for debugging -- shows what's in the gopher cache";
    req = this.cache_requests;
    tim = this.cache_times;
    val = this.cache_values;
    "save values in case cache changes while printing";
    player:tell("size -- expires -- host (port) ------ selector ------------");
    for i in [1..length(req)]
      re = req[i];
      host = $string_utils:left(re[1] + (re[2] == 70 ? "" | tostr(" (", re[2], ")")), 24);
      expires = $string_utils:right($time_utils:dhms(tim[i] - time()), 8);
      va = val[i];
      if (typeof(va) == TYPE_LIST)
        va = length(va);
      elseif (typeof(va) == TYPE_ERR)
        va = toliteral(va);
      else
        va = tostr(va);
      endif
      selector = re[3];
      if (length(selector) > 40)
        selector = "..." + selector[$ - 37..$];
      endif
      player:tell($string_utils:right(va, 8), expires, " ", host, selector);
    endfor
    player:tell("--- end cache display -------------------------------------");
  endverb

  verb get_cache (this none this) owner: #2 flags: "rxd"
    "Usage: get_cache(site, port, selection)";
    "return current cache";
    request = args[1..3];
    if (index = request in this.cache_requests)
      if (this.cache_times[index] > now)
        return this.cache_values[index];
      endif
    endif
    return 0;
  endverb

  verb cache_entry (this none this) owner: #2 flags: "rxd"
    if (index = args in this.cache_requests)
      return index;
    else
      this.cache_times = {@this.cache_times, time() + 240};
      this.cache_values = {@this.cache_values, task_id()};
      this.cache_requests = {@this.cache_requests, args};
      return length(this.cache_requests);
    endif
  endverb

  verb help_msg (this none this) owner: #2 flags: "rxd"
    return this:description();
  endverb

  verb daily (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      day = 24 * 3600;
      hour_of_day_GMT = 10;
      fork (hour_of_day_GMT * 60 * 60 + day - time() % day)
        this:daily();
      endfork
      "  this.frozen = 1";
      this:clear_cache();
      "  suspend(3900)";
      this.frozen = 0;
    endif
  endverb

  verb get_now_EXPERIMENTAL (this none this) owner: #2 flags: "rxd"
    "Copied from Sleeper (#98232):get_now Thu Oct  2 17:15:49 2003 PDT";
    "Copied from Gopher utilities (#15357):get_now by Retired-Wizard-1 (#49853) Thu Oct  2 16:57:12 2003 PDT";
    "Usage:  get_now(site, port, message)";
    "Returns a list of strings, or an error if we couldn't connect.";
    {host, port, message, ?extra = {0}} = args;
    if (!this:trusted(caller_perms()))
      return E_PERM;
    elseif (!match(host, $network.valid_host_regexp) && !match(host, "[0-9]+%.[0-9]+%.[0-9]+%.[0-9]+"))
      "allow either welformed internet hosts or explicit IP addresses.";
      return E_INVARG;
    elseif (port < 100 && !(port in {13, 70, 80, 81, 79}))
      "I added port 13, which is used for atomic clock servers. -Krate";
      "disallow connections to low number ports; necessary?";
      return E_INVARG;
    endif
    opentime = time();
    con = $network:open(host, port);
    opentime = time() - opentime;
    if (typeof(con) == TYPE_ERR)
      return con;
    endif
    if (typeof(message) == TYPE_LIST)
      for line in (message)
        notify(con, line);
      endfor
    else
      notify(con, message);
    endif
    results = {};
    count = this.limit;
    "perhaps this isn't necessary, but if a gopher source is slowly spewing things, perhaps we don't want to hang forever -- perhaps this should just fork a process to close the connection instead?";
    now = time();
    timeout = 30;
    end = "^%.$";
    if (extra[1] == "2")
      end = "^[2-9]";
    endif
    while (typeof(string = `read(con) ! ANY') == TYPE_STR && !match(string, end) && (count = count - 1) > 0 && now + timeout > (now = time()))
      if (string && string[1] == ".")
        string = string[2..$];
      endif
      results = {@results, string};
    endwhile
    $network:close(con);
    if (opentime > 0)
      "This is to keep repeated calls to $network:open to 'slow responding hosts' from totally spamming.";
      suspend(0);
    endif
    return results;
  endverb
endobject