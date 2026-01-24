object SYSOBJ
    name: "System Object"
    parent: ROOT
    owner: ARCH_WIZARD
    readable: true

    property arch_wizard (owner: ARCH_WIZARD, flags: "r") = ARCH_WIZARD;
    property root (owner: ARCH_WIZARD, flags: "r") = ROOT;
    property bench_controller (owner: ARCH_WIZARD, flags: "r") = BENCH_CONTROLLER;
    property bench_subscriber (owner: ARCH_WIZARD, flags: "r") = BENCH_SUBSCRIBER;
    property game_update (owner: ARCH_WIZARD, flags: "r") = GAME_UPDATE;
    property server_options (owner: ARCH_WIZARD, flags: "r") = SERVER_OPTIONS;

    verb do_login_command (this none this) owner: ARCH_WIZARD flags: "rxd"
        return ARCH_WIZARD;
    endverb

endobject
