object HTTP
  name: "HTTP Server"
  parent: THING
  location: BYTE_QUOTA_UTILS_WORKING
  owner: BYTE_QUOTA_UTILS_WORKING

  property alpha (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
  property guests (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {};
  property help_msg (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {
    "HTTP Server",
    "-----------",
    "",
    "To use this device, all you need to do is put a .html property on your object. It will then serve a web page at http://SITE:PORT/OBJID where OBJID is the object's ID number without the # sign, SITE is your MOO's address, and PORT is your MOO's port.",
    "If it starts with the line <HTML>, the server will automatically serve it up as HTML code.",
    "Otherwise, you will need to use your own HTTP headers (for example, if you want to serve plain text instead of HTML).",
    "",
    "If you want to have more functionality than this, you can make a verb :html(args), that does the same thing. The args are a list of anything after the OBJID divided by /.",
    "For example, the URL http://SITE:PORT/123/foo/10 will result in a call to #123:html(\"foo\",\"10\"). This call should return a list of strings that is HTML, or HTTP stuff with the appropriate headers.",
    "",
    "Additionally, your object can call http:tell_key(player). This will generate a special keyed URL for that player, so when your :html() verb is called, the built-in variable PLAYER will be set to the person who was issued the key.",
    "This verb tells the player their custom key directly."
  };
  property master_key (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "902894a52d08e5cf3c45812a321cac36";
  property nonalpha (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = " !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";

  override aliases = {"HTTP Server"};
  override object_size = {7309, 1529542623};

  verb handle_connection (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "HTTP Server";
    "";
    "This gets called by the #0:do_login_command";
    "It keeps track of web browser guests, and it feeds them HTTP instead of Telnet.";
    if (caller != #0 && caller != this)
      return E_PERM;
    endif
    not_http = args ? args | {$login.blank_command};
    guest = $string_utils:connection_hostname(connection_name(player));
    if (!args)
      if ((guestnum = $list_utils:iassoc(guest, this.guests)) > 0)
        guestinfo = this.guests[guestnum];
        if (guestinfo[3] > 1)
          "Three blank requests means this is not a guest anymore.";
          this.guests = listdelete(this.guests, guestnum);
          return not_http;
        endif
        guestinfo[3] = guestinfo[3] + 1;
        this.guests[guestnum] = guestinfo;
      else
        "Don't know who this is. Do the regular welcome.";
        return not_http;
      endif
    elseif ((guestnum = $list_utils:iassoc(guest, this.guests)) > 0)
      "Previously registered HTTP guest.";
      if (args && args[1] == "GET")
        "Registered guest does a GET.";
        guestinfo = {guest, time(), 0};
        this.guests[guestnum] = guestinfo;
        html = {};
        keys = $string_utils:explode(args[2], "/");
        objid = keys ? toobj(keys[1]) | #-1;
        if (valid(objid))
          if ($object_utils:has_callable_verb(objid, "html"))
            new_player = $no_one;
            if (length(keys) >= 3 && length(keys[3]) > 2)
              if (keys[3] == this:gen_key(keys[1], keys[2], (keys[3])[1..2]))
                new_player = toobj(keys[2]);
                keys = length(keys) > 3 ? keys[4..$] | {};
              endif
            endif
            old_player = player;
            player = new_player;
            html = `objid:html(@keys) ! ANY';
            player = old_player;
            if (typeof(html) == ERR)
              html = {"HTTP/1.1 500 Internal Server Error", "Content-Type: text/plain", "", "", "ERROR:", tostr(objid) + ":html()", tostr(html)};
            endif
          elseif ($object_utils:has_readable_property(objid, "html"))
            html = objid.html;
          else
            html = {"HTTP/1.1 404 NOT FOUND", "Content-Type: text/plain", "", "", "HTML not found."};
          endif
        else
          html = {"HTTP/1.1 404 NOT FOUND", "Content-Type: text/plain", "", "", "Object not found."};
        endif
        "Default header is HTML";
        html_header = {"HTTP/1.1 200 OK", "Content-Type: text/html", "", ""};
        if (typeof(html) == STR)
          html = {html};
        endif
        if (`(html[1])[1..6] == "<HTML>" ! ANY => 0')
          html = {@html_header, @html};
        endif
        if (typeof(html) == LIST && length(html) == 0)
          html = html_header;
        endif
        if (html && typeof(html) == LIST)
          for h in (html)
            if (typeof(h) == STR)
              notify(player, h);
            endif
          endfor
        endif
        "Server options need a suspend before they take effect.";
        old_boot = $server_options.boot_msg;
        $server_options.boot_msg = "";
        suspend(0);
        boot_player(player);
        $server_options.boot_msg = old_boot;
      elseif (args[1][$] == ":")
        "Client is telling us some HTTP header info.";
        boot_player(player);
      else
        "They stopped doing GETs, so remove them as HTTP.";
        this.guests = listdelete(this.guests, guestnum);
        return not_http;
      endif
    elseif (args && args[1] == "GET")
      "First visit. Register them as a guest and ask them to refresh.";
      guestinfo = {guest, time(), 0};
      this.guests = {@this.guests, guestinfo};
      html = {"HTTP/1.1 200 OK", "Content-Type: text/html", "", "", "<meta http-equiv=\"refresh\" content=\"0\"><br><b>", "", "", "", "Web service initialized. Please reload this page.", "</b>"};
      for h in (html)
        notify(player, h);
      endfor
      boot_player(player);
    else
      "Regular telnet client.";
      return not_http;
    endif
    "HTTP Session successful";
    return;
  endverb

  verb tell_key (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "This makes a URL key for a specific object and player.";
    "With the keyed URL, the PLAYER will be set correctly when :HTML() is called";
    key = this:gen_key(player, caller);
    notify(player, "This is the private key for you, " + player.name + ". Do not share it.");
    notify(player, "http://" + $network.site + ":" + $network.port + "/" + tostr(caller)[2..$] + "/" + tostr(player)[2..$] + "/" + key + "/");
  endverb

  verb gen_key (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != this)
      return E_PERM;
    endif
    player = args[1];
    object = args[2];
    hash = (tonum(player) + tonum(object) + this.master_key) % 100000000;
    if (length(args) > 2)
      salt = args[3];
      key = crypt(tostr(hash), salt);
    else
      "Make it only alphanumeric salt, to get through a URL";
      salt = this.alpha[random(length(this.alpha))] + this.alpha[random(length(this.alpha))];
      key = crypt(tostr(hash), salt);
    endif
    "Clean out the non-alpha to make key work in a URL";
    for i in [1..length(key)]
      if (j = index(this.nonalpha, key[i], 1))
        key[i] = this.alpha[j];
      endif
    endfor
    return key;
  endverb
endobject