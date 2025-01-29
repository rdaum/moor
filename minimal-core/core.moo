// The "lobby" or "system object" is the first object in the system, and an entity from which other objects can be
// hung on properties to form $style references.
// It also houses basic system level verbs like:
//    * do_login_command for processing the first line of input from each new connection
//    * user_connected / user_reconnected / user_disconnected
//    * do_command to override the builtin system parser
//    * do_out_of_band_command
object #0
    name: "Lobby"
    owner: #0
    parent: #1
    location: #-1
    wizard: false
    programmer: false
    player: false
    fertile: false
    readable: true

    override description = "This is the lobby / system object, upon which many things hang.";

    // Definitions for basic prototypes
    property root (owner: #0, flags: r) = #1;
    property thing (owner: #0, flags: r) = #2;
    property room (owner: #0, flags: r) = #4;

    // This will force a login to the default "wizard" character without doing any password authentication of any
    // kind, and is just here to provide minimal functionality.
    verb do_login_command (this none this) owner: #0 flags: rxd
        return #2;
    endverb

    // Called when a user connection has been authenticated (do_login_command above)
    verb user_connected, user_reconnected (this none this) owner: #2 flags: rxd
        let who = args[1];
        set_task_perms(who);
        notify(who, "Welcome to the machine");
    endverb

endobject

// The root object / prototype from which almost everything in the system eventually descends, defining the base
// properties and behaviours.
object #1
    name: "Root Prototype"
    owner: #0
    parent: #-1
    location: #-1
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true

    // Example root property, everything gets a description.
    property description (owner: #1, flags: rc) = "This is a root object";
 endobject

// The only "player" in our minimal database is the wizard.
 object #2
    name: "Wizard"
    owner: #2           // Owns itself.
    parent: #1          // Descends from root
    location: #3       // Hangs out in the generic (only) room.
    programmer: true
    player: true
    wizard: true
    fertile: false
    readable: true

    override description = "You see a wizard";

    // Basic simple eval verb.
    verb eval (any any any) owner: #2 flags: rxd
        notify(player, toliteral(eval("return " + argstr + ";")[2]));
    endverb
 endobject

// The place where the wizard will be.
object #3
     name: "Generic Room"
     owner: #2
     parent: #1
     location: #-1
     wizard: false
     programmer: false
     player: false
     fertile: true
     readable: true

     override description = "This is a generic room";
endobject
