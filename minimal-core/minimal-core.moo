object #0
    name: "System Object"
    parent: #1
    owner: #3
    readable: true

    verb do_login_command (this none this) owner: #3 flags: "rxd"
        return #3;
    endverb
endobject

object #1
    name: "Root Prototype"
    owner: #3
    readable: true
endobject

object #2
    name: "The First Room"
    parent: #1
    owner: #3

    verb eval (any any any) owner: #3 flags: "d"
        set_task_perms(player);
        const answer = eval("return " + argstr + ";");
        if (answer[1])
          notify(player, tostr("=> ", toliteral(answer[2])));
        else
          for line in (answer[2])
            notify(player, line);
          endfor
        endif
    endverb
endobject

object #3
    name: "Wizard"
    parent: #1
    location: #2
    owner: #3
    player: true
    wizard: true
    programmer: true
endobject
