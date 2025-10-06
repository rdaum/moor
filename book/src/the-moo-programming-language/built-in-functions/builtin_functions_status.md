## Status of builtin function implementation

The following is a table of the status of the implementation of various builtin-functions as defined in the LambdaMOO
1.8
specification, as well as some extensions that were added in ToastStunt and then ported over to mooR. (And some novel
extensions added in mooR itself.)

The table is broken down by category, and each function is marked with a checkmark if it is implemented.

If there are any notes about the implementation, they will be included in the notes column. If you notice anything
missing, or if you have any questions about the implementation, please feel free to open an issue on the [mooR Codeberg
repository issue tracker](https://codeberg.org/timbran/moor/issues).

## LambdaMOO 1.8 builtin function list and status

### Lists

| Name                                          | Complete | Notes                                          |
|-----------------------------------------------|----------|------------------------------------------------|
| [`length`](values.md#length)                  | &check;  |                                                |
| [`setadd`](list_sets.md#setadd)               | &check;  |                                                |
| [`setremove`](list_sets.md#setremove)         | &check;  |                                                |
| [`listappend`](list_sets.md#listappend)       | &check;  |                                                |
| [`listinsert`](list_sets.md#listinsert)       | &check;  |                                                |
| [`listdelete`](list_sets.md#listdelete)       | &check;  |                                                |
| [`listset`](list_sets.md#listset)             | &check;  |                                                |
| [`equal`](values.md#equal)                    | &check;  |                                                |
| [`is_member`](list_sets.md#is_member)         | &check;  |                                                |
| [`match`](regex.md#match)                     | &check;  |                                                |
| [`rmatch`](regex.md#rmatch)                   | &check;  |                                                |
| [`substitute`](regex.md#substitute)           | &check;  |                                                |
| [`complex_match`](list_sets.md#complex_match) | &check;  | Advanced pattern matching with ordinal support |

### Strings

| Name                                | Complete | Notes                                                                                                |
|-------------------------------------|----------|------------------------------------------------------------------------------------------------------|
| [`tostr`](strings.md#tostr)         | &check;  |                                                                                                      |
| [`toliteral`](strings.md#toliteral) | &check;  |                                                                                                      |
| [`crypt`](crypto.md#crypt)          | &check;  | Pretty damned insecure, only here to support existing core password functions.                       |
| [`index`](strings.md#index)         | &check;  |                                                                                                      |
| [`rindex`](strings.md#rindex)       | &check;  |                                                                                                      |
| [`strcmp`](strings.md#strcmp)       | &check;  |                                                                                                      |
| [`strsub`](strings.md#strsub)       | &check;  |                                                                                                      |
| [`salt`](crypto.md#salt)            | &check;  | Generate a random crypto-secure salt for password. Not compatible with toast's function of same name |

### Numbers

| Name                          | Complete | Notes |
|-------------------------------|----------|-------|
| [`toint`](num.md#toint)       | &check;  |       |
| [`tonum`](num.md#tonum)       | &check;  |       |
| [`tofloat`](num.md#tofloat)   | &check;  |       |
| [`min`](num.md#min)           | &check;  |       |
| [`max`](num.md#max)           | &check;  |       |
| [`abs`](num.md#abs)           | &check;  |       |
| [`random`](num.md#random)     | &check;  |       |
| [`time`](num.md#time)         | &check;  |       |
| [`ctime`](num.md#ctime)       | &check;  |       |
| [`floatstr`](num.md#floatstr) | &check;  |       |
| [`sqrt`](num.md#sqrt)         | &check;  |       |
| [`sin`](num.md#sin)           | &check;  |       |
| [`cos`](num.md#cos)           | &check;  |       |
| [`tan`](num.md#tan)           | &check;  |       |
| [`asin`](num.md#asin)         | &check;  |       |
| [`acos`](num.md#acos)         | &check;  |       |
| [`atan`](num.md#atan)         | &check;  |       |
| [`sinh`](num.md#sinh)         | &check;  |       |
| [`cosh`](num.md#cosh)         | &check;  |       |
| [`tanh`](num.md#tanh)         | &check;  |       |
| [`exp`](num.md#exp)           | &check;  |       |
| [`log`](num.md#log)           | &check;  |       |
| [`log10`](num.md#log10)       | &check;  |       |
| [`ceil`](num.md#ceil)         | &check;  |       |
| [`floor`](num.md#floor)       | &check;  |       |
| [`trunc`](num.md#trunc)       | &check;  |       |

### Objects

| Name                                            | Complete | Notes                              |
|-------------------------------------------------|----------|------------------------------------|
| [`toobj`](objects.md#toobj)                     | &check;  |                                    |
| [`typeof`](values.md#typeof)                    | &check;  |                                    |
| [`create`](objects.md#create)                   | &check;  | Quota support not implemented yet. |
| [`recycle`](objects.md#recycle)                 | &check;  |                                    |
| [`valid`](objects.md#valid)                     | &check;  |                                    |
| [`parent`](objects.md#parent)                   | &check;  |                                    |
| [`children`](objects.md#children)               | &check;  |                                    |
| [`chparent`](objects.md#chparent)               | &check;  |                                    |
| [`max_object`](objects.md#max_object)           | &check;  |                                    |
| [`players`](objects.md#players)                 | &check;  | Potentially slow in a large DB.    |
| [`is_player`](objects.md#is_player)             | &check;  |                                    |
| [`set_player_flag`](objects.md#set_player_flag) | &check;  |                                    |
| [`is_anonymous`](objects.md#is_anonymous)       | &check;  |                                    |
| [`is_uuobjid`](objects.md#is_uuobjid)           | &check;  |                                    |
| [`move`](objects.md#move)                       | &check;  |                                    |

### Properties

| Name                                                   | Complete | Notes |
|--------------------------------------------------------|----------|-------|
| [`properties`](properties.md#properties)               | &check;  |       |
| [`property_info`](properties.md#property_info)         | &check;  |       |
| [`set_property_info`](properties.md#set_property_info) | &check;  |       |
| [`add_property`](properties.md#add_property)           | &check;  |       |
| [`delete_property`](properties.md#delete_property)     | &check;  |       |
| [`clear_property`](properties.md#clear_property)       | &check;  |       |
| [`is_clear_property`](properties.md#is_clear_property) | &check;  |       |

### Verbs

| Name                                      | Complete | Notes                                 |
|-------------------------------------------|----------|---------------------------------------|
| [`verbs`](verbs.md#verbs)                 | &check;  |                                       |
| [`verb_info`](verbs.md#verb_info)         | &check;  |                                       |
| [`set_verb_info`](verbs.md#set_verb_info) | &check;  |                                       |
| [`verb_args`](verbs.md#verb_args)         | &check;  |                                       |
| [`set_verb_args`](verbs.md#set_verb_args) | &check;  |                                       |
| [`add_verb`](verbs.md#add_verb)           | &check;  |                                       |
| [`delete_verb`](verbs.md#delete_verb)     | &check;  |                                       |
| [`set_verb_code`](verbs.md#set_verb_code) | &check;  |                                       |
| [`eval`](verbs.md#eval)                   | &check;  |                                       |
| [`disassemble`](verbs.md#disassemble)     | &check;  | Output looks nothing like LambdaMOO's |
| [`verb_code`](verbs.md#verb_code)         | &check;  |                                       |

### Values / encoding

| Name                                        | Complete | Notes                                                                              |
|---------------------------------------------|----------|------------------------------------------------------------------------------------|
| [`value_bytes`](values.md#value_bytes)      | &check;  |                                                                                    |
| [`value_hash`](values.md#value_hash)        |          |                                                                                    |
| [`string_hash`](values.md#string_hash)      | &check;  |                                                                                    |
| [`binary_hash`](values.md#binary_hash)      |          |                                                                                    |
| [`decode_binary`](strings.md#decode_binary) |          | Binary encoding will likely work differently in moor. See README.md for more info. |
| [`encode_binary`](strings.md#encode_binary) |          |                                                                                    |
| [`object_bytes`](values.md#object_bytes)    | &check;  |                                                                                    |

### Server

| Name                                                   | Complete | Notes                                                                    |
|--------------------------------------------------------|----------|--------------------------------------------------------------------------|
| [`server_version`](server.md#server_version)           | &check;  | Crate version + short commit hash, for now                               |
| [`renumber`](objects.md#renumber)                      | &check;  | Supports UUID to numbered conversion with auto-selection                 |
| [`reset_max_object`](server.md#reset_max_object)       |          |                                                                          |
| [`memory_usage`](server.md#memory_usage)               | &check;  |                                                                          |
| [`shutdown`](server.md#shutdown)                       | &check;  |                                                                          |
| [`dump_database`](server.md#dump_database)             | &check;  |                                                                          |
| [`db_disk_size`](server.md#db_disk_size)               | &check;  |                                                                          |
| [`connected_players`](server.md#connected_players)     | &check;  |                                                                          |
| [`connected_seconds`](server.md#connected_seconds)     | &check;  |                                                                          |
| [`idle_seconds`](server.md#idle_seconds)               | &check;  |                                                                          |
| [`connection_name`](server.md#connection_name)         | &check;  | To make this 100% compat with core, reverse DNS & listen port is needed. |
| [`connections`](server.md#connections)                 | &check;  | Returns connections for current player, or other players.                |  mooR extension. |
| [`notify`](server.md#notify)                           | &check;  | With `rich_notify` feature on, supports sending additional content types |
| [`boot_player`](server.md#boot_player)                 | &check;  |                                                                          |
| [`server_log`](server.md#server_log)                   | &check;  |                                                                          |
| [`load_server_options`](server.md#load_server_options) |          |                                                                          |
| [`function_info`](server.md#function_info)             | &check;  |                                                                          |
| [`read`](server.md#read)                               | &check;  |                                                                          |

### Tasks

| Name                                     | Complete | Notes                                                                                 |
|------------------------------------------|----------|---------------------------------------------------------------------------------------|
| [`task_id`](server.md#task_id)           | &check;  |                                                                                       |
| [`queued_tasks`](server.md#queued_tasks) | &check;  |                                                                                       |
| [`kill_task`](server.md#kill_task)       | &check;  |                                                                                       |
| [`resume`](server.md#resume)             | &check;  |                                                                                       |
| [`queue_info`](server.md#queue_info)     | &check;  |                                                                                       |
| [`force_input`](server.md#force_input)   | &check;  | Does not support "at-front" argument, and command executes in parallel not in a queue |
| [`flush_input`](server.md#flush_input)   |          |                                                                                       |

### Execution

| Name                                         | Complete | Notes        |
|----------------------------------------------|----------|--------------|
| [`call_function`](server.md#call_function)   | &check;  |              |
| [`raise`](server.md#raise)                   | &check;  |              |
| [`suspend`](server.md#suspend)               | &check;  |              |
| [`seconds_left`](server.md#seconds_left)     | &check;  |              |
| [`ticks_left`](server.md#ticks_left)         | &check;  |              |
| [`pass`](server.md#pass)                     | &check;  | Is an opcode |
| [`set_task_perms`](server.md#set_task_perms) | &check;  |              |
| [`caller_perms`](server.md#caller_perms)     | &check;  |              |
| [`callers`](server.md#callers)               | &check;  |              |
| [`task_stack`](server.md#task_stack)         |          |              |

### Network connections

mooR handles outbound networking differently than classic LambdaMOO - see
the [networking section](../networking.md#outbound-network-connections-via-curl_worker) for details on using workers for
outbound connections.

| Name                                                           | Complete | Notes                                                                                                                                         |
|----------------------------------------------------------------|----------|-----------------------------------------------------------------------------------------------------------------------------------------------|
| [`set_connection_option`](server.md#set_connection_option)     | &check;  | Supports binary, hold-input, disable-oob, client-echo, flush-command options. See [networking](../networking.md) for details.                |
| [`connection_option`](server.md#connection_option)             | &check;  | Works only for connections, not player objects, since moor has multiple connections per player. `connections(player)` returns all connections |
| [`connection_options`](server.md#connection_options)           | &check;  | Works only for connections, not player objects, since moor has multiple connections per player. `connections(player)` returns all connections |
| [`open_network_connection`](server.md#open_network_connection) |          | Not planned - use worker system instead                                                                                                       |
| [`listen`](server.md#listen)                                   | &check;  | `print-messages` not yet implemented. errors in binding not properly propagating back to the builtin                                          |
| [`unlisten`](server.md#unlisten)                               | &check;  |                                                                                                                                               |
| [`listeners`](server.md#listeners)                             | &check;  |                                                                                                                                               |
| [`output_delimiters`](server.md#output_delimiters)             | &check;  |                                                                                                                                               |
| [`connection_attributes`](server.md#connection_attributes)     | &check;  | mooR extension - returns map/list based on features                                                                                           |
| [`buffered_output_length`](server.md#buffered_output_length)   |          | Not planned                                                                                                                                   |

## Extension from Toast

Functions not in the original LambdaMOO, but were in Toast, and ported over

| Name                                                     | Complete | Notes                                                               |
|----------------------------------------------------------|----------|---------------------------------------------------------------------|
| [`age_generate_keypair`](crypto.md#age_generate_keypair) | &check;  | Generates a new X25519 keypair for use with age encryption.         |
| [`age_encrypt`](crypto.md#age_encrypt)                   | &check;  | Encrypts a message using age encryption for one or more recipients. |
| [`age_decrypt`](crypto.md#age_decrypt)                   | &check;  | Decrypts an age-encrypted message using one or more private keys.   |
| [`argon2`](crypto.md#argon2)                             | &check;  | Same signature as function in ToastSunt                             |
| [`arong2_verify`](crypto.md#argon2_verify)               | &check;  | Same signature as function in ToastSunt                             |
| [`ftime`](server.md#ftime)                               | &check;  | Slight differents in return value, see notes in BfFtime             |
| [`encode_base64`](strings.md#encode_base64)              | &check;  |                                                                     |
| [`decode_base64`](strings.md#decode_base64)              | &check;  |                                                                     |
| [`slice`](values.md#slice)                               | &check;  |                                                                     |
| [`generate_json`](strings.md#generate_json)              | &check;  |                                                                     |
| [`parse_json`](strings.md#parse_json)                    | &check;  |                                                                     |
| [`ancestors`](objects.md#ancestors)                      | &check;  |                                                                     |
| [`descendants`](objects.md#descendants)                  | &check;  |                                                                     |
| [`isa`](objects.md#isa)                                  | &check;  |                                                                     |
| [`responds_to`](objects.md#responds_to)                  | &check;  |                                                                     |
| [`pcre_match`](strings.md#pcre_match)                    | &check;  |                                                                     |
| [`pcre_replace`](strings.md#pcre_replace)                | &check;  |                                                                     |

## Extensions

Functions not part of the original LambdaMOO, but added in moor

### XML / HTML content management

| Name        | Description                                                      | Notes                                                 |
|-------------|------------------------------------------------------------------|-------------------------------------------------------|
| `xml_parse` | Parse a string containing XML into a tree of flyweight objects   | Available only if the flyweights feature is turned on |
| `to_xml`    | Convert a tree of flyweight objects into a string containing XML | Available only if the flyweights feature is turned on |

### Import/Export of Objects

| Name                                                              | Description                                                                                        | Notes             
|-------------------------------------------------------------------|----------------------------------------------------------------------------------------------------|-------------------|
| [`load_object`](../../the-system/object-packaging.md#load_object) | Load an object from objdef format with optional conflict detection and resolution options.         | Wiz only          |
| `dump_object`                                                     | Takes an object and returns a list of strings representing the object definition in objdef format. | Wiz or owner only |

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

| Name                                     | Description                                                                                                                                                                       | Notes                           |
|------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------|
| [`active_tasks`](server.md#active_tasks) | Return information about running non-suspended/non-queued tasks which are actively running                                                                                        |                                 |
| [`wait_task`](server.md#wait_task)       | Causes the current task to wait for a given task id to not be in the background queue                                                                                             |                                 |
| [`commit`](server.md#commit)             | Causes the current task to immediately commit its data, suspend, and then come out of suspension                                                                                  | Semantically same as suspend(0) |
| [`rollback`](server.md#rollback)         | Causes the current task to immediately rollback all mutations to the DB and abort the current task. Only argument is boolean whether to send pending content to the player or not | Wizard only                     |
