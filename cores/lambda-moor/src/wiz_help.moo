object WIZ_HELP
  name: "Wizard Help"
  parent: GENERIC_HELP
  owner: HACKER
  readable: true

  property "$site_db" (owner: HACKER, flags: "rc") = {
    "Database of places",
    "------------------",
    "i.e., places people have connected from.",
    "",
    "  :add(sitename,player)",
    "      records the fact that player connected from sitename.",
    "  :load()",
    "      clears the db and reloads all of the player connection info.",
    "",
    "  .domain",
    "      default domain for unqualified sitenames given to :add.",
    "      ",
    "For each domain we keep a list of players and subdomains. ",
    "For example, :add(\"doc.ic.ac.uk\",#666) enters #666 on the lists for \"doc.ic.ac.uk\", and, if we have to create an entry for \"doc.ic.ac.uk\", we enter \"doc\" on the list for \"ic.ac.uk\", \"ic\" on the list for \"ac.uk\", etc....  In this case, :find(\"ic\") will return the \"ic.ac.uk\" list if there is no other domain in $site_db starting with \"ic\".  Note that the \"ic.ac.uk\" list may contain both objects, i.e., namely players that have connected from the site \"ic.ac.uk\", and strings, i.e., subdomains of \"ic.ac.uk\" like \"doc\".",
    "",
    "  :find_exact(string)    => player/subdomain list or $failed_match",
    "  :find_all_keys(string) => list of all domains that begin with string",
    "  :find_key     (string) => unique domain that begins with string, ",
    "                            $ambiguous_match or $failed_match",
    "",
    "The other $generic_db functions (:find, :find_all) are also available, though admittedly less useful."
  };
  property "@@who" (owner: HACKER, flags: "rc") = {"*forward*", "@net-who"};
  property "@abort-shutdown" (owner: HACKER, flags: "rc") = {
    "Syntax:  @abort-sh*utdown [<text>]",
    "",
    "This aborts any shutdown currently in progress (i.e., set in motion by @shutdown).  All players are notified that no shutdown will actually occur; <text>, if given will be included in this notification."
  };
  property "@blacklist" (owner: HACKER, flags: "rc") = {
    "Syntax:  @redlist   [<domain or subnet> [for <duration>] [commentary]]",
    "         @blacklist [<domain or subnet> [for <duration>] [commentary]]",
    "         @graylist  [<domain or subnet> [for <duration>] [commentary]]",
    "         @spooflist [<domain or subnet> [for <duration>] [commentary]]",
    "",
    "Syntax:  @unredlist   [<domain or subnet> [commentary]]",
    "         @unblacklist [<domain or subnet> [commentary]]",
    "         @ungraylist  [<domain or subnet> [commentary]]",
    "         @unspooflist [<domain or subnet> [commentary]]",
    "",
    "With no argument, the current contents of the corresponding list are printed.",
    "Otherwise, the specified domain or subnet is added to or removed from the list and mail will be sent to $site_log.",
    "",
    "To add a domain or subnet to a *list only temporarily, include a `for <duration>' statement before any commentary.  The <duration> should be in english form such as 1 day or 1 month 2 weeks or 1 year 3 months 2 weeks 4 days.  No commas should separate increments in the duration.  See `help $time_utils:parse_english_time_interval' for more details.  If you are not temporarily *listing a domain or subnet, but are including a commentary, be sure that the commentary does not start with the word `for'.",
    "",
    "If the given domain or subnet has subdomains/subsubnets that are already on the list, you will be prompted as to whether you want to remove them.  Note that adding an entry for a particular domain or subnet effectively adds all subdomains/subsubnets, so unless there's some reason for keeping an explicit entry for a particular subdomain, chances are you will indeed want to remove them.  One reason to keep an explicit entry for a subdomain would be if you intended to unlist the full domain later but wanted to be sure you didn't unlist the subdomain in the process.",
    "",
    "See `help blacklist' for a description of the functions of these lists."
  };
  property "@chown" (owner: HACKER, flags: "rc") = {
    "Syntax:  @chown <object>            [to] <owner>",
    "         @chown <object>.<propname> [to] <owner>",
    "         @chown <object>:<verbname> [to] <owner>",
    "         @chown# <object>:<verbnumber> [to] <owner>",
    "",
    "Changes the ownership of the indicated object, property or verb.",
    "",
    "Verb ownership changes are fairly straightforward, being merely a matter of changing the verb_info() on a single verb. Referring to a verb isn't as straightforward since two verbs on the same object can have the same name. So, @chown# is provided where you can refer to a verb by it's 1-based index in the output of the verbs() builtin.",
    "",
    "Changing an object ownership includes changing the ownership on all +c properties on that object.  Note that @chown will not change the ownership of any other properties, nor will it change verb ownerships.  Use @grant if you need to do a more complete ownership change.  The quota of the former owner is increased by one, as is the quota of the new owner decreased by one.",
    "",
    "Changing a property ownership is truly hairy.  If the property is +c one shouldnot be doing this, unless it is to correct a past injustice which caused the property to be owned by the wrong player.  In the case of -c properties, the property ownership is changed on all descendent objects (currently, if +c instances of a -c property are found in the traversal of all of the descendants, these are not changed, being deemed sufficiently weird that they should be handled on a case-by-case basis...).",
    "",
    "If there's any justice, a future version of the server will prevent occurrences of (1) +c properties being owned by someone other than the object owner (2) -c properties with different owners on descendant objects (3) -c properties that are +c on some descendants."
  };
  property "@chown#" (owner: HACKER, flags: "rc") = {"*forward*", "@chown"};
  property "@denewt" (owner: HACKER, flags: "rc") = {
    "Syntax:    @denewt <player> [commentary]",
    "",
    "Synonyms:  @unnewt",
    "           @get-better",
    "",
    "@denewt reverses the effects of @newt, removing the player from $login.newted, and if appropriate, $login.temporary_newts.",
    "",
    "Mail is sent to $newt_log including any commentary you provide.  E.g.,",
    "",
    "  @denewt Twit  He promises not to do it again."
  };
  property "@deprogrammer" (owner: HACKER, flags: "rc") = {
    "Information about $wiz:@deprog*rammer",
    "----",
    "@deprogrammer victim [for <duration>] [reason]",
    "",
    "Removes the prog-bit from victim.  If a duration is specified (see help $time_utils:parse_english_time_interval), then the victim is put into the temporary list. He will be automatically removed the first time he asks for a progbit after the duration expires.  Either with or without the duration you can specify a reason, or you will be prompted for one. However, if you don't have a duration, don't start the reason with the word `For'."
  };
  property "@detoad" (owner: HACKER, flags: "rc") = {"*forward*", "@untoad"};
  property "@dump-database" (owner: HACKER, flags: "rc") = {
    "Syntax:  @dump-database",
    "",
    "Invokes the builtin dump_database(), which requests that the server checkpoint the database at its next opportunity.  It is not normally necessary to call this function; the server automatically checkpoints the database at regular intervals; see the chapter on server assumptions about the database for details."
  };
  property "@egrep" (owner: HACKER, flags: "rc") = {"*forward*", "@grep"};
  property "@egrepall" (owner: HACKER, flags: "rc") = {"*forward*", "@grep"};
  property "@grant" (owner: HACKER, flags: "rc") = {
    "Information about generic wizard(#218):@grant/@grants*/@transfer",
    "----",
    "@grant <object> to <player>",
    "@grants <object> to <player>   --- same as @grant but may suspend.",
    "@transfer <expression> to <player> -- like 'grant', but evalutes a possible list of objects to transfer.",
    "",
    "Ownership of the object changes as in @chown and :set_owner (i.e., .owner and all c properties change).  In addition all verbs and !c properties owned by the original owner change ownership as well.  Finally, for !c properties, instances on descendant objects change ownership (as in :set_property_owner).",
    "",
    "This verb does the transfer whether the recipient has enough quota for it or not."
  };
  property "@graylist" (owner: HACKER, flags: "rc") = {"*forward*", "@blacklist"};
  property "@grep" (owner: HACKER, flags: "rc") = {
    "*pass*",
    "@grep",
    "",
    "For wizards, the following forms are also available for doing full-db searches",
    "",
    "         @grep  <pattern>",
    "         @grep  <pattern> from [#]<n>",
    "",
    "and likewise for @egrep, @grepall, and @egrepall.",
    "The first searches all objects in the database while the second searches the range [#<n>..max_object()]",
    "",
    "See also:  @grepcore, @who-calls."
  };
  property "@grepall" (owner: HACKER, flags: "rc") = {"*forward*", "@grep"};
  property "@grepcore" (owner: HACKER, flags: "rc") = {
    "Syntax:  @grepcore <pattern>",
    "         @who-calls <verbname>",
    "",
    "@grepcore pattern is @grep pattern in {all core objects}.  Core objects are computed for you by #0:core_objects().",
    "",
    "@who-calls greps for the verbname + \"(\", hoping to catch it as a verb call.  Currently @who-calls does not allow you to restrict the search as @grep does.  (Volunteers?)"
  };
  property "@guests" (owner: HACKER, flags: "rc") = {
    "",
    "@guests now  [shows information about currently connected guests]",
    "@guests all  [shows all entries in $guest_log]",
    "@guests <n>  [shows the last <n> entries of $guest_log]",
    "",
    "Note, some wizards prefer to use verbs on $guest_log manually, particularly :last()."
  };
  property "@log" (owner: HACKER, flags: "rc") = {
    "Syntax:  @log <message>",
    "         @log",
    "",
    "The first form enters <message> as a one-line comment in the server log.",
    "The second form prompts for a sequence of lines to be collectively entered as an extended comment.  This uses $command_utils:read_lines so all of those conventions apply, i.e., a period on a line by itself ends the text, `@abort' aborts the command, etc...).  Example:  If Wizard (#2) types",
    "",
    "    @log I did $dump_interval=3600",
    "",
    "the following line appears in the server log",
    "",
    "    Aug 19 22:36:52:  COMMENT:  from Wizard (#2):  I did $dump_interval=3600"
  };
  property "@make-guest" (owner: HACKER, flags: "rc") = {
    "Syntax:  @make-guest <adjective>",
    "",
    "This creates a new guest character.  For example,",
    "  @make-guest Loud",
    "creates a child of $guest, owned by $hacker, named Loud_Guest and with aliases Loud and Loud_Guest.",
    "",
    "Note that in order to have `connect guest' connect to a guest character, there needs to exist some guest character having \"Guest\" as a name or alias.",
    "",
    "See also `help @make-player'."
  };
  property "@make-player" (owner: HACKER, flags: "rc") = {
    "@make-player name [email-address [commentary]]",
    "Creates a player.",
    "Generates a random password for the player.",
    "Email-address is stored in $registration_db and on the player object.",
    "Comments should be enclosed in quotes.",
    "",
    "Example: @make-player George sanford@frobozz.com \"George shares email with Fred Sanford (Fred #5461)\"",
    "",
    "If the email address is already in use, prompts for confirmation.  If the name is already in use, prompts for confirmation.  (Say no, this is a bug: it will break if you say yes.)  If you say no at one of the confirming prompts, character is not made.",
    "",
    "If network is enabled (via $network.active) then asks if you want to mail the password to the user after character is made."
  };
  property "@net-who" (owner: HACKER, flags: "rc") = {
    "Syntax:  @net-who [<player>...]",
    "         @net-who from [<domain>]",
    "",
    "Synonym: @@who",
    "",
    "@net-who without any arguments prints all connected users and hosts.  If one or more <player> arguments are given, the specified users are printed along with their current or most recent connected hosts.  If any of these hosts are mentioned on $login.blacklist or $login.graylist (see `help blacklist'), ",
    "an annotation appears.",
    "",
    "With a `from...' argument, this command consults $site_db and prints all players who have ever connected from the given domain."
  };
  property "@new-password" (owner: HACKER, flags: "rc") = {
    "@new-password player is [password]",
    "Sets a player's password; omit password string to have one randomly generated.  Prints the encrypted old string when done for error recovery.  [No current software will allow you to give the encrypted string as input.]",
    "",
    "Offers to send mail to the user with the new password, if the user has a registered email address and the network is enabled."
  };
  property "@newt" (owner: HACKER, flags: "rc") = {
    "*subst*",
    "Syntax:  @newt <player> [commentary]",
    "         @temp-newt <player> for <period>",
    "",
    "The @newt command temporarily prevents logins on a given player.",
    "It works by adding the player to $login.newted, and for @temp-newt, also adding the player and an end time to $login.temporary_newts.  $login will deny connection to any player in $login.newted, unless they are temporarily newted and their time has expired, in which case it will clean up---denewt them---and allow the connection attempt.  Use @denewt to reverse this.",
    "",
    "You must give either the player's full name or its object number.",
    "Also, this command does not let you @newt yourself.",
    "",
    "Mail will be sent to $newt_log, listing the player's .all_connect_places and including any commentary you provide.  E.g.,",
    "",
    "  @newt Twit  did real annoying things.",
    "",
    "As with @toad and @programmer, there are messages that one may set",
    "",
    "@newt  [%[$wiz.newt_msg]]",
    "  Printed to everyone in the room in which the victim is being @newted.",
    "  If you're worried about accidentally newting yourself in the process of",
    "  setting this message, you can't (see above).",
    "",
    "@newt_victim  [%[$wiz.newt_victim_msg]]",
    "  Printed to the victim.  ",
    "  This is followed by $login:newt_registration_string().",
    "",
    "See `help @toad' if you need something more drastic.",
    "",
    "The @temp-newt variant of @newt permits you to specify a time period during which this player may not use the MOO.  Time units must be acceptable to $time_utils:parse_english_time_interval."
  };
  property "@players" (owner: HACKER, flags: "rc") = {"Syntax:  @players [with objects]", "", "Hmmm... what *does* this do, anyway?"};
  property "@programmer" (owner: HACKER, flags: "rc") = {
    "*subst*",
    "Syntax:  @programmer <player>",
    "",
    "Sets the programmer flag on the indicated player and sends mail to $new_prog_log.  ",
    "",
    "If the player is not already a descendant of $prog, we @chparent him/her to $prog.  In this case, if $prog has a larger .ownership_quota than its ancestors, then we raise the player's quota by the difference between $prog.ownership_quota and the .ownership_quota of the common ancestor of player and $prog, be this $player or some intermediate class.",
    "",
    "There are messages that one may set to customize how the granting of a programmer bit looks to the victim and to any onlookers.  After all, this is a seminal event in a MOOer's life...  Thus we have",
    "",
    "@programmer  [%[$wiz.programmer_msg]]",
    "  Printed to everyone in the room with the victim being @programmer'ed.",
    "",
    "@programmer_victim  [%[$wiz.programmer_victim_msg]]",
    "  Printed to the victim.",
    "",
    "These are pronoun subbed with victim == dobj."
  };
  property "@quota" (owner: HACKER, flags: "rc") = {
    "*pass*",
    "@quota",
    "",
    " - - - - - - - - - - - - - - - - - - - - - - - - - -",
    "Syntax:  @quota <player> is [public] [+]<number> [<reason>]",
    "",
    "This second and more interesting form of the verb is used to set a player's quota.  Mail will be sent to $quota_log, and also $local.public_quota_log if there is one and if the \"public\" argument is given; if a reason is supplied, it will be included in the message.  If the number is prefixed with a +, it's taken as an amount to add to the player's current quota; if not, it's an absolute amount."
  };
  property "@recycle" (owner: HACKER, flags: "rc") = {
    "*pass*",
    "@recycle",
    "",
    "Of course, wizards are allowed to @recycle anything at all.",
    "",
    "There is, however, a block (in $player:recycle) against recycling actual players, i.e., descendants of $player that have the player flag set.  This is mainly to prevent stupid mistakes.  If, for some reason, you want to recycle a player, you need to @toad it first."
  };
  property "@redlist" (owner: HACKER, flags: "rc") = {"*forward*", "@blacklist"};
  property "@register" (owner: HACKER, flags: "rc") = {
    "Information about $wizard:@register",
    "----",
    "Registers a player.",
    "Syntax:  @register name email-address [additional commentary]",
    "Email-address is stored in $registration_db and on the player object."
  };
  property "@shout" (owner: HACKER, flags: "rc") = {
    "Syntax:  @shout <text>",
    "",
    "Broadcasts the given text to all connected players."
  };
  property "@shutdown" (owner: HACKER, flags: "rc") = {
    "Syntax:  @shutdown [in <m>] [<text>]",
    "",
    "This is the friendly way to do a server shutdown; it arranges for the actual shutdown to take place `m' minutes hence (default two).  Shutdown is preceded by a sequence of warnings to all connected players.  Warnings are likewise given to all players who connect during this time.  <text>, if given is included in these warning messages, perhaps as an explanation for why the server is being shut down.",
    "",
    "Shutdown may be aborted at any time by using @abort-shutdown."
  };
  property "@spooflist" (owner: HACKER, flags: "rc") = {"*forward*", "@blacklist"};
  property "@temp-newt" (owner: HACKER, flags: "rc") = {
    "Information about $wiz:@temp-newt",
    "----",
    "@temp-newt victim [for duration] [reason]",
    "",
    "Temporarily newts victim.  If a duration is specified (see help $time_utils:parse_english_time_interval), then the victim is put into the temporary list. E will be automatically removed the first time e tries to connect after the duration expires.  You will be prompted for a reason for the newting, but as of this writing, specifying a reason from the command line isn't an option."
  };
  property "@toad" (owner: HACKER, flags: "rc") = {
    "*subst*",
    "Syntax:  @toad   <player>  [graylist|blacklist|redlist]",
    "         @toad!  <player>",
    "         @toad!! <player>",
    "",
    "Resets the player flag of <player> (thus causing <player> to be booted), resets the .programmer and .wizard flags, chowns the player object to $hacker, and removes all of its names and aliases from $player_db.",
    "",
    "You must give either the player's full name or its object number.",
    "Also, this command does not let you @toad yourself.",
    "",
    "In some cases you may wish to add the player's last connected site to the site graylist, blacklist or redlist --- see `help blacklist' --- in order to invoke various kinds of blocking on that site (e.g., if player creation is enabled, you may want to enter the player on the blacklist to keep him from immediately creating a new character).  Specifying one of the listnames `graylist', `blacklist' or `redlist' will do this.",
    "",
    "@toad!  <player>  is synonymous with  @toad <player> blacklist",
    "@toad!! <player>  is synonymous with  @toad <player> redlist",
    "",
    "There are messages that one may set to customize toading.  After all, a toading is (supposed to be) a rare event and you will doubtless want to put on a good show.  Thus we have",
    "",
    "@toad  [%[$wiz.toad_msg]]",
    "  Printed to everyone in the room in which the victim is being @toaded.",
    "  If you're worried about accidentally toading yourself in the process of",
    "  setting this message, see above.",
    "",
    "@toad_victim  [%[$wiz.toad_victim_msg]]",
    "  Printed to the victim.",
    "",
    "These are pronoun_subbed with victim == dobj."
  };
  property "@unnewt" (owner: HACKER, flags: "rc") = {"*forward*", "@denewt"};
  property "@untoad" (owner: HACKER, flags: "rc") = {
    "Syntax:  @untoad <object> [as <name>,<alias>,<alias>...]",
    "",
    "Synonym: @detoad",
    "",
    "Turns the object into a player.  ",
    "If the name/alias... specification is given, the object is also renamed.",
    "In order for this to work, the object must be a nonplayer descendant of $player and the new object name (or the original name if none is given in the command line) must be available for use as a player name.  As with ordinary player @renaming, any aliases which are unavailable for use as player names are eliminated.",
    "",
    "If the object is a descendant of $guest, then it becomes a new guest character.",
    "Otherwise the object is chowned to itself.  In the latter case, it is advisable to check that the .password property has something nontrivial in it.",
    "",
    "If the object is a descendant of $prog, then its .programmer flag is set.",
    "Note that the .wizard flag is not set under any circumstances."
  };
  property "@who-calls" (owner: HACKER, flags: "rc") = {"*forward*", "@grepcore"};
  property "adding-help-text" (owner: HACKER, flags: "rc") = {
    "For information about how the help system itself works and about how to associate local help databases with specific rooms or player classes, see `help $help'.",
    "",
    "To get a list of the object numbers associated with various $help databases, type 'help index'.",
    "",
    "If you need to modify existing help text, and need to find which help database the relevant property is defined on, use 'help full-index'.  (Note, it's spammy, but tells you what you need to know.)"
  };
  property advertised (owner: HACKER, flags: "rc") = {
    "Some wizards choose not to be among those listed when a player types '@wizards' (or similar).",
    "",
    "The property $wiz.advertised defaults to 1; set it to 0 to remove yourself from the list.",
    "",
    "To keep your non-wizard character off the list, set your wizard character's .public_identity character to 0.  To get it back on, set .public_identity to the object number of your non-wizard character.",
    "",
    "$wiz_utils:is_wizard returns true for the wizard and the corresponding .public_identity player.  Both will likewise appear in $wiz_utils:connected_wizards_unadvertised() and $wiz_utils:all_wizards_unadvertised().",
    "",
    ":is_wizard is for checking permissions on wizard feature-objects and the like, while :all/connected_wizards_unadvertised wouble be for things like wizard-shouts (e.g., the one issued by $player:recycle)."
  };
  property blacklist (owner: HACKER, flags: "rc") = {
    "",
    "The Site Blacklist",
    "------------------",
    "$login maintains three lists of hosts/domains to support player registration schemes and blocking of connections from highly untrusted hosts:",
    "",
    "  .redlist   -- all connections from these sites are disabled ",
    "  .blacklist -- player creation and guest logins are disabled",
    "  .graylist  -- advisory list of potential trouble spots (putting a site on the",
    "                .graylist merely annotates it in @net-who listings).",
    "  .spooflist -- guests from these sites cannot use @request to request ",
    "                a character",
    "",
    "The lists are kept in a special format so it is highly recommended that you ",
    "either use $wiz:@*list/@un*list or the following verbs to query/update the ",
    "respective lists rather than bash them directly:",
    "",
    "  $login:*listed     (host)              is host is on .*list?",
    "  $login:*list_add   (domain or subnet)  add domain or subnet to .*list",
    "  $login:*list_remove(domain or subnet)  remove domain or subnet from .*list",
    "",
    "where `*' is one of `black', `red', `gray', or `spoof'.",
    "",
    "There are also temporary versions of the above four lists, stored in associated $login.temporary_*list in the same format, except two additional bits of data are stored.  The time the temporary *listing started, and the duration that it will last.  In addition there exists:",
    "",
    "  $login:*list_add_temp(domain or subnet, start time, duration)",
    "  $login:*list_remove_temp(domain or subnet)",
    "",
    "When the normal $login:*listed verb is called, both the regular *list and the temporary *list are checked.  If the host is on the temporary list, then the length of MOO up time since the start time is checked against the duration.  If expired, the host is removed from the temporary *list and a false value is returned (meaning that the host is not *listed).",
    "",
    "One may either specify a domain name (e.g., \"baz.edu\") or a numeric IP address (e.g., \"36.0.23.17\").  Domain names match all hosts underneath that domain, so, e.g., puting \"baz.edu\" on a list effectively adds \"x.bax.edu\" for all x as well.  ",
    "Likewise, an incomplete numeric address, e.g., \"128.42\" will match that entire subnet, in this case all hosts whose IP numbers have the form \"128.42.m.n\" for arbitrary integers m and n.",
    "",
    "One may also give a domain name containing a wildcard (\"*\"), e.g., \"fritz*.baz.edu\", in which case all hostnames matching in the sense of $string_utils:match_string() are considred to be on the list.  Wildcard matching should be avoided since it is more time-consuming.",
    "",
    "It should be noted that, since there is no direct access to the domain name service from within the MOO, it is possible for a host to be blacklisted or redlisted via its domain name, and yet have someone be able to connect from that host (and, in the case of a blacklisted host, create a character) --- this can happen if the name service is down and connection_name() on that player thus has given the numeric IP address rather than the domain name.  Similarly, if you list a host by IP number alone, it will still be possible to get in via the site's domain name.  Thus to be completely assured of shutting out a site, you need to list it both by domain name and IP number."
  };
  property "forked-tasks" (owner: HACKER, flags: "rc") = {
    "If you are a wizard, '@forked' with no arguments will spam you with all the forked tasks that there are (this is useful sometimes, but it's nice to know ahead of time).",
    "",
    "To see just your own, type '@forked me'.  To see just one player's, type '@forked <player>'."
  };
  property "further-reading" (owner: HACKER, flags: "rc") = {
    "Other topics of interest to wizards:",
    "",
    "$login",
    "$guest_log",
    "$no_one",
    "$recycler",
    "$help"
  };
  property graylist (owner: HACKER, flags: "rc") = {"*forward*", "blacklist"};
  property "mail-lists" (owner: HACKER, flags: "rc") = {
    "You probably want to subscribe to (or at least be familiar with) the following mailing lists:",
    "",
    "*Player-Creation-Log",
    "*New-Prog-Log",
    "*Quota-Log",
    "*News",
    "*Site-Locks",
    "*Password-Change-Log",
    ""
  };
  property "news-items" (owner: HACKER, flags: "rc") = {
    "*subst*",
    "To add a news item:",
    "",
    "Send regular mail to *news with the message you want in the news.  Then:",
    "",
    "  @addnews <message-number> to %[tostr($news)]",
    "",
    "To remove a news item:",
    "",
    "  @rmnews <message-number> from %[tostr($news)]",
    "",
    "Note, the message date doesn't show up, so you might consider adding a date to the message body itself."
  };
  property "recycling-players" (owner: HACKER, flags: "rc") = {
    "General procedure:",
    "",
    "  Make sure e doesn't own anything.",
    "  @toad em",
    "  @recycle em",
    "",
    "It makes a real mess if you don't clean up .owned_objects.  See $wiz_utils:initialize_owned, but note, running this verb takes maybe three hours (at last report) and adds to lag.  This is why we frown so severely on leaving blood on the carpet."
  };
  property redlist (owner: HACKER, flags: "rc") = {"*forward*", "blacklist"};
  property routine_tasks (owner: HACKER, flags: "rc") = {
    "There are a number of routine daily or weekly tasks that can help keep your MOO clean or otherwise well-maintained.",
    "",
    "",
    "$byte_quota_utils:schedule_measurement_task",
    "        If you are using byte quota, this will schedule your quota measurement task.  Every night, every item on the moo which has not been measured in the last $byte_quota_utils.cycle_days will be measured.  A report will be mailed to $byte_quota_utils.report_recipients.  You may wish to edit this verb to change the time that it runs---it will run at midnight PST.",
    "",
    "$wiz_utils:expire_mail_weekly",
    "        If you wish to expire old mail from users and mailing lists, run this verb.  Once a week (scheduled from the first time you type ;$wiz_utils:expire_mail_weekly(), not at a particular hour) it will go through and expire mail based on players' @mail-options settings.",
    "",
    "$wiz_utils:flush_editors",
    "        Once a week this will remove all sessions which were begun more than 30 days ago in the note, verb, and mail editors.  Schedule is from when first typed.",
    "",
    "$paranoid_db:semiweeklyish",
    "        This will go through the @paranoid database and remove entries for players who have not connected within the past three days, and for those users who have turned off the @paranoid function.  Schedule is at 11pm PST.",
    "",
    "$login:sample_lag",
    "        This will provide an estimate of the CPU portion of what is normally called \"lag\"---that is, the delay between entering a command and having that command fulfilled.",
    "",
    "$housekeeper:continuous",
    "        If you wish to provide players with the ability to have individual items transported to a known starting location, use this verb.",
    "",
    "",
    "Additionally, there are tasks that you don't have to start manually, but which get started by various actions in the MOO.",
    "",
    "$network:add_queued_mail",
    "        This indicates that there was a temporary failure to deliver email.  If this task is constantly in the queue, it is worth checking $network.queued_mail, deleting those which will never be delivered.  Queued mail does not expire.",
    "",
    "$housekeeper:move_players_home",
    "        This task is used to consolidate the tasks spawned by disconnecting players---they get a 5 minute grace period to log back in before they are moved back home."
  };
  property "site-info" (owner: HACKER, flags: "rc") = {
    "To look at where a player is currently connecting from, use @netwho.  To see previous connect sites, look at <player>.all_connect_places."
  };
  property spooflist (owner: HACKER, flags: "rc") = {"*forward*", "blacklist"};
  property "wiz-index" (owner: HACKER, flags: "rc") = {"*index*", "Wizard Help Topics"};

  override aliases = {"Wizard Help"};
  override description = {"This describes the various commands available on $wiz."};
  override index_cache = {"wiz-index"};
  override object_size = {31466, 1084848672};
endobject