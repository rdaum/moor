## Status of builtin function implementation

The following is a table of the status of various builtin-functions. The table is broken down by category, and each
function is marked with a checkmark if it is implemented. If there are any notes about the implementation, they will be
included in the notes column.

## LambdaMOO 1.8 builtin function list and status

### Lists

| Name         | Complete | Notes |
| ------------ | -------- | ----- |
| `length`     | &check;  |       |
| `setadd`     | &check;  |       |
| `setremove`  | &check;  |       |
| `listappend` | &check;  |       |
| `listinsert` | &check;  |       |
| `listdelete` | &check;  |       |
| `listset`    | &check;  |       |
| `equal`      | &check;  |       |
| `is_member`  | &check;  |       |
| `match`      | &check;  |       |
| `rmatch`     | &check;  |       |
| `substitute` | &check;  |       |

### Strings

| Name        | Complete | Notes                                                                          |
| ----------- | -------- | ------------------------------------------------------------------------------ |
| `tostr`     | &check;  |                                                                                |
| `toliteral` | &check;  |                                                                                |
| `crypt`     | &check;  | Pretty damned insecure, only here to support existing core password functions. |
| `index`     | &check;  |                                                                                |
| `rindex`    | &check;  |                                                                                |
| `strcmp`    | &check;  |                                                                                |
| `strsub`    | &check;  |                                                                                |

### Numbers

| Name       | Complete | Notes |
| ---------- | -------- | ----- |
| `toint`    | &check;  |       |
| `tonum`    | &check;  |       |
| `tofloat`  | &check;  |       |
| `min`      | &check;  |       |
| `max`      | &check;  |       |
| `abs`      | &check;  |       |
| `random`   | &check;  |       |
| `time`     | &check;  |       |
| `ctime`    | &check;  |       |
| `floatstr` | &check;  |       |
| `sqrt`     | &check;  |       |
| `sin`      | &check;  |       |
| `cos`      | &check;  |       |
| `tan`      | &check;  |       |
| `asin`     | &check;  |       |
| `acos`     | &check;  |       |
| `atan`     | &check;  |       |
| `sinh`     | &check;  |       |
| `cosh`     | &check;  |       |
| `tanh`     | &check;  |       |
| `exp`      | &check;  |       |
| `log`      | &check;  |       |
| `log10`    | &check;  |       |
| `ceil`     | &check;  |       |
| `floor`    | &check;  |       |
| `trunc`    | &check;  |       |

### Objects

| Name              | Complete | Notes                              |
| ----------------- | -------- | ---------------------------------- |
| `toobj`           | &check;  |                                    |
| `typeof`          | &check;  |                                    |
| `create`          | &check;  | Quota support not implemented yet. |
| `recycle`         | &check;  |                                    |
| `valid`           | &check;  |                                    |
| `parent`          | &check;  |                                    |
| `children`        | &check;  |                                    |
| `chparent`        | &check;  |                                    |
| `max_object`      | &check;  |                                    |
| `players`         | &check;  | Potentially slow in a large DB.    |
| `is_player`       | &check;  |                                    |
| `set_player_flag` | &check;  |                                    |
| `move`            | &check;  |                                    |

### Properties

| Name                | Complete | Notes |
| ------------------- | -------- | ----- |
| `properties`        | &check;  |       |
| `property_info`     | &check;  |       |
| `set_property_info` | &check;  |       |
| `add_property`      | &check;  |       |
| `delete_property`   | &check;  |       |
| `clear_property`    | &check;  |       |
| `is_clear_property` | &check;  |       |

### Verbs

| Name            | Complete | Notes                                 |
| --------------- | -------- | ------------------------------------- |
| `verbs`         | &check;  |                                       |
| `verb_info`     | &check;  |                                       |
| `set_verb_info` | &check;  |                                       |
| `verb_args`     | &check;  |                                       |
| `set_verb_args` | &check;  |                                       |
| `add_verb`      | &check;  |                                       |
| `delete_verb`   | &check;  |                                       |
| `set_verb_code` | &check;  |                                       |
| `eval`          | &check;  |                                       |
| `disassemble`   | &check;  | Output looks nothing like LambdaMOO's |
| `verb_code`     | &check;  |                                       |

