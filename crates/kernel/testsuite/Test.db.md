`Test.db` is a minimal database with some support objects & conveniences for testing the kernel.

Notable objects:

- `#2`: The First Room
- `#3`: wizard
- `#4`: programmer player
- `#5`: non-programmer player

Globals:

- `$nothing`
- `$o`, `$tmp`, `$tmp1`, `$tmp2`: for use as variables that persist across commands

Verbs:

- `#2:eval`: `notify(player, toliteral(eval(("return " + argstr) + ";")[2]));`
