## Status of builtin function implementation

The following is a table of the status of various builtin-functions. The table is broken down by category, and each
function is marked with a checkmark if it is implemented. If there are any notes about the implementation, they will be
included in the notes column.

## LambdaMOO 1.8 builtin function list and status

### Lists

| Name         | Complete | Notes |
|--------------|----------|-------|
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

| Name        | Complete | Notes                                                                                                |
|-------------|----------|------------------------------------------------------------------------------------------------------|
| `tostr`     | &check;  |                                                                                                      |
| `toliteral` | &check;  |                                                                                                      |
| `crypt`     | &check;  | Pretty damned insecure, only here to support existing core password functions.                       |
| `index`     | &check;  |                                                                                                      |
| `rindex`    | &check;  |                                                                                                      |
| `strcmp`    | &check;  |                                                                                                      |
| `strsub`    | &check;  |                                                                                                      |
| `salt`      | &check;  | Generate a random crypto-secure salt for password. Not compatible with toast's function of same name |

### Numbers

| Name       | Complete | Notes |
|------------|----------|-------|
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
|-------------------|----------|------------------------------------|
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
|---------------------|----------|-------|
| `properties`        | &check;  |       |
| `property_info`     | &check;  |       |
| `set_property_info` | &check;  |       |
| `add_property`      | &check;  |       |
| `delete_property`   | &check;  |       |
| `clear_property`    | &check;  |       |
| `is_clear_property` | &check;  |       |

### Verbs

| Name            | Complete | Notes                                 |
|-----------------|----------|---------------------------------------|
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
|-----------------|----------|------------------------------------------------------------------------------------|
| `value_bytes`   | &check;  |                                                                                    |
| `value_hash`    |          |                                                                                    |
| `string_hash`   | &check;  |                                                                                    |
| `binary_hash`   |          |                                                                                    |
| `decode_binary` |          | Binary encoding will likely work differently in moor. See README.md for more info. |
| `encode_binary` |          |                                                                                    |
| `object_bytes`  | &check;  |                                                                                    |

### Server

| Name                  | Complete | Notes                                                                    |
|-----------------------|----------|--------------------------------------------------------------------------|
| `server_version`      | &check;  | Crate version + short commit hash, for now                               |
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
| `read`                | &check;  |                                                                          |

### Tasks

| Name           | Complete | Notes                                                                                 |
|----------------|----------|---------------------------------------------------------------------------------------|
| `task_id`      | &check;  |                                                                                       |
| `queued_tasks` | &check;  |                                                                                       |
| `kill_task`    | &check;  |                                                                                       |
| `resume`       | &check;  |                                                                                       |
| `queue_info`   | &check;  |                                                                                       |
| `force_input`  | &check;  | Does not support "at-front" argument, and command executes in parallel not in a queue |
| `flush_input`  |          |                                                                                       |

### Execution

| Name             | Complete | Notes        |
|------------------|----------|--------------|
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
|---------------------------|----------|------------------------------------------------------------------------------------------------------|
| `set_connection_option`   |          |                                                                                                      |
| `connection_option`       |          |                                                                                                      |
| `connection_options`      |          |                                                                                                      |
| `open_network_connection` |          |                                                                                                      |
| `listen`                  | &check;  | `print-messages` not yet implemented. errors in binding not properly propagating back to the builtin |
| `unlisten`                | &check;  |                                                                                                      |
| `listeners`               | &check;  |                                                                                                      |
| `output_delimiters`       |          |                                                                                                      |
| `buffered_output_length`  |          |                                                                                                      |

## Extension from Toast

Functions not in the original LambdaMOO, but were in Toast, and ported over

| Name                   | Complete | Notes                                                               |
|------------------------|----------|---------------------------------------------------------------------|
| `age_generate_keypair` | &check;  | Generates a new X25519 keypair for use with age encryption.         |
| `age_encrypt`          | &check;  | Encrypts a message using age encryption for one or more recipients. |
| `age_decrypt`          | &check;  | Decrypts an age-encrypted message using one or more private keys.   |
| `argon2`               | &check;  | Same signature as function in ToastSunt                             |
| `arong2_verify`        | &check;  | Same signature as function in ToastSunt                             |
| `ftime`                | &check;  | Slight differents in return value, see notes in BfFtime             |
| `encode_base64`        | &check;  |                                                                     |
| `decode_base64`        | &check;  |                                                                     |
| `slice`                | &check;  |                                                                     |
| `generate_json`        | &check;  |                                                                     |
| `parse_json`           | &check;  |                                                                     |
| `ancestors`            | &check;  |                                                                     |
| `descendants`          | &check;  |                                                                     |
| `isa`                  | &check;  |                                                                     |
| `responds_to`          | &check;  |                                                                     |
| `pcre_match`           | &check;  |                                                                     |
| `pcre_replace`         | &check;  |                                                                     |

## Extensions

Functions not part of the original LambdaMOO, but added in moor

### XML / HTML content management

| Name        | Description                                                      | Notes                                                 |
|-------------|------------------------------------------------------------------|-------------------------------------------------------|
| `xml_parse` | Parse a string c ntaining XML into a tree of flyweight objects   | Available only if the flyweights feature is turned on |
| `to_xml`    | Convert a tree of flyweight objects into a string containing XML | Available only if the flyweights feature is turned on |

### Flyweights & Symbols (new types)

| Name          | Description                                                             | Notes                                                 |
|---------------|-------------------------------------------------------------------------|-------------------------------------------------------|
| `slots`       | Returns the slots on a given flyweight                                  | Available only if the flyweights feature is turned on |
| `remove_slot` | Returns a copy of the flyweight with the given slot removed, if present | Available only if the flyweights feature is turned on |
| `add_slot`    | Returns a copy of the flyweight with a new slot added                   | Available only if the flyweights feature is turned on |
| `tosym`       | Turns the given value into a Symbol                                     | Available only if the symbols feature is turned on    |

### Expanded error handling

| Name            | Description                                                                    | Notes |
|-----------------|--------------------------------------------------------------------------------|-------|
| `error_code`    | Strip off any message or value from an error and return only the code portion  |       |
| `error_message` | Return the message portion of the error, or the default message if none exists |       |

### Admin

| Name             | Description                                                     | Notes |
|------------------|-----------------------------------------------------------------|-------|
| `bf_counters`    | Performance counters for profiling builtin function performance |       |
| `db_counters`    | Performance counters for profiling DB performance               |       |
| `sched_counters` | Performance counters for profiling scheduling performance       |       |

### Tasks

| Name           | Description                                                                                                                                                                       | Notes                           |
|----------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------|
| `active_tasks` | Return information about running non-suspended/non-queued tasks which are actively running                                                                                        |                                 |
| `wait_task`    | Causes the current task to wait for a given task id to not be in the background queue                                                                                             |                                 |
| `commit`       | Causes the current task to immediately commit its data, suspend, and then come out of suspension                                                                                  | Semantically same as suspend(0) |
| `rollback`     | Causes the current task to immediately rollback all mutations to the DB and abort the current task. Only argument is boolean whether to send pending content to the player or not | Wizard only                     |