### Values / encoding

| Name            | Complete | Notes                                                                              |
| --------------- | -------- | ---------------------------------------------------------------------------------- |
| `value_bytes`   | &check;  |                                                                                    |
| `value_hash`    |          |                                                                                    |
| `string_hash`   | &check;  |                                                                                    |
| `binary_hash`   |          |                                                                                    |
| `decode_binary` |          | Binary encoding will likely work differently in moor. See README.md for more info. |
| `encode_binary` |          |                                                                                    |
| `object_bytes`  | &check;  |                                                                                    |

### Server

| Name                  | Complete | Notes                                                                    |
| --------------------- | -------- | ------------------------------------------------------------------------ |
| `server_version`      | &check;  | Hardcoded value, should derive from bin crate                            |
| `renumber`            |          |                                                                          |
| `reset_max_object`    |          |                                                                          |
| `memory_usage`        | &check;  |                                                                          |
| `shutdown`            | &check;  |                                                                          |
| `dump_database`       | &check;  |                                                                          |
| `db_disk_size`        | &check;  |                                                                          |
| `connected_players`   | &check;  |                                                                          |
| `connected_seconds`   | &check;  |                                                                          |
| `idle_seconds`        | &check;  |                                                                          |
| `connection_name`     | &check;  | To make this 100% compat with core, reverse DNS & listen port is needed. |
| `notify`              | &check;  | With `rich_notify` feature on, supports sending additional content types |
| `boot_player`         | &check;  |                                                                          |
| `server_log`          | &check;  |                                                                          |
| `load_server_options` |          |                                                                          |
| `function_info`       | &check;  |                                                                          |
| `read`                |          |                                                                          |

### Tasks

| Name           | Complete | Notes |
| -------------- | -------- | ----- |
| `task_id`      | &check;  |       |
| `queued_tasks` | &check;  |       |
| `kill_task`    | &check;  |       |
| `resume`       | &check;  |       |
| `queue_info`   | &check;  |       |
| `force_input`  |          |       |
| `flush_input`  |          |       |

### Execution

| Name             | Complete | Notes        |
| ---------------- | -------- | ------------ |
| `call_function`  | &check;  |              |
| `raise`          | &check;  |              |
| `suspend`        | &check;  |              |
| `seconds_left`   | &check;  |              |
| `ticks_left`     | &check;  |              |
| `pass`           | &check;  | Is an opcode |
| `set_task_perms` | &check;  |              |
| `caller_perms`   | &check;  |              |
| `callers`        | &check;  |              |
| `task_stack`     |          |              |

### Network connections

| Name                      | Complete | Notes                                                                                                |
| ------------------------- | -------- | ---------------------------------------------------------------------------------------------------- |
| `set_connection_option`   |          |                                                                                                      |
| `connection_option`       |          |                                                                                                      |
| `connection_options`      |          |                                                                                                      |
| `open_network_connection` |          |                                                                                                      |
| `listen`                  | &check;  | `print-messages` not yet implemented. errors in binding not properly propagating back to the builtin |
| `unlisten`                | &check;  |                                                                                                      |
| `listeners`               | &check;  |                                                                                                      |
| `output_delimiters`       |          |                                                                                                      |
| `buffered_output_length`  |          |                                                                                                      |

## Extensions

Functions not part of the original LambdaMOO, but added in moor

### XML / HTML content management

| Name        | Description                                                      | Notes                                                 |
| ----------- | ---------------------------------------------------------------- | ----------------------------------------------------- |
| `xml_parse` | Parse a string c ntaining XML into a tree of flyweight objects   | Available only if the flyweights feature is turned on |
| `to_xml`    | Convert a tree of flyweight objects into a string containing XML | Available only if the flyweights feature is turned on |
