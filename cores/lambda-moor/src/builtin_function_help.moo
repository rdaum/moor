object BUILTIN_FUNCTION_HELP
  name: "Builtin Function Help"
  parent: GENERIC_HELP
  owner: HACKER
  readable: true

  property "abs()" (owner: HACKER, flags: "rc") = {
    "Syntax:  abs (num <x>)   => num",
    "",
    "Returns the absolute value of <x>.  If <x> is negative, then the result is `-<x>'; otherwise, the result is <x>. The number x can be either integer or floating-point; the result is of the same kind."
  };
  property "acos()" (owner: HACKER, flags: "rc") = {
    "Syntax:  acos (FLOAT <x>)   => FLOAT",
    "",
    "Returns the arc-cosine (inverse cosine) of x, in the range [0..pi]. Raises E_INVARG if x is outside the range [-1.0..1.0]."
  };
  property "add_property()" (owner: HACKER, flags: "rc") = {
    "Syntax:  add_property (obj <object>, str <prop-name>, <value>, list <info>)   => none",
    "",
    "Defines a new property on the given <object>, inherited by all of its descendants; the property is named <prop-name>, its initial value is <value>, and its owner and initial permission bits are given by <info> in the same format as is returned by `property_info()'. If <object> is not valid or <object> already has a property named <prop-name> or <info> does not specify a legitimate owner and permission bits, then E_INVARG is raised.  If the programmer does not have write permission on <object> or if the owner specified by <info> is not the programmer and the programmer is not a wizard, then E_PERM is raised."
  };
  property "add_verb()" (owner: HACKER, flags: "rc") = {
    "Syntax:  add_verb (obj <object>, list <info>, list <args>)   => int",
    "",
    "Defines a new verb on the given <object>.  The new verb's owner, permission bits and name(s) are given by <info> in the same format as is returned by `verb_info()'.  The new verb's direct-object, preposition, and indirect-object specifications are given by <args> in the same format as is returned by `verb_args()'.  The new verb initially has the empty program associated with  it; this program does nothing but return an unspecified value.  ",
    "",
    "If <object> is not valid, or <info> does not specify a valid owner and well-formed permission bits and verb names, or <args> is not a legitimate syntax specification, then `E_INVARG' is raised.  If the programmer does not have write permission on <object> or if the owner specified by <info> is not the programmer and the programmer is not a wizard, then `E_PERM' is raised.  Otherwise, this function returns a positive integer representing the new verb's index in this object's `verbs()' list.  "
  };
  property "asin()" (owner: HACKER, flags: "rc") = {
    "Syntax:  asin (FLOAT <x>)   => FLOAT",
    "",
    "Returns the arc-sine (inverse sine) of x, in the range [-pi/2..pi/2]. Raises E_INVARG if x is outside the range [-1.0..1.0]."
  };
  property "atan()" (owner: HACKER, flags: "rc") = {
    "Syntax:  atan (FLOAT <y> [, FLOAT <x>])   => FLOAT",
    "",
    "Returns the arc-tangent (inverse tangent) of y in the range [-pi/2..pi/2] if x is not provided, or of y/x in the range [-pi..pi] if x is provided."
  };
  property "binary_hash()" (owner: HACKER, flags: "rc") = {
    "Syntax:  binary_hash (STR bin-string)   => STR",
    "         string_hash (STR text)         => STR",
    "",
    "Returns a 32-character hexadecimal string encoding the result of applying the MD5 cryptographically secure hash function to the contents of the string `text' or the binary string `bin-string'. MD5, like other such functions, has the property that, if",
    "",
    "string_hash(x) == string_hash(y)",
    "",
    "then, almost certainly",
    "",
    "equal(x, y)",
    "",
    "This can be useful, for example, in certain networking applications:  after sending a large piece of text across a connection, also send across the result of applying string_hash() to the text; if the destination site also applies string_hash() to the text and gets the same result, you can be quite confident that the large text has arrived unchanged."
  };
  property "boot_player()" (owner: HACKER, flags: "rc") = {
    "Syntax:  boot_player (obj <player>)   => none",
    "",
    "Immediately terminates any currently-active connection to the given <player>.  The connection will not actually be closed until the currently-running task returns or suspends, but all MOO functions (such as notify(), connected_players(), and the like) immediately behave as if the connection no longer exists. If the programmer is not either a wizard or the same as <player>, then `E_PERM' is returned.  If there is no currently-active connection to <player>, then this function does nothing.",
    "",
    "If there was a currently-active connection, then the following verb call is made when the connection is actually closed:",
    "",
    "$user_disconnected(player)",
    "",
    "It is not an error if this verb does not exist; the corresponding call is simply skipped."
  };
  property "buffered_output_length()" (owner: HACKER, flags: "rc") = {
    "Syntax:  buffered_output_length ([OBJ conn])   => INT",
    "",
    "Returns the number of bytes currently buffered for output to the connection `conn'.  If conn is not provided, returns the maximum number of bytes that will be buffered up for output on any connection."
  };
  property "builtin-index" (owner: HACKER, flags: "rc") = {"*index*", "Server Built-in Functions"};
  property "call_function()" (owner: HACKER, flags: "rc") = {
    "Syntax:  call_function (STR func-name, arg, ...)   => value",
    "",
    "Calls the built-in function named `func-name', passing the given arguments, and returns whatever that function returns. Raises E_INVARG if func-name is not recognized as the name of a known built-in function. This allows you to compute the name of the function to call and, in particular, allows you to write a call to a built-in function that may or may not exist in the particular version of the server you're using."
  };
  property "caller_perms()" (owner: HACKER, flags: "rc") = {
    "Syntax:  caller_perms ()   => obj",
    "",
    "Returns the permissions in use by the verb that called the currently-executing",
    "verb.  If the currently-executing verb was not called by another verb (i.e., it",
    "is the first verb called in a command or server task), then",
    "`caller_perms()' returns `#-1'."
  };
  property "callers()" (owner: HACKER, flags: "rc") = {
    "Syntax:  callers ([include-line-numbers])   => list",
    "",
    "Returns information on each of the verbs and built-in functions currently waiting to resume execution in the current task.  When one verb or function calls another verb or function, execution of the caller is temporarily suspended, pending the called verb or function returning a value.  At any given time, there could be several such pending verbs and functions: the one that called the currently executing verb, the verb or function that called that one, and so on.  The result of `callers()' is a list, each element of which gives information about one pending verb or function in the following format:",
    "",
    "  {<this>, <verb-name>, <programmer>, <verb-loc>, <player>, <line-number>}",
    "",
    "For verbs, <this> is the initial value of the variable `this' in that verb, <verb-name> is the name used to invoke that verb, <programmer> is the player with whose permissions that verb is running, <verb-loc> is the object on which that verb is defined, and <player> is the initial value of the variable `player' in that verb, and <line-number> indicates which line of the verb's code is executing. The <line-number> element is included only if the `include-line-numbers' argument was provided and is true.",
    "",
    "For functions, <this>, <programmer>, and <verb-loc> are all #-1, <verb-name> is the name of the function, and <line-number> is an index used internally to determine the current state of the built-in function. The simplest correct test for a built-in function entry is",
    "",
    "(VERB-LOC == #-1 && PROGRAMMER == #-1 && VERB-NAME != \"\")",
    "",
    "",
    "The first element of the list returned by `callers()' gives information on the verb that called the currently-executing verb, the second element describes the verb that called that one, and so on.  The last element of the list describes the first verb called in this task."
  };
  property "ceil()" (owner: HACKER, flags: "rc") = {
    "Syntax:  ceil (FLOAT <x>)   => FLOAT",
    "",
    "Returns the smallest integer not less than x, as a floating-point number."
  };
  property "children()" (owner: HACKER, flags: "rc") = {"*forward*", "parent()"};
  property "chparent()" (owner: HACKER, flags: "rc") = {
    "Syntax:  chparent (obj <object>, obj <new-parent>)   => none",
    "",
    "Changes the parent of <object> to be <new-parent>. If <object> is not valid, or if <new-parent> is neither valid nor equal to #-1, then E_INVARG is raised. If the programmer is neither a wizard or the owner of <object>, or if <new-parent> is not fertile (i.e., its `f' bit is not set) and the programmer is neither the owner of <new-parent> nor a wizard, then `E_PERM' is raised.  If <new-parent> is equal to <object> or one of its current ancestors, E_RECMOVE is raised. If <object> or one of its descendants defines a property with the same name as one defined either on <new-parent> or on one of its ancestors, then `E_INVARG' is returned.",
    "",
    "Changing an object's parent can have the effect of removing some properties from and adding some other properties to that object and all of its descendants (i.e., its children and its children's children, etc.).  Let <common> be the nearest ancestor that <object> and <new-parent> have in common before the parent of <object> is changed.  Then all properties defined by ancestors of <object> under <common> (that is, those ancestors of <object> that are in turn descendants of <common>) are removed from <object> and all of its descendants.  All properties defined by <new-parent> or its ancestors under <common> are added to <object> and all of its descendants.  As with `create()', the newly-added properties are given the same permission bits as they have on <new-parent>, the owner of each added property is either the owner of the object it's added to (if the `c' permissions bit is set) or the owner of that property on <new-parent>, and the value of each added property is \"clear\"; see the description of the built-in function `clear_property()' for details.  All properties that are not removed or added in the reparenting process are completely unchanged.",
    "",
    "If <new-parent> is equal to #-1, then <object> is given no parent at all; it becomes a new root of the parent/child hierarchy. In this case, all formerly inherited properties on <object> are simply removed."
  };
  property "clear_property()" (owner: HACKER, flags: "rc") = {
    "Syntax:  clear_property (OBJ <object>, STR <prop-name>)  => none",
    "      is_clear_property (OBJ <object>, STR <prop-name>)  => INT",
    "",
    "These two functions test for clear and set to clear, respectively, the property named <prop-name> on the given <object>.  If <object> is not valid, then E_INVARG is raised.  If <object> has no non-built-in property named <prop-name>, then E_PROPNF is raised.  If the programmer does not have read (write) permission on the property in question, then `is_clear_property()' (`clear_property()') raises E_PERM. If a property is clear, then when the value of that property is queried the value of the parent's property of the same name is returned.  If the parent's property is clear, then the parent's parent's value is examined, and so on. If <object> is the definer of the property <prop-name>, as opposed to an inheritor of the property, then `clear_property()' raises E_INVARG."
  };
  property "connected_players()" (owner: HACKER, flags: "rc") = {
    "Syntax:  connected_players ([include-all])   => LIST",
    "",
    "Returns a list of the object numbers of those player objects with currently-active connections. If <include-all> is provided and true, includes the object numbers associated with all current connections, including those that are outbound and/or not yet logged-in."
  };
  property "connected_seconds()" (owner: HACKER, flags: "rc") = {
    "Syntax:  connected_seconds (obj <player>)   => int",
    "              idle_seconds (obj <player>)   => int",
    "",
    "These functions return the number of seconds that the currently-active connection to <player> has existed and been idle, respectively.  If <player> is not the object number of a player object with a currently-active connection, then E_INVARG is raised."
  };
  property "connection_name()" (owner: HACKER, flags: "rc") = {
    "Syntax:  connection_name (obj <player>)   => str",
    "",
    "Returns a network-specific string identifying the connection being used by the given player.  If the programmer is not a wizard and not <player>, then E_PERM is raised.  If <player> is not currently connected, then E_INVARG is raised.",
    "",
    "For the TCP/IP networking configurations, for in-bound connections, the string has the form",
    "",
    "  \"port <lport> from <host>, port <port>\"",
    "",
    "where <lport> is the listening port on which the connection arrived, <host> is either the name or decimal TCP address of the host to which the connection was opened, and <port> is the decimal TCP port of the connection on that host.",
    "",
    "For the System V 'local' networking configuration, the string is the UNIX login name of the connecting user or, if no such name can be found, something of the form",
    "",
    "  \"User <#number>\"",
    "",
    "where <#number> is a UNIX numeric user ID.",
    "",
    "For the other networking configurations, the string is the same for all connections and, thus, useless."
  };
  property "connection_option()" (owner: HACKER, flags: "rc") = {
    "Syntax:  connection_option (OBJ conn, STR name)   => value",
    "",
    "Returns the current setting of the option <name> for the connection <conn>. Raises E_INVARG if <conn> does not specify a current connection and E_PERM if the programmer is neither <conn> nor a wizard."
  };
  property "connection_options()" (owner: HACKER, flags: "rc") = {
    "Syntax:  connection_options (OBJ conn)   => LIST",
    "",
    "Return a list of (<name>, <value>) pairs describing the current settings of all of the allowed options for the connection <conn>. Raises E_INVARG if <conn> does not specify a current connection and E_PERM if the programmer is neither <conn> nor a wizard."
  };
  property "cos()" (owner: HACKER, flags: "rc") = {"*forward*", "sin()"};
  property "cosh()" (owner: HACKER, flags: "rc") = {"*forward*", "sinh()"};
  property "create()" (owner: HACKER, flags: "rc") = {
    "Syntax:  create (obj <parent> [, obj <owner>])   => obj",
    "",
    "Creates and returns a new object whose parent is <parent> and whose owner is as described below.  Either the given <parent> object must be fertile (i.e., its `f' bit must be set) or else the programmer must own <parent> or be a wizard; otherwise `E_PERM' is raised. `E_PERM' is also raised if <owner> is provided and not the same as the programmer, unless the programmer is a wizard.  After the new object is created, its `initialize' verb, if any, is called with no arguments.",
    "",
    "The new object is assigned the least non-negative object number that has not yet been used for a created object.  Note that no object number is ever reused, even if the object with that number is recycled.",
    "",
    "The owner of the new object is either the programmer (if <owner> is not provided), the new object itself (if <owner> was given as `#-1'), or <owner> (otherwise).",
    "",
    "The other built-in properties of the new object are initialized as follows:",
    "    name         \"\"",
    "    location     #-1",
    "    contents     {}",
    "    programmer   0",
    "    wizard       0",
    "    r            0",
    "    w            0",
    "    f            0",
    "",
    "In addition, the new object inherits all of the other properties on <parent>.  These properties have the same permission bits as on <parent>.  If the `c' permissions bit is set, then the owner of the property on the new object is the same as the owner of the new object itself; otherwise, the owner of the property on the new object is the same as that on <parent>.  The initial value of every inherited property is \"clear\"; see the description of the built-in function `clear_property()' for details.",
    "",
    "",
    "If the intended owner of the new object has a property named `ownership_quota' and the value of that property is a number, then `create()' treats that value as a \"quota\".  If the quota is less than or equal to zero, then the quota is considered to be exhausted and `create()' raises `E_QUOTA' instead of creating an object. Otherwise, the quota is decremented and stored back into the `ownership_quota' property as a part of the creation of the new object."
  };
  property "crypt()" (owner: HACKER, flags: "rc") = {
    "Syntax:  crypt (str <text> [, str <salt>])   => str",
    "",
    "Encrypts the given <text> using the standard UNIX encryption method.  If provided, <salt> should be a two-character string for use as the extra encryption ``salt'' in the algorithm.  If <salt> is not provided, a random pair of characters is used.  In any case, the salt used is also returned as the first two characters of the resulting encrypted string.  ",
    "",
    "Aside from the possibly-random selection of the salt, the encryption algorithm is entirely deterministic.  In particular, you can test whether or not a given string is the same as the one used to produced a given piece of encrypted text; simply extract the first two characters of the encrypted text and pass the candidate string and those two characters to `crypt()'.  If the result is identical to the given encrypted text, then you've got a match.  ",
    "",
    "    crypt(\"foobar\")         =>   \"J3fSFQfgkp26w\"",
    "    crypt(\"foobar\", \"J3\")   =>   \"J3fSFQfgkp26w\"",
    "    crypt(\"mumble\", \"J3\")   =>   \"J3D0.dh.jjmWQ\"",
    "    crypt(\"foobar\", \"J4\")   =>   \"J4AcPxOJ4ncq2\"",
    "",
    "Note: As of version 1.8.3, the entire salt (of any length) is passed to the operating system's low-level crypt function.  It is unlikely, however, that all operating systems will return the same string when presented with a longer salt.  Therefore, identical calls to `crypt()' may generate different results on different platforms, and your password verification systems will fail.  Use a salt longer than two characters at your own risk."
  };
  property "ctime()" (owner: HACKER, flags: "rc") = {
    "Syntax:  ctime ([INT <time>])   => str",
    "",
    "Interprets <time> as a time, using the same representation as given in the description of `time()', and converts it into a 28-character, human-readable string in the following format:",
    "",
    "    Mon Aug 13 19:13:20 1990 PDT",
    "",
    "If the current day of the month is less than 10, then an extra blank appears between the month and the day:",
    "",
    "    Mon Apr  1 14:10:43 1991 PST",
    "",
    "If <time> is not provided, then the current time is used.",
    "",
    "Note that `ctime()' interprets <time> for the local time zone of the computer on which the MOO server is running."
  };
  property "db_disk_size()" (owner: HACKER, flags: "rc") = {
    "Syntax:  db_disk_size()   => INT",
    "",
    "Returns the total size, in bytes, of the most recent full representation of the database as one or more disk files. Raises E_QUOTA if, for some reason, no such on-disk representation is currently available."
  };
  property "decode_binary()" (owner: HACKER, flags: "rc") = {
    "Syntax:  decode_binary (STR bin-string [, fully])   => LIST",
    "",
    "Returns a list of strings and/or integers representing the bytes in the binary string <bin-string> in order. If <fully> is false or omitted, the list contains an integer only for each non-printing, non-space byte; all other characters are grouped into the longest possible contiguous substrings. If <fully> is proved and true, the list contains only integers, one for each byte represented in <bin-string>. Raises E_INVARG if <bin-string> is not a properly-formed binary string. (See the LambdaMOO programmer's manual on MOO value types for a full description of binary strings.)",
    "",
    "decode_binary(\"foo\")               =>  {\"foo\"}",
    "decode_binary(\"~~foo\")             =>  {\"~foo\"}",
    "decode_binary(\"foo~0D~0A\")         =>  {\"foo\", 13, 10}",
    "decode_binary(\"foo~0Abar~0Abaz\")   =>  {\"foo\", 10, \"bar\", 10, \"baz\"}",
    "decode_binary(\"foo~0D~0A\", 1)      =>  {102, 111, 111, 13, 10}"
  };
  property "delete_property()" (owner: HACKER, flags: "rc") = {
    "Syntax:  delete_property (obj <object>, str <prop-name>)   => none",
    "",
    "Removes the property named <prop-name> from the given <object> and all of its descendants.  If <object> is not valid, then E_INVARG is raised.  If the programmer does not have write permission on <object>, then E_PERM is raised.  If <object> does not directly define a property named <prop-name> (as opposed to inheriting one from its parent), then `E_PROPNF' is raised."
  };
  property "delete_verb()" (owner: HACKER, flags: "rc") = {
    "Syntax:  delete_verb (obj <object>, str <verb-name>)   => none",
    "",
    "Removes the verb named <verb-name> from the given <object>.  If <object> is not valid, then E_INVARG is raised.  If the programmer does not have write permission on <object>, then E_PERM is raised. If <object> does not define a verb named <verb-name>, then E_VERBNF is raised."
  };
  property "disassemble()" (owner: HACKER, flags: "rc") = {
    "Syntax:  disassemble (OBJ object, STR verb-desc)   => LIST",
    "",
    "Returns a (longish) list of strings giving a listing of the server's internal \"compiled\" form of the verb as specified by <verb-desc> on <object>. This format is not documented and may indeed change from release to release, but some programmers may nonetheless find the output of `disassemble()' interesting to peruse as a way to gain a deeper appreciation of how the server works.",
    "",
    "If <object> is not valid, then E_INVARG is raised. If <object> does not define a verb as specified by <verb-desc>, then E_VERBNF is raised. If the programmer does not have read permission on the verb in question, then disassemble() raises E_PERM."
  };
  property "dump_database()" (owner: HACKER, flags: "rc") = {
    "Syntax:  dump_database ()   => none",
    "",
    "Requests that the server checkpoint the database at its next opportunity.  It is not normally necessary to call this function; the server automatically checkpoints the database at regular intervals; see the chapter on server assumptions about the database for details.  If the programmer is not a wizard, then E_PERM is raised."
  };
  property "encode_binary()" (owner: HACKER, flags: "rc") = {
    "Syntax:  encode_binary(arg, ...)   => STR",
    "",
    "Each argument must be an integer between 0 and 255, a string, or a list containing only legal arguments for this function. This function translates each integer and string in turn into its binary string equivalent, returning the concatenation of all these substrings into a single binary string. (See the early sections in the LambdaMOO Programmer's Manual on MOO value types for a full description of binary strings.)",
    "",
    "encode_binary(\"~foo\")                     =>  \"~7Efoo\"",
    "encode_binary({\"foo\", 10}, {\"bar\", 13})   =>  \"foo~0Abar~0D\"",
    "encode_binary(\"foo\", 10, \"bar\", 13)       =>  \"foo~0Abar~0D\""
  };
  property "equal()" (owner: HACKER, flags: "rc") = {
    "Syntax:  equal(value1, value2)   => INT",
    "",
    "Returns true if <value1> is completely indistinguishable from <value2>. This is much the same operation as \"<value1> == <value2>\" except that, unlike ==, the `equal()' function does not treat upper- and lower-case characters in strings as equal.",
    "",
    "Raises E_ARGS if none, one, or more than two arguments are given.",
    "",
    "equal(1, 2)                   => 0",
    "equal(\"ChIcKeN\", \"chicken\")   => 0",
    "equal(\"ABC123\", \"ABC123\")     => 1"
  };
  property "eval()" (owner: HACKER, flags: "rc") = {
    "Syntax:  eval (str <string>)   => list",
    "",
    "The MOO-code compiler processes <string> as if it were to be the program associated with some verb and, if no errors are found, that fictional verb is invoked.  If the programmer is not, in fact, a programmer, then E_PERM is raised.  The normal result of calling `eval()' is a two element list. The first element is true if there were no compilation errors and false otherwise.  The second element is either the result returned from the fictional verb (if there were no compilation errors) or a list of the compiler's error messages (otherwise).",
    "",
    "When the fictional verb is invoked, the various built-in variables have values as shown below:",
    "",
    "    player    the same as in the calling verb",
    "    this      #-1",
    "    caller    the same as the initial value of `this' in the calling verb",
    "",
    "    args      {}",
    "    argstr    \"\"",
    "",
    "    verb      \"\"",
    "    dobjstr   \"\"",
    "    dobj      #-1",
    "    prepstr   \"\"",
    "    iobjstr   \"\"",
    "    iobj      #-1",
    "",
    "The fictional verb runs with the permissions of the programmer and as if its `d' permissions bit were on.",
    "",
    "    eval(\"return 3 + 4;\")   =>   {1, 7}"
  };
  property "exp()" (owner: HACKER, flags: "rc") = {
    "Syntax:  exp (FLOAT x)   => FLOAT",
    "",
    "Returns `e' raised to the power of <x>."
  };
  property "floatstr()" (owner: HACKER, flags: "rc") = {
    "Syntax:  floatstr (FLOAT x, INT precision [, scientific])   => STR",
    "",
    "Converts <x> into a string with more control than provided by either `tostr()' or `toliteral()'. <Precision> is the number of digits to appear to the right of the decimal point, capped at 4 more than the maximum available precision, a total of 19 on most machines; this makes it possible to avoid rounding errors if the resulting string is subsequently read back as a floating-point value. If <scientific> is false or not provided, the result is a string in the form \"MMMMMMM.DDDDDD\", preceded by a minus sign if and only if <x> is negative. If <scientific> is provided and true, the result is a string in the form \"M.DDDDDDe+EEE\", again preceded by a minus sign if and only if <x> is negative."
  };
  property "floor()" (owner: HACKER, flags: "rc") = {
    "Syntax:  floor (FLOAT x)   => FLOAT",
    "",
    "Returns the largest integer not greater than x, as a floating-point number."
  };
  property "flush_input()" (owner: HACKER, flags: "rc") = {
    "Syntax:  flush_input (OBJ conn [, show-messages])   => none",
    "",
    "Performs the same actions as if the connection <conn>'s definied flush command had been received on that connection, i.e., removes all pending lines of input from <conn>'s queue and, if <show-messages> is provided and true, prints a messages to <conn> listing the flushed lines, if any.  See the chapter in the LambdaMOO Programmer's Manual on server assumptions about the database for more information about a connection's defined flush command."
  };
  property "force_input()" (owner: HACKER, flags: "rc") = {
    "Syntax:  force_input (OBJ conn, STR line [, at-front])   => none",
    "",
    "Inserts the string <line> as an input task in the queue for the connection <conn>, just as if it had arrived as input over the network. If <at-front> is provided and true, then the new line of input is put at the front of <conn>'s queue, so that it will be the very next line of input processed even if there is already some other input in that queue. Raises E_INVARG if <conn> does not specify a current connection and E_PERM if the programmer is neither <conn> nor a wizard."
  };
  property "function_info()" (owner: HACKER, flags: "rc") = {
    "Syntax:  function_info ([STR name])   => LIST",
    "",
    "Returns descriptions of the various built-in functions available on the server. If <name> is provided, only the description of the function with that name is returned. If <name> is omitted, a list of descriptions is returned, one for each function available on the server. E_INVARG is raised if <name> is provided but no function with that name is available on the server.",
    "",
    "Each function description is a list of the following form:",
    "",
    "  {<name>, <min-args>, <max-args>, <types>}",
    "",
    "where <name> is the name of the built-in function, <min-args> is the minimum number of arguments that must be to the function, <max-args> is the maximum number of arguments that can be provided to the function or -1 if there is no maximum, and <types> is a list of <max-args> integers (or <min-args> if <max-args> is -1), each of which represents the type of argument required in the corresponding position. Each type number is as would be returned from the `typeof()' built-in function except that -1 indicates that any type of value is acceptable and -2 indicates that either integers or floating-point numbers may be given. For example, here are several entries from the list:",
    "",
    "  {\"listdelete\", 2, 2, {4, 0}}",
    "  {\"suspend\", 0, 1, {0}}",
    "  {\"server_log\", 1, 2, {2, -1}}",
    "  {\"max\", 1, -1, {-2}}",
    "  {\"tostr\", 0, -1, {}}",
    "",
    "`Listdelete()' takes exactly 2 arguments, of which the first must be a list (LIST == 4) and the second must be an integer (INT == 0). `Suspend()' has one optional argument that, if provided, must be an integer. `Server_log()' has one required argument that must be a string (STR == 2) and one optional argument that, if provided, may be of any type. `Max()' requires at least one argument but can take any number above that, and the first argument must be either an integer or a floating-point number; the type(s) required for any other arguments can't be determined from this description. Finally, `tostr()' takes any number of arguments at all, but it can't be determined from this description which argument types would be acceptable in which positions."
  };
  property "idle_seconds()" (owner: HACKER, flags: "rc") = {"*forward*", "connected_seconds()"};
  property "index()" (owner: HACKER, flags: "rc") = {
    "Syntax:  index (STR <str1>, STR <str2> [, <case-matters>])   => INT",
    "        rindex (STR <str1>, STR <str2> [, <case-matters>])   => INT",
    "",
    "The function `index()' (`rindex()') returns the index of the first character of the first (last) occurrence of <str2> in <str1>, or zero if <str2> does not occur in <str1> at all.  By default the search for an occurrence of <str2> is done while ignoring the upper/lower case distinction.  If <case-matters> is provided and true, then case is treated as significant in all comparisons.",
    "",
    "    index(\"foobar\", \"o\")        =>   2",
    "    rindex(\"foobar\", \"o\")       =>   3",
    "    index(\"foobar\", \"x\")        =>   0",
    "    index(\"foobar\", \"oba\")      =>   3",
    "    index(\"Foobar\", \"foo\", 1)   =>   0"
  };
  property "is_clear_property()" (owner: HACKER, flags: "rc") = {"*forward*", "clear_property()"};
  property "is_member()" (owner: HACKER, flags: "rc") = {
    "Syntax:  is_member (ANY value, LIST list)   => INT",
    "",
    "Returns true if there is an element of <list> that is completely indistinguishable from <value>. This is much the same operation as \"<value> in <list>\" except that, unlike `in', the `is_member()' function does not treat upper- and lower-case characters in strings as equal.",
    "",
    "Raises E_ARGS if two values are given or if more than two values are given. Raises E_TYPE if the second argument is not a list. Otherwise returns the index of <value> in <list>, or 0 if it's not in there.",
    "",
    "  is_member(3, {3, 10, 11})                 => 1",
    "  is_member(\"a\", {\"A\", \"B\", \"C\"})           => 0",
    "  is_member(\"XyZ\", {\"XYZ\", \"xyz\", \"XyZ\"})   => 3"
  };
  property "is_player()" (owner: HACKER, flags: "rc") = {
    "Syntax:  is_player (OBJ <object>)   => INT",
    "",
    "Returns a true value if the given <object> is a player object and a false value otherwise.  If <object> is not valid, E_INVARG is raised."
  };
  property "kill_task()" (owner: HACKER, flags: "rc") = {
    "Syntax:  kill_task (INT <task-id>)   => none",
    "",
    "Removes the task with the given <task-id> from the queue of waiting tasks. If the programmer is not the owner of that task and not a wizard, then E_PERM is raised.  If there is no task on the queue with the given <task-id>, then E_INVARG is raised."
  };
  property "length()" (owner: HACKER, flags: "rc") = {
    "Syntax:  length (<list or string>)   => int",
    "",
    "Returns the number of characters in <list or string>.  ",
    "",
    "    length(\"foo\")       =>   3",
    "    length(\"\")          =>   0",
    "    length({1, 2, 3})   =>   3",
    "    length({})          =>   0"
  };
  property "listappend()" (owner: HACKER, flags: "rc") = {"*forward*", "listinsert()"};
  property "listdelete()" (owner: HACKER, flags: "rc") = {
    "Syntax:  listdelete (LIST <list>, INT <index>)   => LIST",
    "",
    "Returns a copy of <list> with the <index>th element removed.  If <index> is not in the range `[1..length(<list>)]', then E_RANGE is raised.",
    "",
    "    x = {\"foo\", \"bar\", \"baz\"};",
    "    listdelete(x, 2)   =>   {\"foo\", \"baz\"}"
  };
  property "listen()" (owner: HACKER, flags: "rc") = {
    "Syntax:  listen (OBJ object, point [, print-messages])   => value",
    "",
    "Create a new point at which the server will listen for network connections, just as it does normally. <Object> is the object whose verbs `do_login_command', `do_command', `do_out_of_band_command', `user_connected', `user_created', `user_reconnected', `user_disconnected', and `user_client_disconnected' will be called at appropriate points asthese verbs are called on #0 for normal connections. (See the chapter in the LambdaMOO Programmer's Manual on server assumptions about the database for the complete story on when these functions are called.) <Point> is a network-configuration-specific parameter describing the listening point. If <print-messages> is provided and true, then the various database-configurable messages (also detailed in the chapter on server assumptions) will be printed on connections received at the new listening point. `Listen()' returns <canon>, a `canonicalized' version of <point>, with any configuration-specific defaulting or aliasing accounted for.",
    "",
    "This raises E_PERM if the programmer is not a wizard, E_INVARG if <object> is invalid or there is already a listening point described by <point>, and E_QUOTA if some network-configuration-specific error occurred.",
    "",
    "For the TCP/IP configurations, <point> is a TCP port number on which to listen and <canon> is equal to <point> unless <point> is zero, in which case <canaon> is a port number assigned by the operating system.",
    "",
    "For the local multi-user configurations, <point> is the UNIX file name to be used as the connection point and <canon> is always equal to <point>.",
    "",
    "In the single-user configuration, there can be only one listening point at a time; <point> can be any value at all and <canon> is always zero."
  };
  property "listeners()" (owner: HACKER, flags: "rc") = {
    "Syntax:  listeners ()  => LIST",
    "",
    "Returns a list describing all existing listening points, including the default one set up automatically by the server when it was started (unless that one has since been destroyed by a call to `unlisten()'). Each element of the list has the following form:",
    "",
    "  {<object>, <canon>, <print-messages>}",
    "",
    "where <object> is the first argument given in the call to `listen()' to create this listening point, <print-messages> is true if the third argument in that call was provided and true, and <canon> was the value returned by that call. (For the initial listening point, <object> is #0, <canon> is determined by the command-line arguments or a network-configuration-specific default, and <print-messages> is true.)"
  };
  property "listinsert()" (owner: HACKER, flags: "rc") = {
    "Syntax:  listinsert (LIST <list>, <value> [, INT <index>])   => list",
    "         listappend (LIST <list>, <value> [, INT <index>])   => list",
    "",
    "These functions return a copy of <list> with <value> added as a new element.  `listinsert()' and `listappend()' add <value> before and after (respectively) the existing element with the given <index>, if provided.",
    "",
    "The following three expressions always have the same value:",
    "",
    "    listinsert(<list>, <element>, <index>)",
    "    listappend(<list>, <element>, <index> - 1)",
    "    {@<list>[1..<index> - 1], <element>, @<list>[<index>..length(<list>)]}",
    "",
    "If <index> is not provided, then `listappend()' adds the <value> at the end of the list and `listinsert()' adds it at the beginning; this usage is discouraged, however, since the same intent can be more clearly expressed using the list-construction expression, as shown in the examples below.",
    "",
    "    x = {1, 2, 3};",
    "    listappend(x, 4, 2)   =>   {1, 2, 4, 3}",
    "    listinsert(x, 4, 2)   =>   {1, 4, 2, 3}",
    "    listappend(x, 4)      =>   {1, 2, 3, 4}",
    "    listinsert(x, 4)      =>   {4, 1, 2, 3}",
    "    {@x, 4}               =>   {1, 2, 3, 4}",
    "    {4, @x}               =>   {4, 1, 2, 3}"
  };
  property "listset()" (owner: HACKER, flags: "rc") = {
    "Syntax:  listset (LIST <list>, <value>, INT <index>)   => LIST",
    "",
    "Returns a copy of <list> with the <index>th element replaced by <value>.  If <index> is not in the range `[1..length(<list>)]', then E_RANGE is raised.",
    "",
    "    x = {\"foo\", \"bar\", \"baz\"};",
    "    listset(x, \"mumble\", 2)   =>   {\"foo\", \"mumble\", \"baz\"}",
    "",
    "This function exists primarly for historical reasons; it was used heavily before the server supported indexed assignments like x[i] = v. New code should always use indexed assignment instead of `listset()' wherever possible."
  };
  property "load_server_options()" (owner: HACKER, flags: "rc") = {
    "Syntax:  load_server_options ()   => none",
    "",
    "After modifying properties on $server_options, wizards must call `load_server_options()'.  Changes made may not take effect until this function is called.  This allows the server to cache option values internally; this significantly speeds up built-in function invocation.  If the programmer is not a wizard, then E_PERM is raised."
  };
  property "log()" (owner: HACKER, flags: "rc") = {
    "Syntax:  log (FLOAT x)     => FLOAT",
    "         log10 (FLOAT x)   => FLOAT",
    "",
    "Returns the natural or base 10 logarithm of <x>. Raises E_INVARG if <x> is not positive."
  };
  property "log10()" (owner: HACKER, flags: "rc") = {"*forward*", "log()"};
  property "log_cache_stats()" (owner: HACKER, flags: "rc") = {"*forward*", "verb_cache_stats()"};
  property "match()" (owner: HACKER, flags: "rc") = {
    "Syntax:  match (STR <subject>, STR <pattern> [, <case-matters>])  => LIST",
    "         rmatch (STR <subject>, STR <pattern> [, <case-matters>])  => LIST",
    "",
    "The function `match()' (`rmatch()') searches for the first (last) occurrence of the regular expression <pattern> in the string <subject>.  If <pattern> is syntactically malformed, then E_INVARG is raised. The process of matching can in some cases consume a great deal of memory in the server; should this memory consumption become excessive, then the matching process is aborted and E_QUOTA is raised.",
    "",
    "If no match is found, the empty list is returned; otherwise, these functions return a list containing information about the match (see below).  By default, the search ignores upper/lower case distinctions.  If <case-matters> is provided and true, then case is treated as significant in all comparisons.",
    "",
    "The list that `match()' (`rmatch()') returns contains the details about the match made.  The list is in the form:",
    "",
    "     {<start>, <end>, <replacements>, <subject>}",
    "",
    "where <start> is the index in STRING of the beginning of the match, <end> is the index of the end of the match, <replacements> is a list described below, and <subject> is the same string that was given as the first argument to the `match()' or `rmatch()'.",
    "",
    "The <replacements> list is always nine items long, each item itself being a list of two numbers, the start and end indices in <subject> matched by some parenthesized sub-pattern of <pattern>.  The first item in <replacements> carries the indices for the first parenthesized sub-pattern, the second item carries those for the second sub-pattern, and so on.  If there are fewer than nine parenthesized sub-patterns in <pattern>, or if some sub-pattern was not used in the match, then the corresponding item in <replacements> is the list {0, -1}.  See the discussion of `%)' in `help regular-expressions', for more information on parenthesized sub-patterns.",
    "",
    "   match(\"foo\", \"f*o\")          =>  {1, 2, {{0, -1}, ...}, \"foo\"}",
    "   match(\"foo\", \"fo*\")          =>  {1, 3, {{0, -1}, ...}, \"foo\"}",
    "   match(\"foobar\", \"o*b\")       =>  {2, 4, {{0, -1}, ...}, \"foobar\"}",
    "   rmatch(\"foobar\", \"o*b\")      =>  {4, 4, {{0, -1}, ...}, \"foobar\"}",
    "   match(\"foobar\", \"f%(o*%)b\")  =>  {1, 4, {{2, 3}, {0, -1}, ...}, \"foobar\"}",
    "",
    "See `help regular-expressions' for information on the syntax and semantics of patterns."
  };
  property "max()" (owner: HACKER, flags: "rc") = {"*forward*", "min()"};
  property "max_object()" (owner: HACKER, flags: "rc") = {
    "Syntax:  max_object ()   => obj",
    "",
    "Returns the largest object number yet assigned to a created object.  Note that",
    "the object with this number may no longer exist; it may have been recycled.",
    "The next object created will be assigned the object number one larger than the",
    "value of `max_object()'."
  };
  property "memory_usage()" (owner: HACKER, flags: "rc") = {
    "Syntax:  memory_usage ()   => list",
    "",
    "On some versions of the server, this returns statistics concerning the server",
    "consumption of system memory.  The result is a list of lists, each in the",
    "following format:",
    "",
    "    {<block-size>, <nused>, <nfree>}",
    "",
    "where <block-size> is the size in bytes of a particular class of memory",
    "fragments, <nused> is the number of such fragments currently in use in the",
    "server, and <nfree> is the number of such fragments that have been reserved",
    "for use but are currently free.",
    "",
    "On servers for which such statistics are not available, `memory_usage()'",
    "returns `{}'.  The compilation option `USE_SYSTEM_MALLOC' controls",
    "whether or not statistics are available; if the option is provided, statistics",
    "are not available."
  };
  property "min()" (owner: HACKER, flags: "rc") = {
    "Syntax:  min (num <x>, ...)   => num",
    "         max (num <x>, ...)   => num",
    "",
    "These two functions return the smallest or largest of their arguments, respectively.  All of the arguments must be numbers of the same kind (i.e., either integer or floating-point); otherwise E_TYPE is raised."
  };
  property "move()" (owner: HACKER, flags: "rc") = {
    "Syntax:  move (OBJ <what>, OBJ <where>)   => none",
    "",
    "Changes <what>'s location to be <where>.  This is a complex process because a number of permissions checks and notifications must be performed. The actual movement takes place as described in the following paragraphs.",
    "",
    "<what> should be a valid object and <where> should be either a valid object or `#-1' (denoting a location of 'nowhere'); otherwise E_INVARG is raised.  The programmer must be either the owner of <what> or a wizard; otherwise, E_PERM is raised.",
    "",
    "If <where> is a valid object, then the verb-call",
    "",
    "    <where>:accept(<what>)",
    "",
    "is performed before any movement takes place.  If the verb returns a false value and the programmer is not a wizard, then <where> is considered to have refused entrance to <what>; `move()' raises E_NACC.  If <where> does not define an `accept' verb, then it is treated as if it defined one that always returned false.",
    "",
    "If moving <what> into <where> would create a loop in the containment hierarchy (i.e., <what> would contain itself, even indirectly), then E_RECMOVE is raised instead.",
    "",
    "The `location' property of <what> is changed to be <where>, and the `contents' properties of the old and new locations are modified appropriately.  Let <old-where> be the location of <what> before it was moved.  If <old-where> is a valid object, then the verb-call",
    "",
    "    <old-where>:exitfunc(<what>)",
    "",
    "is performed and its result is ignored; it is not an error if <old-where> does not define a verb named `exitfunc'.  Finally, if <where> and <what> are still valid objects, and <where> is still the location of <what>, then the verb-call",
    "",
    "    <where>:enterfunc(<what>)",
    "",
    "is performed and its result is ignored; again, it is not an error if <where> does not define a verb named `enterfunc'."
  };
  property "notify()" (owner: HACKER, flags: "rc") = {
    "Syntax:  notify (OBJ conn, STR string [, no-flush]) => 0 or 1",
    "",
    "Enqueues <string> for output (on a line by itself) on the connection <conn>. If the programmer is not <conn> or a wizard, then E_PERM is raised. If <conn> is not a currently-active connection, then this function does nothing. Output is normally written to connections only between tasks, not during execution.",
    "",
    "The server will not queue an arbitrary amount of output for a connection; the `MAX_QUEUED_OUTPUT' compilation option (in `options.h') controls the limit. When an attempt is made to enqueue output that would take the server over its limit, it first tries to write as much output as possible to the connection without having to wait for the other end. If that doesn't result in the new output being able to fit in the queue, the server starts throwing away the oldest lines in the queue until the new output will fit. The server remembers how many lines of output it has `flushed' in this way and, when next it can succeed in writing anything to the connection, it first writes a line like `>> Network buffer overflow; X lines of output to you have been lost <<' where <X> is the number of of flushed lines.",
    "",
    "If <no-flush> is provided and true, then `notify()' never flushes any output from the queue; instead it immediately returns false. `Notify()' otherwise always returns true."
  };
  property "object_bytes()" (owner: HACKER, flags: "rc") = {
    "Syntax:  object_bytes (OBJ object)   => INT",
    "",
    "Returns the number of bytes of the server's memory required to store the given <object>, including the space used by the values of all its non-clear properties and by the verbs and properties defined directly on the object. Raises E_INVARG if <object> is not a valid object and E_PERM if the programmer is not a wizard."
  };
  property "open_network_connection()" (owner: HACKER, flags: "rc") = {
    "Syntax:  open_network_connection (<value>, ... [, <listener>])   => obj",
    "",
    "Establishes a network connection to the place specified by the arguments and more-or-less pretends that a new, normal player connection has been established from there.  The new connection, as usual, will not be logged in initially and will have a negative object number associated with it for use with `read()', `notify()', and `boot_player()'.  This object number is the value returned by this function.",
    "",
    "If the programmer is not a wizard or if the `OUTBOUND_NETWORK' compilation option was not used in building the server, then `E_PERM' is raised.  If the network connection cannot be made for some reason, then other errors will be returned, depending upon the particular network implementation in use.",
    "",
    "For the TCP/IP network implementations (the only ones as of this writing that support outbound connections), there must be at least two arguments, a string naming a host (possibly using the numeric Internet syntax) and an integer specifying a TCP port.  If a connection cannot be made because the host does not exist, the port does not exist, the host is not reachable or refused the connection, `E_INVARG' is raised.  If the connection cannot be made for other reasons, including resource limitations, then `E_QUOTA' is raised.",
    "",
    "Beginning with version 1.8.3, an optional third argument may be supplied, `listener' must be an object, whose listening verbs will be called at appropriate points.  See the description in `listen()' for more details.",
    "",
    "The outbound connection process involves certain steps that can take quite a long time, during which the server is not doing anything else, including responding to user commands and executing MOO tasks.  See the chapter on server assumptions about the database for details about how the server limits the amount of time it will wait for these steps to successfully complete.",
    "",
    "It is worth mentioning one tricky point concerning the use of this function.  Since the server treats the new connection pretty much like any normal player connection, it will naturally try to parse any input from that connection as commands in the usual way.  To prevent this treatment, you should use `set_connection_option()' to set the `hold-input' option true on the connection."
  };
  property "output_delimiters()" (owner: HACKER, flags: "rc") = {
    "Syntax:  output_delimiters (OBJ <player>)   => LIST",
    "",
    "Returns a list of two strings, the current \"output prefix\" and \"output suffix\" for <player>.  If <player> does not have an active network connection, then E_INVARG is raised.  If either string is currently undefined, the value `\"\"' is used instead.  See the discussion of the `PREFIX' and `SUFFIX' commands in the LambdaMOO Programmers Manual for more information about the output prefix and suffix."
  };
  property "parent()" (owner: HACKER, flags: "rc") = {
    "Syntax:  parent (OBJ <object>)   => OBJ",
    "       children (OBJ <object>)   => LIST",
    "",
    "These functions return the parent and a list of the children of <object>, respectively.  If <object> is not valid, then E_INVARG is raised."
  };
  property "pass()" (owner: HACKER, flags: "rc") = {
    "Syntax:  pass (<arg>, ...)   => value",
    "",
    "Often, it is useful for a child object to define a verb that *augments* the behavior of a verb on its parent object. For example, the root object (an ancestor of every other object) defines a :description() verb that simply returns the value of `this.description'; this verb is used by the implementation of the `look' command. In many cases, a programmer would like the description of some object to include some non-constant part; for example, a sentence about whether or not the object was `awake' or `sleeping'.  This sentence should be added onto the end of the normal description.  The programmer would like to have a means of calling the normal `description' verb and then appending the sentence onto the end of that description.  The function `pass()' is for exactly such situations.",
    "",
    "`Pass()' calls the verb with the same name as the current verb but as defined on the parent of the object that defines the current verb.  The arguments given to the called verb are the ones given to pass() and the returned value of the called verb is returned from the call to pass(). The initial value of `this' in the called verb is the same as in the calling verb.",
    "",
    "Thus, in the example above, the child-object's :description() verb might have the following implementation:",
    "",
    "    return pass(@args) + \"  It is \" + (this.awake ? \"awake.\" | \"sleeping.\");",
    "",
    "That is, it calls its parent's :description() verb and then appends to the result a sentence whose content is computed based on the value of a property on the object.",
    "",
    "In the above example, `pass()' would have worked just as well, since :description() is not normally given any arguements.  However, it is a good idea to get into the habit of using `pass(@args)' rather than `pass(args[1])' or `pass()' even if the verb being pass()ed to is already known to take a set number of arguments or none at all.  For one thing, though the args may be irrelevant to the code that you've written, it may be that the corresponding verb on the parent has been rewritten to take additional arguments, in which case you will want your verb to continue to work..."
  };
  property "players()" (owner: HACKER, flags: "rc") = {
    "Syntax:  players ()   => list",
    "",
    "Returns a list of the object numbers of all player objects in the database."
  };
  property "properties()" (owner: HACKER, flags: "rc") = {
    "Syntax:  properties (OBJ <object>)   => LIST",
    "",
    "Returns a list of the names of the properties defined directly on the given <object>, not inherited from its parent.  If <object> is not valid, then E_INVARG is raised.  If the programmer does not have read permission on <object>, then E_PERM is raised."
  };
  property "property_info()" (owner: HACKER, flags: "rc") = {
    "Syntax:  property_info (OBJ <object>, STR <prop-name>)   => LIST",
    "     set_property_info (OBJ <object>, STR <prop-name>, LIST <info>)   => none",
    "",
    "These two functions get and set (respectively) the owner and permission bits for the property named <prop-name> on the given <object>.  If <object> is not valid, then E_INVARG is raised.  If <object> has no non-built-in property named <prop-name>, then E_PROPNF is raised.  If the programmer does not have read (write) permission on the property in question, then `property_info()' (`set_property_info()') raises E_PERM.  Property info has the following form:",
    "",
    "    {<owner>, <perms> [, new-name]}",
    "",
    "where <owner> is an object and <perms> is a string containing only characters from the set `r', `w', and `c', and <new-name> is a string; <new-name> is never part of the value returned by `property_info()', but it may optionally be given as part of the value provided to `set_property_info()'.  This list is the kind of value returned by `property_info()' and expected as the third argument to `set_property_info()'; the latter function raises E_INVARG if <owner> is not valid or <perms> contains any illegal characters, or, when <new-name> is given, if <prop-name> is not defined directly on <object> or <new-name> names an existing property defined on <object> or any of its ancestors or descendants."
  };
  property "queue_info()" (owner: HACKER, flags: "rc") = {
    "queue_info([obj user])",
    "",
    "Returns the number of forked tasks that <user> has at the moment.  Since it doesn't say which tasks, security is not a significant issue.  If no argument is given, then gives a list of all users with task queues in the server.  (Essentially all connected players + all open connections + all users with tasks running in the background.)"
  };
  property "queued_tasks()" (owner: HACKER, flags: "rc") = {
    "Syntax:  queued_tasks ()   => LIST",
    "",
    "Returns information on each of the background tasks (i.e., forked, suspended, or reading)  owned by the programmer (or, if the programmer is a wizard, all queued tasks). The returned value is a list of lists, each of which encodes certain information about a particular queued task in the following format:",
    "",
    "    {<task-id>, <start-time>, <ticks>, <clock-id>,",
    "     <programmer>, <verb-loc>, <verb-name>, <line>, <this>, <task-size>}",
    "",
    "where <task-id> is a numeric identifier for this queued task, <start-time> is the time after which this task will begin execution (in `time()' format), <ticks> is the number of ticks this task will have when it starts (always 20,000 now, though this is changeable. This makes this value obsolete and no longer interesting), <clock-id> is a number whose value is no longer interesting, <programmer> is the permissions with which this task will begin execution (and also the player who \"owns\" this task), <verb-loc> is the object on which the verb that forked this task was defined at the time, <verb-name> is that name of that verb, <line> is the number of the first line of the code in that verb that this task will execute, and <this> is the value of the variable `this' in that verb. For reading tasks, <start-time> is `-1'.  <task-size> is in bytes, and is the size of memory in use by the task for local variables, stack frames, etc.",
    "",
    "The <ticks> and <clock-id> fields are now obsolete and are retained only for backward-compatibility reasons.  They may disappear in a future version of the server."
  };
  property "raise()" (owner: HACKER, flags: "rc") = {
    "Syntax:  raise (code [, STR message [, value]])   => none",
    "",
    "Raises <code> as an error in the same way as other MOO expressions, statements, and functions do. <Message>, which defaults to the value `tostr(<code>)', and <value>, which defaults to zero, are made available to any `try-except' statements to catch the error. If the error is not caught, then <message> will appear on the first line of the traceback printed to the user."
  };
  property "random()" (owner: HACKER, flags: "rc") = {
    "Syntax:  random ([INT <mod>])   => INT",
    "",
    "<Mod> must be a positive integer; otherwise, E_INVARG is raised.  An integer is chosen randomly from the range `[1..<mod>]' and returned. If <mod> is not provided, it defaults to the largest MOO integer, 2147483647."
  };
  property "read()" (owner: HACKER, flags: "rc") = {
    "Syntax:  read ([OBJ <conn> [, non-blocking]])   => STR",
    "",
    "Reads and returns a line of input from the connection <conn> (or, if not provided, from the player that typed the command that initiated the current task). If <non-blocking> is false or not provided, this function suspends the current task, resuming it when there is input available to be read. If <non-blocking> is provided and true, this function never suspends the calling task; if there is no input currently available for input, `read()' simply returns 0 immediately.",
    "",
    "If <conn> is provided, then the programmer must either be a wizard or the owner of <conn>, if <conn> is not provided, then `read()' may only be called by a wizard and only in the task that was last spawned by a command from the connection in question. Otherwise, E_PERM is raised. If the given <conn> is not currently connected and has no pending lines of input, or if the connection is closed while a task is waiting for input but before any lines of input are received, then `read()' raises E_INVARG.",
    "",
    "The restriction on the use of `read()' without any arguments preserves the following simple invariant: if input is being read from a player, it is for the task started by the last command that the player typed. This invariant adds responsibility to the programmer, however. If your program calls another verb before doing a `read()', then either that verb must not suspend, or else you must arrange that no commands will be read from the connection in the meantime. The most straightforward way to do this is to call",
    "",
    "  set_connection_option(<conn>, \"hold-input\", 1)",
    "",
    "before any task suspension could happen, then make all of your calls to `read()' and other code that might suspend, and finally call",
    "",
    "  set_connection_option(<conn>, \"hold-input\", 0)",
    "",
    "to allow commands once again to be read and interpreted normally."
  };
  property "recycle()" (owner: HACKER, flags: "rc") = {
    "Syntax:  recycle (OBJ <object>)   => none",
    "",
    "The given <object> is destroyed, irrevocably.  The programmer must either own <object> or be a wizard; otherwise, E_PERM is raised.  If <object> is not valid, then E_INVARG is raised.  The children of <object> are reparented to the parent of <object>.  Before <object> is recycled, each object in its contents is moved to `#-1' (implying a call to <object>'s `exitfunc' verb, if any) and then <object>'s `recycle' verb, if any, is called with no arguments.",
    "",
    "After <object> is recycled, if the owner of the former object has a property named `ownership_quota' and the value of that property is a number, then `recycle()' treats that value as a \"quota\" and increments it by one, storing the result back into the `ownership_quota' property."
  };
  property "renumber()" (owner: HACKER, flags: "rc") = {
    "Syntax:  renumber (OBJ <object>)   => OBJ",
    "",
    "The object number of the object currently numbered <object> is changed to be the least nonnegative object number not currently in use and the new object number is returned.  If <object> is not valid, then E_INVARG is raised.  If the programmer is not a wizard, then E_PERM is raised. If there are no unused nonnegative object numbers less than <object>, then <object> is returned and no changes take place.",
    "",
    "The references to <object> in the parent/children and location/contents hierarchies are updated to use the new object number, and any verbs, properties and/or objects owned by <object> are also changed to be owned by the new object number.  The latter operation can be quite time consuming if the database is large.  No other changes to the database are performed; in particular, no object references in property values or verb code are updated.",
    "",
    "This operation is intended for use in making new versions of the LambdaCore database from the then-current LambdaMOO database, and other similar situations.  Its use requires great care."
  };
  property "reset_max_object()" (owner: HACKER, flags: "rc") = {
    "Syntax:  reset_max_object ()   => none",
    "",
    "The server's idea of the highest object number ever used is changed to be the highest object number of a currently-existing object, thus allowing reuse of any higher numbers that refer to now-recycled objects.  If the programmer is not a wizard, then E_PERM is raised.",
    "",
    "This operation is intended for use in making new versions of the LambdaCore database from the then-current LambdaMOO database, and other similar situations.  Its use requires great care."
  };
  property "resume()" (owner: HACKER, flags: "rc") = {
    "Syntax:  resume (INT task-id [, value])   => none",
    "",
    "Immediately ends the suspension of the suspended task with the given <task-id>; that task's call to `suspend()' will return <value>, which defaults to zero. `Resume()' raises E_INVARG if <task-id> does not specify an existing suspended task and E_PERM if the programmer is neither a wizard nor the owner of the specified task."
  };
  property "rindex()" (owner: HACKER, flags: "rc") = {"*forward*", "index()"};
  property "rmatch()" (owner: HACKER, flags: "rc") = {"*forward*", "match()"};
  property "seconds_left()" (owner: HACKER, flags: "rc") = {"*forward*", "ticks_left()"};
  property "server_log()" (owner: HACKER, flags: "rc") = {
    "Syntax:  server_log (STR <message> [, <is-error>])  => none",
    "",
    "The text in <message> is sent to the server log.  If the programmer is not a wizard, then E_PERM is raised.  If <is-error> is provided and true, then <message> is marked in the server log as an error."
  };
  property "server_version()" (owner: HACKER, flags: "rc") = {
    "Syntax:  server_version ()   => str",
    "",
    "Returns a string giving the version number of the MOO server in the following",
    "format:",
    "",
    "    \"<major>.<minor>.<release>\"",
    "",
    "where <major>, <minor>, and <release> are all decimal numbers.",
    "",
    "The major version number changes very slowly, only when existing MOO code might",
    "stop working, due to an incompatible change in the syntax or semantics of the",
    "programming language, or when an incompatible change is made to the database",
    "format.",
    "",
    "The minor version number changes more quickly, whenever an upward-compatible",
    "change is made in the programming language syntax or semantics.  The most",
    "common cause of this is the addition of a new kind of expression, statement, or",
    "built-in function.",
    "",
    "The release version number changes as frequently as bugs are fixed in the",
    "server code.  Changes in the release number indicate changes that should only",
    "be visible to users as bug fixes, if at all."
  };
  property "set_connection_option()" (owner: HACKER, flags: "rc") = {
    "Syntax:  set_connection_option (OBJ conn, STR option, value)   => none",
    "",
    "Controls a number of optional behaviors associated the connection <conn>. Raises `E_INVARG' if <conn> does not specify a current connection and `E_PERM' if the programmer is neither <conn> nor a wizard. Unless otherwise specified below, options can only be set (<value> is true) or unset (otherwise). The following values for <option> are currently supported:  ",
    "",
    "\"binary\"",
    "   When set, the connection is in `binary mode', in which case both input from and output to <conn> can contain arbitrary bytes. Input from a connection in binary mode is not broken into lines at all; it is delivered to either the read() function or normal command parsing as `binary strings', in whatever size chunks come back from the operating system. (See the early section in the LambdaMOO Programmers Manual on MOO value types for a description of the binary string representation.) For output to a connection in binary mode, the second argument to `notify()' must be a binary string; if it is malformed, `E_INVARG' is raised.",
    "",
    "   Fine point: If the connection mode is changed at any time when there is pending input on the connection, said input will be delivered as per the previous mode (i.e., when switching out of binary mode, there may be pending ``lines'' containing tilde-escapes for embedded linebreaks, tabs, tildes and other characters; when switching into binary mode, there may be pending lines containing raw tabs and from which nonprintable characters have been silently dropped as per normal mode. Only during the initial invocation of `$do_login_command()' on an incoming connection or immediately after the call to `open_network_connection()' that creates an outgoing connection is there guaranteed not to be pending input. At other times you will probably want to flush any pending input immediately after changing the connection mode.",
    "",
    "\"hold-input\"",
    "   When set, no input received on <conn> will be treated as a command; instead, all input remains in the queue until retrieved by calls to `read()' or until this connection option is unset, at which point command processing resumes. Processing of out-of-band input lines is unaffected by this option.",
    "",
    "\"disable-oob\"",
    "   When set, disables all out of band processing. All subsequent input lines until the next command that unsets this option will be made available for reading tasks or normal command parsing exactly as if the out-of-band prefix and the out-of-band quoting prefix had not been defined for this server.",
    "",
    "\"client-echo\"",
    "   The setting of this option is of no significance to the server. However calling `set_connection_option()' for this option sends the Telnet Protocol `WONT ECHO' or `WILL ECHO' command, depending on whether <value> is true or false, respectively. For clients that support the Telnet Protocol, this should toggle whether or not the client echoes locally the characters typed by the user. Note that the server itself never echoes input characters under any circumstances. (This option is only available under the TCP/IP networking configurations.)",
    "",
    "\"flush-command\"",
    "   This option is string-valued. If <value> is a non-empty string, then it becomes the new `flush' command for this connection, by which the player can flush all queued input that has not yet been processed by the server. If the string is empty, then <conn> is set to have no flush command at all. `set_connection_option' also allows specifying a non-string <value> which is equivalent to specifying the empty string. The default value of this option can be set via the property `$server_options.default_flush_command'; see the chapter in the LambdaMOO Programmers Manual on server assumptions about the database for details.",
    "",
    "\"intrinsic-commands\"",
    "   This option value is a list of strings, each being the name of one of the available server intrinsic commands (see the section in the LambdaMOO Programmers Manual on Command Lines That Receive Special Treatment). Commands not on the list are disabled, i.e., treated as normal MOO commands to be handled by `$do_command' and/or the built-in command parser. `set_connection_option' also allows specifying an integer <value> which, if zero, is equivalent to specifying the empty list, and otherwise is taken to be the list of all available intrinsic commands (the default setting).  ",
    "",
    "   Thus, one way to make the verbname `PREFIX' available as an ordinary command is as follows:",
    "",
    "    set_connection_option(player, \"intrinsic-commands\",",
    "      setremove(connection_option(player, \"intrinsic-commands\"), \"PREFIX\"));",
    "",
    "   Note that `connection_option()' always returns the list, even if `set_connection_option' was previously called with a numeric value.  Thus,",
    "",
    "    save = connection_option(player,\"intrinsic-commands\");",
    "    set_connection_option(player, \"intrinsic-commands, 1);",
    "    full_list = connection_option(player,\"intrinsic-commands\");",
    "    set_connection_option(player,\"intrinsic-commands\", save);",
    "    return full_list;",
    "",
    "   is a way of getting the full list of intrinsic commands available in the server while leaving the current connection unaffected."
  };
  property "set_player_flag()" (owner: HACKER, flags: "rc") = {
    "Syntax:  set_player_flag (OBJ <object>, <value>)   => none",
    "",
    "Confers or removes the ``player object'' status of the given <object>, depending upon the truth value of <value>.  If <object> is not valid, E_INVARG is raised.  If the programmer is not a wizard, then E_PERM is raised.",
    "",
    "If <value> is true, then <object> gains (or keeps) \"player object\" status: it will be an element of the list returned by `players()', the expression `is_player(<object>)' will return true, and users can connect to <object> by name when they log into the server.",
    "",
    "If <value> is false, the <object> loses (or continues to lack) \"player object\" status: it will not be an element of the list returned by `players()', the expression `is_player(<object>)' will return false, and users cannot connect to <object> by name when they log into the server.  In addition, if a user is connected to <object> at the time that it loses ``player object'' status, then that connection is immediately broken, just as if `boot_player(<object>)' had been called (see the description of `boot_player()' below)."
  };
  property "set_property_info()" (owner: HACKER, flags: "rc") = {"*forward*", "property_info()"};
  property "set_task_perms()" (owner: HACKER, flags: "rc") = {
    "Syntax:  set_task_perms (OBJ <player>)   => none",
    "",
    "Changes the permissions with which the currently-executing verb is running to be those of <player>.  If <player> is not of type OBJ, then E_INVARG is raised.  If the programmer is neither <player> nor a wizard, then E_PERM is raised.",
    "",
    "Note: This does not change the owner of the currently-running verb, only the permissions of this particular invocation.  It is used in verbs owned by wizards to make themselves run with lesser (usually non-wizard) permissions."
  };
  property "set_verb_args()" (owner: HACKER, flags: "rc") = {"*forward*", "verb_args()"};
  property "set_verb_code()" (owner: HACKER, flags: "rc") = {"*forward*", "verb_code()"};
  property "set_verb_info()" (owner: HACKER, flags: "rc") = {"*forward*", "verb_info()"};
  property "setadd()" (owner: HACKER, flags: "rc") = {
    "Syntax:  setadd (LIST <list>, <value>)   => LIST",
    "      setremove (LIST <list>, <value>)   => LIST",
    "",
    "Returns a copy of <list> with the given <value> added or removed, as appropriate.  `Setadd()' only adds <value> if it is not already an element of <list>; <list> is thus treated as a mathematical set. <value> is added at the end of the resulting list, if at all.  Similarly, `setremove()' returns a list identical to <list> if <value> is not an element.  If <value> appears more than once in <list>, only the first occurrence is removed in the returned copy.",
    "",
    "    setadd({1, 2, 3}, 3)         =>   {1, 2, 3}",
    "    setadd({1, 2, 3}, 4)         =>   {1, 2, 3, 4}",
    "    setremove({1, 2, 3}, 3)      =>   {1, 2}",
    "    setremove({1, 2, 3}, 4)      =>   {1, 2, 3}",
    "    setremove({1, 2, 3, 2}, 2)   =>   {1, 3, 2}"
  };
  property "setremove()" (owner: HACKER, flags: "rc") = {"*forward*", "setadd()"};
  property "shutdown()" (owner: HACKER, flags: "rc") = {
    "Syntax:  shutdown ([STR <message>])   => none",
    "",
    "Requests that the server shut itself down at its next opportunity.  Before doing so, the given <message> is printed to all connected players.  If the programmer is not a wizard, then E_PERM is raised."
  };
  property "sin()" (owner: HACKER, flags: "rc") = {
    "Syntax:  cos (FLOAT x)   => FLOAT",
    "         sin (FLOAT x)   => FLOAT",
    "         tan (FLOAT x)   => FLOAT",
    "",
    "Returns the cosine, sine, or tangent of <x>, respectively."
  };
  property "sinh()" (owner: HACKER, flags: "rc") = {
    "Syntax:  cosh (FLOAT x)   => FLOAT",
    "         sinh (FLOAT x)   => FLOAT",
    "         tanh (FLOAT x)   => FLOAT",
    "",
    "Returns the hyperbolic cosine, sine, or tangent of <x>, respectively."
  };
  property "sqrt()" (owner: HACKER, flags: "rc") = {
    "Syntax:  sqrt (FLOAT <x>)  => FLOAT",
    "",
    "Returns the square root of <x>.  If <x> is negative, then E_INVARG is raised."
  };
  property "strcmp()" (owner: HACKER, flags: "rc") = {
    "Syntax:  strcmp (STR <str1>, STR <str2>)   => INT",
    "",
    "Performs a case-sensitive comparison of the two argument strings.  If <str1> is lexicographically less than <str2>, the `strcmp()' returns a negative number.  If the two strings are identical, `strcmp()' returns zero.  Otherwise, `strcmp()' returns a positive number.  The ASCII character ordering is used for the comparison."
  };
  property "string_hash()" (owner: HACKER, flags: "rc") = {"*forward*", "binary_hash()"};
  property "strsub()" (owner: HACKER, flags: "rc") = {
    "Syntax:  strsub (STR <subject>, STR <what>, STR <with> [, <case-matters>])   => STR",
    "",
    "Replaces all occurrences in <subject> of <what> with <with>, performing string substitution.  The occurrences are found from left to right and all substitutions happen simultaneously.  By default, occurrences of <what> are searched for while ignoring the upper/lower case distinction. If <case-matters> is provided and true, then case is treated as significant in all comparisons.",
    "",
    "    strsub(\"%n is a fink.\", \"%n\", \"Fred\")   =>   \"Fred is a fink.\"",
    "    strsub(\"foobar\", \"OB\", \"b\")             =>   \"fobar\"",
    "    strsub(\"foobar\", \"OB\", \"b\", 1)          =>   \"foobar\""
  };
  property "substitute()" (owner: HACKER, flags: "rc") = {
    "Syntax:  substitute (STR <template>, LIST <subs>)  => STR",
    "",
    "Performs a standard set of substitutions on the string <template>, using the information contained in <subs>, returning the resulting, transformed <template>.  <Subs> should be a list like those returned by `match()' or `rmatch()' when the match succeeds.",
    "",
    "In <template>, the strings `%1' through `%9' will be replaced by the text matched by the first through ninth parenthesized sub-patterns when `match()' or `rmatch()' was called.  The string `%0' in <template> will be replaced by the text matched by the pattern as a whole when `match()' or `rmatch()' was called. The string '%%' will be replaced by a single '%' sign. If '%' appears in <template> followed by any other character, E_INVARG will be raised.",
    "",
    "     subs = match(\"*** Welcome to LambdaMOO!!!\", \"%(%w*%) to %(%w*%)\");",
    "     substitute(\"I thank you for your %1 here in %2.\", subs)",
    "             =>   \"I thank you for your Welcome here in LambdaMOO.\""
  };
  property "suspend()" (owner: HACKER, flags: "rc") = {
    "Syntax:  suspend ([INT <seconds>])   => value",
    "",
    "Suspends the current task, and resumes it after at least <seconds> seconds. (If <seconds> is not provided, the task is suspended indefinitely; such a task can only be resumed by use of the `resume()' function.) When the task is resumed, it will have a full quota of ticks and seconds.  This function is useful for programs that run for a long time or require a lot of ticks.  If <seconds> is negative, then E_INVARG is raised. `Suspend()' returns zero unless it was resumed via `resume()' in which case it returns the second argument given to that function.",
    "",
    "In some sense, this function forks the `rest' of the executing task.  However, there is a major difference between the use of `suspend(<seconds>)' and the use of the `fork (<seconds>)'.  The `fork' statement creates a new task (a \"forked task\") while the currently-running task still goes on to completion, but a `suspend()' suspends the currently-running task (thus making it into a \"suspended task\").  This difference may be best explained by the following examples, in which one verb calls another:",
    "",
    "    .program   #0:caller_A",
    "    #0.prop = 1;",
    "    #0:callee_A();",
    "    #0.prop = 2;",
    "    .",
    "",
    "    .program   #0:callee_A",
    "    fork(5)",
    "      #0.prop = 3;",
    "    endfork",
    "    .",
    "",
    "    .program   #0:caller_B",
    "    #0.prop = 1;",
    "    #0:callee_B();",
    "    #0.prop = 2;",
    "    .",
    "",
    "    .program   #0:callee_B",
    "    suspend(5);",
    "    #0.prop = 3;",
    "    .",
    "",
    "Consider `#0:caller_A', which calls `#0:callee_A'.  Such a task would assign 1 to `#0.prop', call `#0:callee_A', fork a new task, return to `#0:caller_A', and assign 2 to `#0.prop', ending this task.  Five seconds later, if the forked task had not been killed, then it would begin to run; it would assign 3 to `#0.prop' and then stop.  So, the final value of `#0.prop' (i.e., the value after more than 5 seconds) would be 3.",
    "",
    "Now consider `#0:caller_B', which calls `#0:callee_B' instead of `#0:callee_A'.  This task would assign 1 to `#0.prop', call `#0:callee_B', and suspend.  Five seconds later, if the suspended task had not been killed, then it would resume; it would assign 3 to `#0.prop', return to `#0:caller', and assign 2 to `#0.prop', ending the task. So, the final value of `#0.prop' (i.e., the value after more than 5 seconds) would be 2.",
    "",
    "A suspended task, like a forked task, can be described by the `queued_tasks()' function and killed by the `kill_task()' function. Suspending a task does not change its task id.  A task can be suspended again and again by successive calls to `suspend()'.",
    "",
    "Once `suspend()' has been used in a particular task, then the `read()' function will always raise E_PERM in that task.  For more details, see the description of `read()'.",
    "",
    "By default, there is no limit to the number of tasks any player may suspend, but such a limit can be imposed from within the database. See the chapter in the LambdaMOO Programmers Manual on server assumptions about the database for details."
  };
  property "tan()" (owner: HACKER, flags: "rc") = {"*forward*", "sin()"};
  property "tanh()" (owner: HACKER, flags: "rc") = {"*forward*", "sinh()"};
  property "task_id()" (owner: HACKER, flags: "rc") = {
    "Syntax:  task_id ()   => INT",
    "",
    "Returns the numeric identifier for the currently-executing task.  Such numbers are randomly selected for each task and can therefore safely be used in circumstances where unpredictability is required."
  };
  property "task_stack()" (owner: HACKER, flags: "rc") = {
    "Syntax:  task_stack (INT task-id [, include-line-numbers])  => LIST",
    "",
    "Returns information like that returned by the `callers()' function, but for the suspended task with the given <task-id>; the <include-line-numbers> argument has the same meaning as in `callers()'. Raises E_INVARG if <task-id> does not specify an existing suspended task and E_PERM if the programmer is neither a wizard nor the owner of the specified task."
  };
  property "ticks_left()" (owner: HACKER, flags: "rc") = {
    "Syntax:  ticks_left ()   => INT",
    "       seconds_left ()   => INT",
    "",
    "These two functions return the number of ticks or seconds (respectively) left to the current task before it will be forcibly terminated.  These are useful, for example, in deciding when to fork another task to continue a long-lived computation."
  };
  property "time()" (owner: HACKER, flags: "rc") = {
    "Syntax:  time ()   => INT",
    "",
    "Returns the current time, represented as the number of seconds that have elapsed since midnight on 1 January 1970, Greenwich Mean Time."
  };
  property "tofloat()" (owner: HACKER, flags: "rc") = {
    "Syntax:  tofloat (value)   => FLOAT",
    "",
    "Converts the given MOO value into a floating-point number and returns that number. Integers and objects numbers are converted into the corresponding integral floating-point numbers. Strings are parsed as the decimal encoding of a real number which is then represented as closely as possible as a floating-point number. Errors are first converted to integers as in `toint()' and then converted as integers are. `Tofloat()' raises E_TYPE if <value> is a LIST. If <value> is a string but the string does not contain a syntactically-correct number, then `tofloat()' raises E_INVARG.",
    "",
    "  tofloat(34)       =>  34.0",
    "  tofloat(#34)      =>  34.0",
    "  tofloat(\"34\")     =>  34.0",
    "  tofloat(\"34.7\")   =>  34.7",
    "  tofloat(E_TYPE)   =>  1.0"
  };
  property "toint()" (owner: HACKER, flags: "rc") = {"*forward*", "tonum()"};
  property "toliteral()" (owner: HACKER, flags: "rc") = {
    "Syntax:  toliteral (<value>)   => STR",
    "",
    "Returns a string containing a MOO literal expression that, when evaluated, would be equal to <value>. If no arguments or more than one argument is given, E_ARGS is raised.",
    "",
    "Examples:",
    "toliteral(43)                       =>  \"43\"",
    "toliteral(1.0/3.0)                  =>  \"0.33333333333333\"",
    "toliteral(#17)                      =>  \"#17\"",
    "toliteral(E_PERM)                   =>  \"E_PERM\"",
    "toliteral({\"A\", \"B\", {\"C\", 123}})   =>  \"{\\\"A\\\", \\\"B\\\", {\\\"C\\\", 123}}\""
  };
  property "tonum()" (owner: HACKER, flags: "rc") = {
    "Syntax:  toint (<value>)   => INT",
    "         tonum (<value>)   => INT",
    "",
    "Converts the given MOO value into an integer and returns that integer. Floating-point numbers are rounded toward zero, truncating their fractional parts. Object numbers are converted into the equivalent integers, strings are parsed as the decimal encoding of a real number which is then converted to an integer. Errors are converted into integers obeying the same ordering (with respect to `<=' as the errors themselves.) `Toint()' raises E_TYPE if <value> is a LIST.  If <value> is a string but the string does not contain a syntactically-correct number, then `toint()' returns 0.",
    "",
    "    toint(34.7)        =>   34",
    "    toint(-34.7)       =>   34",
    "    toint(#34)         =>   34",
    "    toint(\"34\")        =>   34",
    "    toint(\"34.7\")      =>   34",
    "    toint(\" - 34  \")   =>  -34",
    "    toint(E_TYPE)      =>    1"
  };
  property "toobj()" (owner: HACKER, flags: "rc") = {
    "Syntax:  toobj (<value>)   => OBJ",
    "",
    "Converts the given MOO value into an object number and returns that object number.  The conversions are very similar to those for `toint()' except that for strings, the number *may* be preceded by `#'.",
    "",
    "    toobj(\"34\")       =>   #34",
    "    toobj(\"#34\")      =>   #34",
    "    toobj(\"foo\")      =>   #0",
    "    toobj({1, 2})     -error->   E_TYPE"
  };
  property "tostr()" (owner: HACKER, flags: "rc") = {
    "Syntax:  tostr (<value>, ...)   => STR",
    "",
    "Converts all of the given MOO values into strings and returns the concatenation of the results.",
    "",
    "    tostr(17)                  =>   \"17\"",
    "    tostr(1.0/3.0)             =>   \"0.333333333333333\"",
    "    tostr(#17)                 =>   \"#17\"",
    "    tostr(\"foo\")               =>   \"foo\"",
    "    tostr({1, 2})              =>   \"{list}\"",
    "    tostr(E_PERM)              =>   \"Permission denied\"",
    "    tostr(\"3 + 4 = \", 3 + 4)   =>   \"3 + 4 = 7\"",
    "",
    "Note that `tostr()' does not do a good job of converting lists into strings; all lists, including the empty list, are converted into the string `\"{list}\"'. The function `toliteral()' is better for this purpose."
  };
  property "trunc()" (owner: HACKER, flags: "rc") = {
    "Syntax:  trunc (FLOAT <x>)   => FLOAT",
    "",
    "Returns the integer obtained by truncating <x> at the decimal point, as a floating-point number. For negative <x>, this is equivalent to `ceil()'; otherwise, it is equivalent to `floor()'."
  };
  property "typeof()" (owner: HACKER, flags: "rc") = {
    "Syntax:  typeof (<value>)   => INT",
    "",
    "Takes any MOO value and returns a number representing the type of <value>. The result is the same as the initial value of one of these built-in variables: `INT', `FLOAT', `STR', `LIST', `OBJ', or `ERR'.  Thus, one usually writes code like this:",
    "",
    "    if (typeof(x) == LIST) ...",
    "",
    "and not like this:",
    "",
    "    if (typeof(x) == 4) ...",
    "",
    "because the former is much more readable than the latter."
  };
  property "unlisten()" (owner: HACKER, flags: "rc") = {
    "Syntax:  unlisten (<canon>)   => none",
    "",
    "Stop listening for connections on the point described by <canon>, which should be the second element of some element of the list returned by `listeners()'. Raises E_PERM if the programmer is not a wizard and E_INVARG if there does not exist a listener with that description."
  };
  property "valid()" (owner: HACKER, flags: "rc") = {
    "Syntax:  valid (OBJ <object>)   => INT",
    "",
    "Returns a non-zero integer (i.e., a true value) if <object> is a valid object (one that has been created and not yet recycled) and zero (i.e., a false value) otherwise.",
    "",
    "    valid(#0)    =>   1",
    "    valid(#-1)   =>   0"
  };
  property "value_bytes()" (owner: HACKER, flags: "rc") = {
    "Syntax:  value_bytes(<value>)   => INT",
    "",
    "Returns the number of bytes of the server's memory required to store the given <value>."
  };
  property "value_hash()" (owner: HACKER, flags: "rc") = {
    "Syntax:  value_hash (<value>)   => STR",
    "",
    "Returns the same string as `string_hash(toliteral(<value>))'; see the description of `string_hash()' for details."
  };
  property "verb_args()" (owner: HACKER, flags: "rc") = {
    "Syntax:  verb_args (OBJ <object>, STR <verb-name>)   => LIST",
    "     set_verb_args (OBJ <object>, STR <verb-name>, LIST <args>)   => none",
    "",
    "These two functions get and set (respectively) the direct-object, preposition, and indirect-object specifications for the verb named <verb-name> on the given <object>.  If <object> is not valid, then E_INVARG is raised.  If <object> does not define a verb named <verb-name>, then E_VERBNF is raised.  If the programmer does not have read (write) permission on the verb in question, then `verb_args()' (`set_verb_args()') raises E_PERM.  Verb args specifications have the following form:",
    "",
    "    {<dobj>, <prep>, <iobj>}",
    "",
    "where <dobj> and <iobj> are strings drawn from the set `\"this\"', `\"none\"', and `\"any\"', and <prep> is a string that is either `\"none\"', `\"any\"', or one of the prepositional phrases listed much earlier in the description of verbs in the first chapter.  This is the kind of value returned by `verb_info()' and expected as the third argument to `set_verb_info()'.  Note that for `set_verb_args()', <prep> must be only one of the prepositional phrases, not (as is shown in that table) a set of such phrases separated by `/' characters.  `Set_verb_args()' raises E_INVARG if any of the <dobj>, <prep>, or <iobj> strings is illegal.",
    "",
    "    verb_args($container, \"take\")",
    "                        =>   {\"any\", \"out of/from inside/from\", \"this\"}",
    "    set_verb_args($container, \"take\", {\"any\", \"from\", \"this\"})"
  };
  property "verb_cache_stats()" (owner: HACKER, flags: "rc") = {
    "Syntax:  verb_cache_stats ()   => LIST",
    "         log_cache_stats ()    => none",
    "",
    "As of version 1.8.1, the server caches verbname-to-program lookups to improve performance.  These functions respectively return or write to the server log file the current cache statistics.  For `verb_cache_stats' the return value will be a list of the form",
    "",
    "    {<hits>, <negative_hits>, <misses>, <table_clears>, <histogram>}",
    "",
    "though this may change in future server releases.  The cache is invalidated by any builtin function call that may have an effect on verb lookups (e.g., `delete_verb()')."
  };
  property "verb_code()" (owner: HACKER, flags: "rc") = {
    "Syntax:  verb_code (OBJ <object>, STR <verb-name> [, <fully-paren> [, <indent>]])   => LIST",
    "     set_verb_code (OBJ <object>, STR <verb-name>, LIST <code>)   => LIST",
    "",
    "These functions get and set (respectively) the MOO-code program associated with the verb named <verb-name> on <object>.  The program is represented as a list of strings, one for each line of the program; this is the kind of value returned by `verb_code()' and expected as the third argument to `set_verb_code()'.  For `verb_code()', the expressions in the returned code are usually written with the minimum-necessary parenthesization; if <fully-paren> is true, then all expressions are fully parenthesized. Also for `verb_code()', the lines in the returned code are usually not indented at all; if <indent> is true, each line is indented to better show the nesting of statements.",
    "",
    "If <object> is not valid, then E_INVARG is raised.  If <object> does not define a verb named <verb-name>, then E_VERBNF is raised.  If the programmer does not have read (write) permission on the verb in question, then `verb_code()' (`set_verb_code()') raises E_PERM.  If the programmer is not, in fact, a programmer, then E_PERM is raised.",
    "",
    "For `set_verb_code()', the result is a list of strings, the error messages generated by the MOO-code compiler during processing of <code>.  If the list is non-empty, then `set_verb_code()' did not install <code>; the program associated with the verb in question is unchanged."
  };
  property "verb_info()" (owner: HACKER, flags: "rc") = {
    "Syntax:  verb_info (OBJ <object>, STR <verb-name>)   => LIST",
    "     set_verb_info (OBJ <object>, STR <verb-name>, LIST <info>)   => none",
    "",
    "These two functions get and set (respectively) the owner, permission bits, and name(s) for the verb named <verb-name> on the given <object>.  If <object> is not valid, then E_INVARG is raised.  If <object> does not define a verb named <verb-name>, then E_VERBNF is raised. If the programmer does not have read (write) permission on the verb in question, then `verb_info()' (`set_verb_info()') raises E_PERM.  Verb info has the following form:",
    "",
    "    {<owner>, <perms>, <names>}",
    "",
    "where <owner> is an object, <perms> is a string containing only characters from the set `r', `w', `x', and `d', and <names> is a string.  This is the kind of value returned by `verb_info()' and expected as the third argument to `set_verb_info()'. The latter function raises E_INVARG if <owner> is not valid, if <perms> contains any illegal characters, or if <names> is the empty string or consists entirely of spaces; it raises E_PERM if <owner> is not the programmer and the programmer is not a wizard."
  };
  property "verbs()" (owner: HACKER, flags: "rc") = {
    "Syntax:  verbs (OBJ <object>)   => LIST",
    "",
    "Returns a list of the names of the verbs defined directly on the given <object>, not inherited from its parent.  If <object> is not valid, then E_INVARG is raised.  If the programmer does not have read permission on <object>, then E_PERM is raised."
  };

  override aliases = {"Builtin Function Help"};
  override description = {
    "A help database (in the sense of anything that is usable by $player:help()) is any object having the following two verbs:",
    "",
    "  :find_topics(string)",
    "     returns a list of strings or some boolean false value.",
    "",
    "  :get_topic(string)",
    "     given one of the strings returned by :find_topics this either",
    "     returns a list of strings (text to be spewed to the player) or",
    "     returns 1 to indicate that it has already taken care of printing",
    "     information to the player.",
    "",
    "$player:help() consults any .help properties that exist on the player, its ancestors, player.location and its ancestors (in that order).  These properties are assumed to have values that are objects or lists of objects, each object itself assumed to be a help database in the above sense.  The main help database ($help) is placed at the end of the list of databases to be consulted.",
    "",
    "The Generic Help Database (this object) is the standard model help database of which the actual help database itself ($help) is an instance.  On help databases of this type, every help topic has a corresponding property, interpreted as follows:",
    "",
    "this.(topic) = string           - one-line help text.",
    "this.(topic) = {\"*verb*\",@args} - call this:verb(@args) to get text",
    "this.(topic) = any other list   - multi-line help text",
    "",
    "For the {\"*verb*\",...} form, the current verbs available are",
    "",
    "  {\"*forward*\", topic2, @rest}   ",
    "     - get topic2 help text and then append rest.  ",
    "       rest may, in turn, begin with a \"*verb*\"...",
    "",
    "  {\"*subst*\", @lines} ",
    "     - all occurences of %[exp] in lines are replaced with value of exp.  ",
    "       exp is assumed to evaluate to a string.  Evaluation is done using ",
    "       $no_one's permissions so exp can only refer to public information.",
    "",
    "  {\"*index*\"}",
    "     - returns a list of all topics in this database, arranged in columns."
  };
  override import_export_id = "builtin_function_help";
  override index_cache = {"builtin-index"};
  override object_size = {89775, 1084848672};
endobject