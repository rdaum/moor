`Test.db` is a minimal database with some support objects & conveniences for testing the kernel.

Notable objects:

- `#2`: The First Room
- `#3`: wizard
- `#4`: programmer player
- `#5`: non-programmer player

Globals:

- `$nothing`: `#-1`
- `$system`: `#0`
- `$object`, `$tmp`, `$tmp1`, `$tmp2`: for use as variables that persist across commands

Verbs:

- `#2:do_login_command`: `connect $EXPR` logs in with the player as the object `eval($EXPR)`
- `#2:eval`:
  - sets correct task permissions
  - when an exception is thrown, returns the error value instead of a nice stack trace
