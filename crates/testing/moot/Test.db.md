`Test.db` is a minimal database with some support objects & conveniences for testing the kernel.

Notable objects:

- `#2`: The First Room
- `#3`: wizard
- `#4`: programmer player
- `#5`: non-programmer player

Corified globals:

- `$nothing`: `#-1`
- `$ambiguous_match`: `#-2`
- `$failed_match`: `#-3`
- `$system`: `#0`
- `$wizard_player`: `#3`
- `$programmer_player`: `#4`
- `$player`: `#5`
- `$object`, `$tmp`, `$tmp1`, `$tmp2`: for use as variables that persist across commands
- `$invalid_object`: `toobj(2142147483647)`

Verbs:

- `#2:do_login_command`: `connect $EXPR` logs in with the player as the object `eval($EXPR)`
- `#2:eval`:
  - sets correct task permissions
  - when an exception is thrown, returns the error value instead of a nice stack trace
