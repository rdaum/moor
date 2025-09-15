object GENERIC_BIGLIST_HOME
  name: "Generic BigList Resident"
  parent: ROOT_CLASS
  owner: HACKER
  fertile: true
  readable: true

  property _genprop (owner: HACKER, flags: "rc") = "a";
  property _mgr (owner: HACKER, flags: "rc") = BIGLIST;
  property mowner (owner: HACKER, flags: "rc") = HACKER;

  override aliases = {"biglist", "resident", "gblr"};
  override description = {
    "This is the object you want to use as a parent in order to @create a place for your biglists to live.  Suitably sick souls may wish to reimplement :_genprop and :_kill to reclaim unused properties (this :_kill just throws them away and this :_genprop just relentlessly advances....  who cares).  Anyway, you'll need to look at $biglist before this will make sense."
  };
  override object_size = {3606, 1084848672};

  verb _make (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":_make(...) => new node with value {...}";
    if (!(caller in {this._mgr, this}))
      return E_PERM;
    endif
    prop = this:_genprop();
    add_property(this, prop, args, {$generic_biglist_home.owner, ""});
    return prop;
  endverb

  verb _kill (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":_kill(node) destroys the given node.";
    if (!(caller in {this, this._mgr}))
      return E_PERM;
    endif
    delete_property(this, args[1]);
  endverb

  verb _get (this none this) owner: HACKER flags: "rxd"
    return caller == this._mgr ? this.((args[1])) | E_PERM;
  endverb

  verb _put (this none this) owner: HACKER flags: "rxd"
    return caller == this._mgr ? this.((args[1])) = listdelete(args, 1) | E_PERM;
  endverb

  verb _genprop (this none this) owner: HACKER flags: "rxd"
    gp = this._genprop;
    ngp = "";
    for i in [1..length(gp)]
      if (gp[i] != "z")
        ngp = ngp + "bcdefghijklmnopqrstuvwxyz"[strcmp(gp[i], "`")] + gp[i + 1..$];
        return " " + (this._genprop = ngp);
      endif
      ngp = ngp + "a";
    endfor
    return " " + (this._genprop = ngp + "a");
  endverb

  verb _ord (this none this) owner: HACKER flags: "rxd"
    "this is a dummy. You have to decide what your leaves are going to look like and then write this verb accordingly.  It should, given a leaf/list-element, return the corresponding key value.  So for an ordinary alist, where all of the leaves are of the form {key,datum}, you want:";
    return args[1][1];
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    pass(@args);
    this.mowner = $hacker;
    this._mgr = $biglist;
  endverb
endobject