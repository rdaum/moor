object ERROR
  name: "Error Generator"
  parent: ROOT_CLASS
  owner: HACKER
  readable: true

  property all_errors (owner: HACKER, flags: "r") = {
    E_NONE,
    E_TYPE,
    E_DIV,
    E_PERM,
    E_PROPNF,
    E_VERBNF,
    E_VARNF,
    E_INVIND,
    E_RECMOVE,
    E_MAXREC,
    E_RANGE,
    E_ARGS,
    E_NACC,
    E_INVARG,
    E_QUOTA,
    E_FLOAT
  
  };
  property names (owner: HACKER, flags: "rc") = {
    "E_NONE",
    "E_TYPE",
    "E_DIV",
    "E_PERM",
    "E_PROPNF",
    "E_VERBNF",
    "E_VARNF",
    "E_INVIND",
    "E_RECMOVE",
    "E_MAXREC",
    "E_RANGE",
    "E_ARGS",
    "E_NACC",
    "E_INVARG",
    "E_QUOTA",
    "E_FLOAT"
  };

  override aliases = {"Error Generator"};
  override description = {
    "Object to automatically generate errors.",
    "",
    "raise(error) actually raises the error."
  };
  override object_size = {7458, 1084848672};

  verb raise (this none this) owner: HACKER flags: "rxd"
    raise(@args);
    "this:(this.names[tonum(args[1]) + 1])()";
  endverb

  verb E_NONE (this none this) owner: HACKER flags: "rxd"
    "... hmmm... don't know how to raise E_NONE...";
    return E_NONE;
  endverb

  verb E_TYPE (this none this) owner: HACKER flags: "rxd"
    "...raise E_TYPE ...";
    1[2];
  endverb

  verb E_DIV (this none this) owner: HACKER flags: "rxd"
    "...raise E_DIV ...";
    1 / 0;
  endverb

  verb E_PERM (this none this) owner: HACKER flags: "rxd"
    "...raise E_PERM ...";
    this.owner.password;
  endverb

  verb E_PROPNF (this none this) owner: HACKER flags: "rxd"
    "...raise E_PROPNF ...";
    this.a;
  endverb

  verb E_VERBNF (this none this) owner: HACKER flags: "rxd"
    "...raise E_VERBNF ...";
    this:a();
  endverb

  verb E_VARNF (this none this) owner: HACKER flags: "rxd"
    "...raise E_VARNF ...";
    a;
  endverb

  verb E_INVIND (this none this) owner: HACKER flags: "rxd"
    "...raise E_INVIND ...";
    #-1.a;
  endverb

  verb E_RECMOVE (this none this) owner: HACKER flags: "rxd"
    move(this, this);
  endverb

  verb E_MAXREC (this none this) owner: HACKER flags: "rxd"
    "...raise E_MAXREC ...";
    this:(verb)();
  endverb

  verb E_RANGE (this none this) owner: HACKER flags: "rxd"
    "...raise E_RANGE ...";
    {}[1];
  endverb

  verb E_ARGS (this none this) owner: HACKER flags: "rxd"
    "...raise E_ARGS ...";
    toint();
  endverb

  verb E_NACC (this none this) owner: HACKER flags: "rxd"
    "...raise E_NACC ...";
    move($hacker, this);
  endverb

  verb E_INVARG (this none this) owner: HACKER flags: "rxd"
    "...raise E_INVARG ...";
    parent(#-1);
  endverb

  verb E_QUOTA (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms($no_one);
    "...raise E_QUOTA ...";
    create($thing);
  endverb

  verb accept (this none this) owner: HACKER flags: "rxd"
    return 0;
  endverb

  verb name (this none this) owner: HACKER flags: "rxd"
    return toliteral(args[1]);
    "return this.names[tonum(args[1]) + 1];";
  endverb

  verb toerr (this none this) owner: HACKER flags: "rxd"
    "toerr -- given a string or a number, return the corresponding ERR.";
    "If not found or an execution error, return -1.";
    if (typeof(string = args[1]) == STR)
      for e in (this.all_errors)
        if (tostr(e) == string)
          return e;
        endif
      endfor
    elseif (typeof(number = args[1]) == INT)
      for e in (this.all_errors)
        if (toint(e) == number)
          return e;
        endif
      endfor
    endif
    return -1;
  endverb

  verb match_error (this none this) owner: HACKER flags: "rxd"
    "match_error -- searches for tostr(E_WHATEVER) in a string, returning the ERR, returns -1 if no error string is found.";
    string = args[1];
    for e in (this.all_errors)
      if (index(string, tostr(e)))
        return e;
      endif
    endfor
    return -1;
  endverb
endobject