object #2
  name: "Wizard"
  parent: WIZ
  location: PLAYER_START
  owner: #2
  player: true
  wizard: true
  programmer: true

  override aliases = {"Wizard"};
  override current_folder = #2;
  override current_message = {
    0,
    0,
    {NEW_PLAYER_LOG, 0, 0},
    {NEW_PROG_LOG, 0, 0},
    {QUOTA_LOG, 0, 0},
    {NEWT_LOG, 0, 0}
  };
  override features = {PASTING_FEATURE, STAGE_TALK};
  override first_connect_time = 1529444339;
  override last_connect_place = "";
  override last_connect_time = 1529543480;
  override last_disconnect_time = 1529543472;
  override object_size = {5052, 1084848672};
  override owned_objects = {
    SYSOBJ,
    ROOT_CLASS,
    #2,
    ROOM,
    BUILDER,
    THING,
    PLAYER,
    EXIT,
    CONTAINER,
    NOTE,
    LOGIN,
    LAST_HUH,
    GUEST_LOG,
    LIMBO,
    NEW_PLAYER_LOG,
    STRING_UTILS,
    BUILDING_UTILS,
    WIZ_UTILS,
    NEW_PROG_LOG,
    QUOTA_LOG,
    MAIL_RECIPIENT_CLASS,
    PERM_UTILS,
    OBJECT_UTILS,
    LOCK_UTILS,
    LETTER,
    COMMAND_UTILS,
    WIZ,
    PROG,
    NEWT_LOG,
    NETWORK,
    GOPHER,
    GENERIC_UTILS,
    SERVER_OPTIONS,
    FTP,
    PASSWORD_VERIFIER,
    GENDERED_OBJECT,
    HTTP
  };
  override ownership_quota = -10000;
  override password = 0;
  override previous_connection = {1529444339, "localhost"};
  override size_quota = {50000, 769725, 1084848672, 0};
endobject