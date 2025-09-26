object FTP
  name: "FTP utilities"
  parent: ROOT_CLASS
  owner: BYTE_QUOTA_UTILS_WORKING
  readable: true

  property connections (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {};
  property port (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 21;
  property trusted (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 1;

  override aliases = {"FTP utilities"};
  override object_size = {9099, 1084848672};

  verb open (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!this:trusted(caller_perms()))
      return E_PERM;
    endif
    {host, ?user = "", ?pass = ""} = args;
    if (typeof(conn = $network:open(host, this.port)) == ERR)
      return {"Unable to connect to host."};
    endif
    this.connections = {@this.connections, {conn, caller_perms(), {}, 0, {}}};
    if (!this:wait_for_response(conn) || (user && !this:do_command(conn, "USER " + user)) || (pass && !this:do_command(conn, "PASS " + pass)))
      messages = this:get_messages(conn);
      this.connections = listdelete(this.connections, $list_utils:iassoc(conn, this.connections));
      $network:close(conn);
      return messages;
    endif
    return conn;
  endverb

  verb close (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    conn = args[1];
    if (!this:controls(caller_perms(), conn))
      return E_PERM;
    endif
    this:do_command(conn, "QUIT");
    info = $list_utils:assoc(conn, this.connections);
    this.connections = setremove(this.connections, info);
    $network:close(conn);
    if ($network:is_open(info[4]))
      $network:close(info[4]);
    endif
    return info[3];
  endverb

  verb do_command (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    {conn, cmd, ?nowait = 0} = args;
    if (!this:controls(caller_perms(), conn))
      return E_PERM;
    endif
    $network:notify(conn, cmd);
    return nowait ? 1 | this:wait_for_response(conn);
  endverb

  verb wait_for_response (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    {conn, ?first_only = 0} = args;
    if (!this:controls(caller_perms(), conn))
      return E_PERM;
    endif
    matchstr = first_only ? "^[1-9][0-9][0-9] " | "^[2-9][0-9][0-9] ";
    messages = {};
    result = "";
    while (typeof(result) == STR && !match(result, matchstr))
      result = $network:read(conn);
      messages = {@messages, result};
    endwhile
    i = $list_utils:iassoc(conn, this.connections);
    this.connections[i][3] = {@this.connections[i][3], @messages};
    if (typeof(result) == STR)
      if (result[1] in {"4", "5"})
        player:tell(result);
        return E_NONE;
      else
        return 1;
      endif
    else
      return result;
    endif
  endverb

  verb controls (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return args[1].wizard || {@$list_utils:assoc(args[2], this.connections), 0, 0}[2] == args[1];
  endverb

  verb get_messages (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    {conn, ?keep = 0} = args;
    if (!this:controls(caller_perms(), conn))
      return E_PERM;
    endif
    i = $list_utils:iassoc(conn, this.connections);
    messages = this.connections[i][3];
    if (!keep)
      this.connections[i][3] = {};
    endif
    return messages;
  endverb

  verb open_data (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    conn = args[1];
    if (!this:controls(caller_perms(), conn))
      return E_PERM;
    endif
    i = $list_utils:iassoc(conn, this.connections);
    if (!$network:is_open(this.connections[i][4]))
      this:do_command(conn, "PASV");
      msg = (msg = this:get_messages(conn, 1))[$];
      if (msg[1..3] != "227")
        return E_TYPE;
      elseif (!(match = match(msg, "(%([0-9]+%),%([0-9]+%),%([0-9]+%),%([0-9]+%),%([0-9]+%),%([0-9]+%))")))
        return E_TYPE;
      elseif (typeof(dconn = $network:open(substitute("%1.%2.%3.%4", match), toint(substitute("%5", match)) * 256 + toint(substitute("%6", match)))) == ERR)
        return dconn;
      else
        this.connections[i][4] = dconn;
      endif
      this.connections[i][5] = E_INVARG;
      set_task_perms(caller_perms());
      fork (0)
        this:listen(conn, dconn);
      endfork
    endif
    return 1;
  endverb

  verb get_data (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    {conn, ?nowait = 0} = args;
    if (!this:controls(caller_perms(), conn))
      return E_PERM;
    endif
    i = $list_utils:iassoc(conn, this.connections);
    while (!nowait && this.connections[i][5] == E_INVARG)
      suspend(0);
    endwhile
    return this.connections[i][5];
  endverb

  verb put_data (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    {conn, data} = args;
    if (!this:controls(caller_perms(), conn))
      return E_PERM;
    endif
    i = $list_utils:iassoc(conn, this.connections);
    dconn = this.connections[i][4];
    if (!$network:is_open(dconn))
      return E_INVARG;
    else
      for line in (data)
        notify(dconn, line);
        $command_utils:suspend_if_needed(0);
      endfor
      this:close_data(conn);
      this.connections[i][4] = 0;
    endif
  endverb

  verb trusted (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return args[1].wizard || (typeof(this.trusted) == LIST ? args[1] in this.trusted | this.trusted);
  endverb

  verb listen (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != this)
      return E_PERM;
    endif
    {conn, dconn} = args;
    data = {};
    line = `read(dconn) ! ANY';
    while (typeof(line) == STR)
      data = {@data, line};
      line = read(dconn);
      $command_utils:suspend_if_needed(0);
    endwhile
    if (i = $list_utils:iassoc(conn, this.connections))
      this.connections[i][5] = data;
    endif
  endverb

  verb close_data (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    conn = args[1];
    if (!this:controls(caller_perms(), conn))
      return E_PERM;
    endif
    if (!$network:is_open(dconn = $list_utils:assoc(conn, this.connections)[4]))
      return E_INVARG;
    else
      $network:close(dconn);
      "...let the reading task come to terms with its abrupt superfluousness...";
      suspend(0);
      return 1;
    endif
  endverb

  verb get (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":get(host, username, password, filename)";
    if (!this:trusted(caller_perms()))
      return E_PERM;
    endif
    if (typeof(conn = this:open(@args[1..3])) != OBJ)
      return E_NACC;
    else
      result = this:open_data(conn) && this:do_command(conn, "RETR " + args[4]) && this:get_data(conn);
      this:close(conn);
      return result;
    endif
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      this.connections = {};
      this.trusted = 1;
      pass(@args);
    endif
  endverb

  verb put (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":put(host, username, password, filename, data)";
    if (!this:trusted(caller_perms()))
      return E_PERM;
    endif
    if (typeof(conn = this:open(@args[1..3])) != OBJ)
      return E_NACC;
    else
      result = this:open_data(conn) && this:do_command(conn, "STOR " + args[4], 1) && this:put_data(conn, args[5]);
      this:close(conn);
      return result;
    endif
  endverb

  verb data_connection (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "return the data connection associated with the control connection args[1]";
    conn = args[1];
    i = $list_utils:iassoc(conn, this.connections);
    return this.connections[i][4];
  endverb
endobject