object BUILDER_HELP
  name: "Builder Help DB"
  parent: GENERIC_HELP
  owner: HACKER
  readable: true

  property "@add-entrance" (owner: HACKER, flags: "r") = {
    "Syntax:  @add-entrance <exit-object-number>",
    "",
    "Add the exit with the given object number as a recognized entrance to the current room (that is, one whose use is not considered teleportation).  Usually, @dig does this for you, but it doesn't if you don't own the room in question.  Instead, it tells you the object number of the new exit and you have to find the owner of the room and get them to use the @add-entrance command to link it up."
  };
  property "@add-exit" (owner: HACKER, flags: "r") = {
    "Syntax:  @add-exit <exit-object-number>",
    "",
    "Add the exit with the given object number as a conventional exit from the current room (that is, an exit that can be invoked simply by typing its name, like 'east').  Usually, @dig does this for you, but it doesn't if you don't own the room in question.  Instead, it tells you the object number of the new exit and you have to find the owner of the room and get them to use the @add-exit command to link it up."
  };
  property "@add-owned" (owner: HACKER, flags: "rc") = {
    "Syntax:  @add-owned <object>",
    "",
    "Adds an object to your .owned_objects property in case it managed not to get updated properly upon creation of that object.  Checks to ensure that the objects is really owned by you and otherwise belongs in your .owned_objects property.  See help @audit for more information."
  };
  property "@audit" (owner: HACKER, flags: "r") = {
    "Syntax:  @audit [<player>] [for <string>] [from <number>] [to <number>] ",
    "",
    "`@audit'        prints a report of all of the objects you own.",
    "`@audit player' prints the same report for another player.",
    "",
    "The `for' string restricts the search to objects whose names begin with that string.",
    "It is also possible to restrict the range of object numbers to include only those above a given number (`from') or below a given number (`to').",
    "",
    "All forms of @audit print a report:",
    "",
    "   #14 Gemba                          [The Pool]",
    "  #144 Popgun                         [Gemba]",
    " #1479 Cockatoo                      *[The Living Room]",
    " #1673 Bottom of Swimming Pool       ",
    " #2147 Cavern                        <-*west",
    " #2148 tunnel                         Bottom of Swimming ->Cavern",
    "",
    "The first column is the object's number, the second its name. The third column shows the object's location: Gemba is in The Pool, and is carrying the Popgun (#144).",
    "For exits, the third column shows source ->dest.",
    "For rooms, the third column shows any entrances owned by someone else.",
    "Object location, exit sources and destinations owned by another player are preceded by a *.",
    "",
    "@audit uses a property .owned_objects on the player, for speed.  This property is updated at the time of each object creation and destruction and ownership change.  The verb @auditdb (same args as @audit) actually searches through the entire database for objects.",
    "",
    "See also @verify-owned, @sort-owned, and @add-owned.",
    "",
    "See also @prospectus, which gives some additional information."
  };
  property "@build-options" (owner: HACKER, flags: "rc") = {
    "Syntax:  @build-option",
    "         @build-option <option>",
    "",
    "Synonyms:  @buildoption, @builder-option @builderoption",
    "",
    "The first form displays all of your builder options",
    "The second displays just that one option, which may be one of the flags listed below.  The builder options control various annoying details of your building commands (e.g., @create, ...)",
    "",
    "The remaining forms of this command are for setting your programmer options:",
    "",
    "         @build-option create_flags [is] <flags>",
    "         @build-option create_flags=<flags>",
    "         @build-option -create_flags",
    "                      (equivalent to create_flags=\"\")",
    "",
    "where flags is some substring of \"rwf\".  This option determines the read/write/fertility permissions of an object freshly created with @create or @recreate (see `help @create' and `help @recreate' and `help @chmod').  E.g., to make every object you create henceforth readable by default, do",
    "",
    "         @build-option create_flags=r",
    "",
    "For controlling the behavior of @dig, we have",
    "",
    "         @build-option  dig_room=<room>",
    "         @build-option  dig_room [is] <room>",
    "         @build-option -dig_room",
    "                      (equivalent to dig_room=$room)",
    "         @build-option  dig_exit=<exit>",
    "         @build-option  dig_exit [is] <exit>",
    "         @build-option -dig_exit",
    "                      (equivalent to dig_exit=$exit)",
    "",
    "The following respectively set and reset the specified flag option",
    "",
    "         @build-option +<option>",
    "         @build-option -<option>",
    "         @build-option !<option>           (equivalent to -<option>)",
    "",
    "Currently the only builder flag option available is",
    " -bi_create     @create/@recycle re-use object numbers.",
    " +bi_create     @create/@recycle call create()/recycle() directly ",
    "",
    "we prefer that you not use +bi_create, since this drives up the object numbers."
  };
  property "@builder-options" (owner: HACKER, flags: "rc") = {"*forward*", "@build-options"};
  property "@builderoptions" (owner: HACKER, flags: "rc") = {"*forward*", "@build-options"};
  property "@buildoptions" (owner: HACKER, flags: "rc") = {"*forward*", "@build-options"};
  property "@classes" (owner: HACKER, flags: "r") = {
    "Syntax:  @classes",
    "         @classes <class-name> ...",
    "",
    "The wizards have identified several useful classes of objects in the database.  The @classes command is used to see which classes exist and what their member objects are.",
    "",
    "The first form simply lists all of the defined classes along with short descriptions of the membership of each.",
    "",
    "The second form prints an indented listing of that subset of the object parent/child hierarchy containing the objects in the class(es) you specify."
  };
  property "@contents" (owner: HACKER, flags: "rc") = {
    "Syntax:  @contents object",
    "",
    "A quick way to find out the contents of an object.  Prints out the names and object numbers of all direct contents.  This can be useful when you need to refer to something by object number because something is wrong with its aliases.",
    "",
    "Example:",
    "  @contents here",
    "  The Entrance Hall(#19) contains:",
    "  Strasbourg Clock(#71)   mirror at about head height(#7444)"
  };
  property "@count" (owner: HACKER, flags: "r") = {
    "Syntax:  @count [player]",
    "",
    "Prints out the number of objects you or another person own.  Do not be surprised if this is one larger than you think it should be: remember that your player object is owned by you as well, even though you didn't create it in the usual way.",
    "",
    "If byte-based quota is enabled, also prints the total usage by all objects at last measurement."
  };
  property "@create" (owner: HACKER, flags: "r") = {
    "Syntax:  @create <class-name> named \"<names>\"",
    "         @create <parent-object> named \"<names>\"",
    "",
    "The main command for creating objects other than rooms and exits (for them, see 'help @dig'; it's much more convenient).",
    "",
    "The first argument specifies the 'parent' of the new object: loosely speaking, the 'kind' of object you're creating.  <class-name> is one of the four standard classes of objects: $note, $letter, $thing, or $container.  As time goes on, more 'standard classes' may be added.  If the parent you have in mind for your new object isn't one of these, you may use the parent's name (if it's in the same room as you) or else its object number (e.g., #4562).",
    "",
    "You may use \"called\" instead of \"named\" in this command, if you wish.",
    "",
    "An object must be fertile to be used as a parent-class.  See help @chmod for details.",
    "",
    "The <names> are given in the same format as in the @rename command:",
    "        <name-and-alias>,<alias>,...,<alias> [preferred]",
    "        <name>:<alias>,...,<alias> [not preferred]",
    "",
    "See 'help @rename' for a discussion of the difference between a name and an alias."
  };
  property "@dig" (owner: HACKER, flags: "rc") = {
    "Syntax:  @dig \"<new-room-name>\"",
    "         @dig <exit-spec> to \"<new-room-name>\"",
    "         @dig <exit-spec> to <old-room-object-number>",
    "",
    "This is the basic building tool.  The first form of the command creates a new room with the given name.  The new room is not connected to anywhere else; it is floating in limbo.  The @dig command tells you its object number, though, so you can use the @move command to get there easily.",
    "",
    "The second form of the command not only creates the room, but one or two exits linking your current location to (and possibly from) the new room.  An <exit-spec> has one of the following two forms:",
    "        <names>",
    "        <names>|<names>",
    "where the first form is used when you only want to create one exit, from your current room to the new room, and the second form when you also want an exit back, from the new room to your current room.  In any case, the <names> piece is just a list of names for the exit, separated by commas; these are the names of the commands players can type to use the exit.  It is usually a good idea to include explicitly the standard abbreviations for direction names (e.g., 'n' for 'north', 'se' for 'southeast', etc.).  DO NOT put spaces in the names of exits; they are useless in MOO.",
    "",
    "The third form of the command is just like the second form except that no new room is created; you instead specify by object number the other room to/from which the new exits will connect.",
    "",
    "NOTE: You must own the room at one end or the other of the exits you create.  If you own both, everything is hunky-dorey.  If you own only one end, then after creating the exits you should write down their object numbers.  You must then get the owner of the other room to use @add-exit and @add-entrance to link your new exits to their room.",
    "",
    "Examples:",
    "    @dig \"The Conservatory\"",
    "creates a new room named \"The Conservatory\" and prints out its object number.",
    "    @dig north,n to \"The North Pole\"",
    "creates a new room and also an exit linking the player's current location to the new room; players would say either 'north' or 'n' to get from here to the new room.  No way to get back from that room is created.",
    "    @dig west,w|east,e,out to \"The Department of Auto-Musicology\"",
    "creates a new room and two exits, one taking players from here to the new room (via the commands 'west' or 'w') and one taking them from the new room to here (via 'east', 'e', or 'out').",
    "    @dig up,u to #7164",
    "creates an exit leading from the player's current room to #7164, which must be an existing room."
  };
  property "@dump" (owner: #2, flags: "r") = {
    "Syntax:  @dump <object> [with [id=#<id>] [noprops] [noverbs] [create]]",
    "",
    "This spills out all the properties and verbs on an object, calling suspend at appropriate intervals.",
    "   id=#<id> -- specifies an idnumber to use in place of the object's actual id (for porting to another MOO)",
    "   noprops  -- don't show properties.",
    "   noverbs  -- don't show verbs.",
    "   create   -- indicates that a @create command should be generated and all of the verbs be introduced with @verb rather than @args; the default assumption is that the object already exists and you're just doing this to have a look at it."
  };
  property "@entrances" (owner: HACKER, flags: "rc") = {
    "Syntax:  @entrances",
    "",
    "Prints a list of all recognized entrances to the current room (but only if you own the room).  A recognized entrance is one whose use is not considered to be teleportation."
  };
  property "@exits" (owner: HACKER, flags: "rc") = {
    "Syntax:  @exits",
    "",
    "Prints a list of all conventional exits from the current room (but only if you own the room).  A conventional exit is one that can be used simply by typing its name, like 'east'."
  };
  property "@locations" (owner: HACKER, flags: "rc") = {
    "Syntax:  @locations object",
    "",
    "Prints out the names and object numbers of all containing objects.",
    "",
    "Example:",
    "  @locations ur-Rog",
    "  ur-Rog(#6349)   ur-Rog's Display Case(#6355)   Editorial Boardroom(#5747)"
  };
  property "@lock" (owner: HACKER, flags: "r") = {
    "Syntax:  @lock <object> with <key expression>",
    "",
    "Set a lock on <object> to restrict its use.  See 'help locking' for general information about locking and 'help keys' for the syntax and semantics of key expressions.",
    "",
    "N.B.  In the case of rooms, you are actually better off setting room.free_entry to 0 thus preventing teleportation and then @locking the various entrances.  The problem with @locking the room itself is that this can make it impossible to drop objects in the room."
  };
  property "@lock_for_open" (owner: HACKER, flags: "rc") = {
    "Syntax:",
    "  @lock_for_open <container> with <key expression>",
    "",
    "Set the lock on <container> which restricts who can open it.  See 'help locking' for general information about locking and 'help keys' for the syntax and semantics of key expressions.",
    "",
    "See 'help containers' for information on containers."
  };
  property "@measure" (owner: HACKER, flags: "rc") = {
    "Syntax:",
    "  @measure object <object name>",
    "  @measure summary [player]",
    "  @measure new [player]",
    "  @measure breakdown <object name>",
    "  @measure recent [number of days] [player]",
    "",
    "When the MOO is under byte-quota, objects in the MOO are measured approximately once a week, and the usage tally as reported by @quota is updated.  You may wish to measure an object specially, however, without waiting for the automatic measurement to take place, or if the MOO is under object-quota.  @measure has some subcommands to handle this.",
    "",
    "@measure object will measure an individual object right now, update the usage of that object in your usage as reported by @quota, and update the date of that object's measurement.",
    "",
    "@measure summary will go through your or another player's objects and produce the summary information that is printed by @quota.  Normally this will be just the same as @quota prints out, but occasionally the addition/subtraction done to keep @quota in sync will get out of date, and @measure summary will be needed.",
    "",
    "@measure new will go through all your or another player's objects, measuring exactly those which have never been measured before (that is, are too newly @created to have any measurement data).  This is necessary as any player is only permitted to own 10 unmeasured objects, or object creation will not be permitted.",
    "",
    "@measure breakdown will give you full information on where an object's size is coming from.  It will offer to moomail you the result.  Caution: don't forget to delete this message, as it is large and takes up a lot of space!",
    "",
    "@measure recent will let you re-measure objects of yours or another player's which have not been measured in the specified number of days (the default is the ordinary cycle of the measurement task)."
  };
  property "@opacity" (owner: HACKER, flags: "rc") = {
    "Syntax:",
    "  @opacity <container> is <integer>",
    "",
    "The opacity can take on one of three values:",
    "   0:  The container is transparent and you can always see into it.",
    "   1:  The container is opaque, and you cannot see into it when closed",
    "   2:  The container is a black hole, and you can never see into it whether closed or open.  ",
    "",
    "The default @opacity is 1."
  };
  property "@parents" (owner: HACKER, flags: "rc") = {
    "Syntax:  @parents object",
    "",
    "A quick way to find out the ancestry of an object.  Prints out the names and object numbers of all ancestors.",
    "",
    "Example:",
    "  @parents Haakon",
    "  Haakon(#2)   generic wizard(#218)   generic programmer(#217)   generic ",
    "  player(#6)   Root Class(#1)"
  };
  property "@quota" (owner: HACKER, flags: "r") = {"*pass*", "@quota"};
  property "@recreate" (owner: HACKER, flags: "rc") = {
    "Usage: @recreate <object> as <parent> named <name spec>",
    "",
    "This is a combination of @create and @chparent.  It takes an existing object, completely strips it of any verbs, properties, and values for inherited properties.  This object is then reshaped into a child of the parent specified, as though @create had been called, but retaining the same object number as the original.",
    "",
    "You may use \"called\" instead of \"named\" in this command, if you wish.",
    "",
    "The <parent> and <name spec> arguments are as in @create."
  };
  property "@recycle" (owner: HACKER, flags: "rc") = {
    "Syntax:  @recycle <object-name-or-number>",
    "",
    "Destroys the indicated object utterly and irretrievably.  Naturally, you may only do this to objects that you own."
  };
  property "@remove-entrance" (owner: HACKER, flags: "rc") = {
    "Syntax:  @remove-entrance <entrance>",
    "",
    "Remove the specified entrance from the current entrances list of the room.  Entrance may be either the name or object number of an entrance to this room."
  };
  property "@remove-exit" (owner: HACKER, flags: "rc") = {
    "Syntax:  @remove-exit <exit>",
    "",
    "Remove the specified exit from the current exits list of the room.  Exit may be either the name or object number of an exit from this room."
  };
  property "@resident" (owner: HACKER, flags: "rc") = {
    "Syntax: @resident player",
    "        @resident !player",
    "        @resident",
    "",
    "Adds or removes a player from the residents list of a room.  The residents list controls who is allowed to use @sethome in that room.  This defaults to just the owner of the room; by manipulating the residents list you may allow additional players to use that room as their home.",
    "",
    "@resident player adds that player to the list.  ",
    "@resident !player removes that player from the list.",
    "@resident with no arguments simply displays the current list (which may be \"none\", indicating no additional people besides the owner may use that room as their home).",
    "",
    "See also help @sethome.",
    "",
    "Hints for programmers: The verb $room:accept_for_abode is called by @sethome.  By overriding this verb you can give different criteria to @sethome.  It should return 1 for allowed and 0 for denied."
  };
  property "@set" (owner: HACKER, flags: "rc") = {"*forward*", "@setprop", "@set is a valid abbreviation for @setprop."};
  property "@setprop" (owner: HACKER, flags: "rc") = {
    "Syntax:  @set <object>.<prop-name> to <value>",
    "",
    "Changes the value of the specified object's property to the given value.",
    "You must have permission to modify the property, either because you own the property or if it is writable."
  };
  property "@sort-owned" (owner: HACKER, flags: "rc") = {
    "Syntax:  @sort-owned  [ object | size ]",
    "",
    "Sorts your .owned_objects property so @audit shows up sorted.  See help @audit for more information.",
    "",
    "@sort-owned object will sort by object number (the default).  @sort-owned size will sort by size of object as periodically recorded."
  };
  property "@unlock" (owner: HACKER, flags: "r") = {
    "Syntax:  @unlock <object>",
    "",
    "Clear any lock that might exist on the given object.  See 'help locking' for general information about locking."
  };
  property "@unlock_for_open" (owner: HACKER, flags: "rc") = {
    "Syntax:",
    "  @unlock_for_open <container>",
    "",
    "Clears the lock which restricts who may open <container>.  See 'help locking' for general information about locking. ",
    "",
    "See 'help containers' for information on containers."
  };
  property "@verify-owned" (owner: HACKER, flags: "rc") = {
    "Syntax:  @verify-owned",
    "",
    "Checks that all the objects in your .owned_objects property are actually owned by you, and effects repairs if needed.  See help @audit for more information."
  };
  property audit_bytes (owner: #2, flags: "r") = {
    "Usage:  @build-option [+|-|!]audit_bytes",
    "Lets you see the actual bytes of small objects in @audit and @prospectus.  Ignored if `audit_float' is turned on.",
    "",
    "  -audit_bytes     @audit/@prospectus shows `<1K'",
    "  +audit_bytes     @audit/@prospectus shows bytes.",
    "",
    "Default: -audit_bytes"
  };
  property audit_float (owner: #2, flags: "r") = {
    "Usage: @build-option [+|-|!]audit_float",
    "Lets you see object sizes in @audit and @prospectus as floating point numbers to one decimal place.",
    "",
    "  -audit_float     @audit/@prospectus shows integer sizes (1K)",
    "  +audit_float     @audit/@prospectus shows floating-point sizes (1.2K)",
    "",
    "Default: -audit_float"
  };
  property "builder-index" (owner: HACKER, flags: "rc") = {"*index*", "Builder Help Topics"};
  property building (owner: HACKER, flags: "r") = {
    "There are a number of commands available to players for building new parts of the MOO.  Help on them is available under the following topics:",
    "",
    "creation -- making, unmaking, and listing your rooms, exits, and other objects",
    "topology -- making and listing the connections between rooms and exits",
    "descriptions -- setting the names and descriptive texts for new objects",
    "locking -- controlling use of and access to your objects"
  };
  property common_quota (owner: #2, flags: "r") = {
    "Syntax:  @quota",
    "",
    "Each player has a limit as to how many objects that player may create, called their 'quota'.  Every object they create lowers the quota by one and every object they recycle increases it by one.  If the quota goes to zero, then that player may not create any more objects (unless, of course, they recycle some first).",
    "",
    "The @quota command prints out your current quota.",
    "",
    "The quota mechanism is intended to solve a long-standing problem in many MUDs: database bloat.  The problem is that a large number of people build a large number of dull objects and areas that are subsequently never used or visited.  The database becomes quite large and difficult to manage without getting substantially more interesting.  With the quota system, we can make it possible for players to experiment and learn while simultaneously keeping random building to acceptable levels."
  };
  property "container-messages" (owner: HACKER, flags: "rc") = {
    "*subst*",
    "Several kinds of messages can be set on a container object; they are printed to various audiences at certain times whenever an attempt is made to use the container.  The ones whose names begin with 'o' are always shown prefixed with the name of the player making the attempt and a single space character.  The standard pronoun substitutions (with respect to the player) are made on each message before it is printed; see 'help pronouns' for details.",
    "",
    "The default message is given in brackets after each name below:",
    "",
    "@empty[%[$container.empty_msg]]",
    "  Printed in place of the contents list when the container is empty.",
    "",
    "@open  [%[$container.open_msg]]",
    "  Printed to the player who successfully opens the container.",
    "",
    "@oopen  [%[$container.oopen_msg]]",
    "  Printed to others in the same room if the player successfully opens the container.",
    "",
    "@open_fail  [%[$container.open_fail_msg]]",
    "  Printed to the player who cannot open the container.",
    "",
    "@oopen_fail  [%[$container.oopen_fail_msg]]",
    "  Printed to others in the room when a player fails to open a container.",
    "",
    "@close  [%[$container.close_msg]]",
    "  Printed to the player who closes a container.",
    "",
    "@oclose  [%[$container.oclose_msg]]",
    "  Printed to others in the room when a player closes a container.",
    "",
    "@put  [%[$container.put_msg]]",
    "  Printed to a player when an object is successfully placed in a container.",
    "",
    "@oput  [%[$container.oput_msg]]",
    "  Printed to others in the room when a player successfully places an object in a container.",
    "",
    "@put_fail  [%[$container.put_fail_msg]]",
    "  Printed when a player fails to put an object in a container.",
    "",
    "@oput_fail  [%[$container.oput_fail_msg]]",
    "  Printed to others in the room when a player fails to place an object in a container.",
    "",
    "@remove  [%[$container.remove_msg]]",
    "  Printed when a player succeeds in removing an object from a container.",
    "",
    "@oremove  [%[$container.oremove_msg]]",
    "  Printed to others in the room when a player succeeds in removing an object from a container.",
    "",
    "@remove_fail  [%[$container.remove_fail_msg]]",
    "  Printed when a player fails to remove an object from a container.",
    "",
    "@oremove_fail  [%[$container.oremove_fail_msg]]",
    "  Printed to others in the room when a player fails to remove an object from a container."
  };
  property containers (owner: HACKER, flags: "r") = {
    "Containers are objects that allow you to store other objects inside them.  The following help topics cover verbs that can be used with containers:",
    "",
    "put -- putting an object into a container",
    "remove -- taking an object out of a container",
    "",
    "Containers may be open or closed, using the verbs 'open container' and 'close container'.  Containers have a separate lock to determine if a player may open them.  See the following help topics:",
    "",
    "@lock_for_open -- setting the lock for opening a container",
    "@unlock_for_open -- clearing the lock",
    "",
    "You can make a container by creating a child of the standard container, $container (see 'help @create').",
    "",
    "Containers have a large number of messages which get printed when players act upon them.  See 'help container-messages' for more information.",
    "",
    "Containers have opacity.  See 'help @opacity' for more information."
  };
  property creation (owner: HACKER, flags: "r") = {
    "The primary means for players to extend the MOO is for them to create new objects with interesting behavior.  There are convenient commands for creating and recycling objects and for keeping track of the objects you've created.  Help is available on these commands in the following topics:",
    "",
    "@dig -- conveniently building new rooms and exits",
    "@create -- making other kinds of objects",
    "@recycle -- destroying objects you no longer want",
    "@quota -- determining how many more objects you can build",
    "@count -- determining how many objects you already own",
    "@audit -- listing all of your objects",
    "@classes -- listing all of the public classes available for your use",
    "@move -- moving your objects from place to place",
    "@parents, @kids -- examine the inheritance hierarchy."
  };
  property "exit-messages" (owner: HACKER, flags: "r") = {
    "*subst*",
    "Several kinds of messages can be set on an exit object (see 'help messages' for instructions on doing so); they are printed to various audiences at certain times whenever an attempt is made to go through the exit.  The ones whose names begin with 'o' are always shown prefixed with the name of the player making the attempt and a single space character.  The standard pronoun substitutions (with respect to the player) are made on each message before it is printed; see 'help pronouns' for details.",
    "",
    "The default message is given in brackets after each name below:",
    "",
    "@leave  [%[$exit.leave_msg]]",
    "  Printed to the player just before they successfully use the exit.",
    "",
    "@oleave  [%[$exit.oleave_msg||\"has left.\"]]",
    "  Printed to others in the source room when a player successfully uses the exit.",
    "",
    "@arrive  [%[$exit.arrive_msg]]",
    "  Printed to the player just after they successfully use the exit.",
    "",
    "@oarrive  [%[$exit.oarrive_msg||\"has arrived.\"]]",
    "  Printed to others in the destination room when a player successfully uses the exit.",
    "",
    "@nogo  [%[$exit.nogo_msg||\"You can't go that way.\"]]",
    "  Printed to the player when they fail in using the exit.",
    "",
    "@onogo  [%[$exit.onogo_msg]]",
    "  Printed to others when a player fails in using the exit."
  };
  property "key-representation" (owner: HACKER, flags: "r") = {
    "The representation of key expressions is very simple and makes it easy to construct new keys on the fly.",
    "",
    "Objects are represented by their object numbers and all other kinds of key expressions are represented by lists.  These lists have as their first element a string drawn from the following set:",
    "        \"&&\"     \"||\"     \"!\"     \"?\"",
    "For the first two of these, the list should be three elements long; the second and third elements are the representations of the key expressions on the left- and right-hand sides of the appropriate operator.  In the third case, \"!\", the list should be two elements long; the second element is again a representation of the operand.  Finally, in the \"?\" case, the list is also two elements long but the second element must be an object number.",
    "",
    "As an example, the key expression",
    "        #45  &&  ?#46  &&  (#47  ||  !#48)",
    "would be represented as follows:",
    "        {\"&&\", {\"&&\", #45, {\"?\", #46}}, {\"||\", #47, {\"!\", #48}}}"
  };
  property keys (owner: HACKER, flags: "r") = {
    "LambdaMOO supports a simple but powerful notation for specifying locks on objects, encryption on notes, and other applications.  The idea is to describe a constraint that must be satisfied concerning what some object must be or contain in order to use some other object.",
    "",
    "The constraint is given in the form of a logical expression, made up of object numbers connected with the operators 'and', 'or', and 'not' (written '&&', '||', and '!', for compatibility with the MOO programming language).  When writing such expressions, though, one usually does not use object numbers directly, but rather gives their names, as with most MOO commands.",
    "",
    "These logical expressions (called 'key expressions') are always evaluated in the context of some particular 'candidate' object, to see if that object meets the constraint.  To do so, we consider the candidate object, along with every object it contains (and the ones those objects contain, and so on), to be 'true' and all other objects to be 'false'.",
    "",
    "As an example, suppose the player Munchkin wanted to lock the exit leading to his home so that only he and the holder of his magic wand could use it.  Further, suppose that Munchkin was object #999 and the wand was #1001.  Munchkin would use the '@lock' command to lock the exit with the following key expression:",
    "        me || magic wand",
    "and the system would understand this to mean",
    "        #999 || #1001",
    "That is, players could only use the exit if they were (or were carrying) either #999 or #1001.",
    "",
    "To encrypt a note so that it could only be read by Munchkin or someone carrying his book, his bell, and his candle, Munchkin would use the 'encrypt' command with the key expression",
    "        me || (bell && book && candle)",
    "",
    "Finally, to keep players from taking a large gold coffin through a particularly narrow exit, Munchkin would use this key expression:",
    "        ! coffin",
    "That is, the expression would be false for any object that was or was carrying the coffin.",
    "",
    "There is one other kind of clause that can appear in a key expression:",
    "        ? <object>",
    "This is evaluated by testing whether the given object is unlocked for the candidate object; if so, this clause is true, and otherwise, it is false.  This allows you to have several locks all sharing some single other one; when the other one is changed, all of the locks change their behavior simultaneously.",
    "",
    "[Note to programmers: The internal representation of key expressions, as stored in .key on every object, for example, is very simple and easy to construct on the fly.  For details, see 'help key-representation'.]"
  };
  property locking (owner: HACKER, flags: "r") = {
    "It is frequently useful to restrict the use of some object.  For example, one might want to keep people from using a particular exit unless they're carrying a bell, a book, and a candle.  Alternatively, one might allow anyone to use the exit unless they're carrying that huge golden coffin in the corner.  LambdaMOO supports a general locking mechanism designed to make such restrictions easy to implement, usually without any programming.",
    "",
    "Every object supports a notion of being 'locked' with respect to certain other objects.  For example, the exit above might be locked for any object that was carrying the coffin object but unlocked for all other objects.  In general, if some object 'A' is locked for another object, 'B', then 'B' is usually prevented from using 'A'.  Of course, the meaning of 'use' in this context depends upon the kind of object.",
    "",
    "The various standard classes of objects use locking as follows:",
    "  + Rooms and containers refuse to allow any object inside them if they're locked for it.",
    "  + Exits refuse to transport any object that they're locked for.",
    "  + Things (including notes and letters) cannot be moved to locations that they're locked for.",
    "",
    "There are two sides to locking:",
    "  + How is it specified whether one object is locked for another one?",
    "  + What is the effect of an object being locked?",
    "Note that these two questions are entirely independent: one could invent a brand-new way to specify locking, but the effect of an exit being locked would be unchanged.",
    "",
    "[Note to programmers: the interface between these two sides is the verb x:is_unlocked_for(y), which is called by x to determine if it is locked for the object y.  The way in which 'is_unlocked_for' is implemented is entirely independent of the ways in which x uses its results.  Note that you can play on either side of this interface with your own objects, either defining new implementations of 'is_unlocked_for' that match your particular circumstances or having your objects interpret their being locked in new ways.]",
    "",
    "There is a default way to specify locks on objects; the following help topics cover the relevant commands:",
    "",
    "@lock -- setting a lock on an object",
    "@unlock -- clearing the lock on an object",
    "keys -- describes the language used to describe lock keys"
  };
  property "object-quota" (owner: HACKER, flags: "rc") = {
    "*forward*",
    "common_quota",
    "",
    "To get a larger quota, talk to a wizard.  They will take a look at what you've done with the objects you've built so far and make a determination about whether or not it would be a net gain for the MOO community if you were to build some more things.  If so, they will increase your quota; if not, they will try to explain some ways in which you could build things that were more useful, entertaining, or otherwise interesting to other players.  Wizards may be more impressed by objects which are interactive and employ a fair number of verbs."
  };
  property "room-messages" (owner: HACKER, flags: "rc") = {
    "*subst*",
    "A few different messages can be set on a room object (see 'help messages' for instructions on doing so); they are printed to various audiences when a player or other object is ejected from the room.  (See 'help @eject'.)  The standard pronoun substitutions are made on each message before it is printed; see 'help pronouns' for details.",
    "",
    "The default message is given in brackets after each name below:",
    "",
    "@ejection  [%[$room.ejection_msg]]",
    "  Printed to the player doing the ejecting.",
    "",
    "@victim_ejection  [%[$room.victim_ejection_msg]]",
    "  Printed to the object being ejected.",
    "",
    "@oejection  [%[$room.oejection_msg]]",
    "  Printed to others in the room from which the object is being ejected."
  };
  property rooms (owner: HACKER, flags: "r") = {
    "Rooms may be made by builders, using the DIG verb. By default, all rooms are instances of _the_ room, $room, or #3, which you can examine to see how it works. If you require a room to have a more specific behaviour, you can make a subclass of room."
  };
  property "thing-messages" (owner: HACKER, flags: "r") = {
    "*subst*",
    "Several kinds of messages can be set on 'things', objects that have $thing as an ancestor (see 'help messages' for instructions on doing so).  They are printed to various audiences under various circumstances when an attempt is made to 'take' or 'drop' a thing.  The ones whose names begin with 'o' are always shown prefixed with the name of the player making the attempt and a single space character.  The standard pronoun substitutions (with respect to the player) are made on each message before it is printed; see 'help pronouns' for details.",
    "",
    "The default message is given in brackets after each name below:",
    "",
    "@take_failed  [%[$thing.take_failed_msg]]",
    "  Printed to a player who fails to take the object.",
    "",
    "@otake_failed [%[$thing.otake_failed_msg]]",
    "  Printed to others in the same room if a player fails to take the object.",
    "",
    "@take_succeeded  [%[$thing.take_succeeded_msg]]",
    "  Printed to a player who succeeds in taking the object.",
    "",
    "@otake_succeeded  [%[$thing.otake_succeeded_msg]]",
    "  Printed to others in the same room if a player succeeds in taking the object.",
    "",
    "@drop_failed  [%[$thing.drop_failed_msg]]",
    "  Printed to a player who fails to drop the object.",
    "",
    "@odrop_failed [%[$thing.odrop_failed_msg]]",
    "  Printed to others in the same room if a player fails to drop the object.",
    "",
    "@drop_succeeded  [%[$thing.drop_succeeded_msg]]",
    "  Printed to a player who succeeds in dropping the object.",
    "",
    "@odrop_succeeded  [%[$thing.odrop_succeeded_msg]]",
    "  Printed to others in the room if a player succeeds in dropping the object."
  };
  property topology (owner: HACKER, flags: "r") = {
    "The topology of the MOO universe is determined by the rooms that exist and the exits that connect them.  Several commands are available for creating and discovering the topology of the MOO.  Help on them is available under the following topics:",
    "",
    "@dig -- creating new rooms and exits",
    "@add-exit -- adding other players' exits from your rooms",
    "@add-entrance -- adding other player's entrances to your rooms",
    "@remove-exit -- removing exits from your room",
    "@remove-entrance -- removing entrances from your room",
    "@exits -- listing all of the conventional exits from your rooms",
    "@entrances -- listing all of the conventional entrances to your rooms",
    "@resident -- listing or changing the residents of your rooms"
  };

  override aliases = {"Builder Help DB", "BHD"};
  override description = "This help database contains topics about the generic builder and building commands.";
  override index_cache = {"builder-index"};
  override object_size = {39390, 1084848672};

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (!caller_perms().wizard)
      raise(E_PERM);
    endif
    pass(@args);
    this.("@quota") = {"*forward*", "object-quota"};
  endverb
endobject