## Status of builtin function implementation

The following is a table of the status of various builtin-functions, to keep an inventory of what remains to be done.

### Lists

| Name       | Complete | Notes |
|------------|----------|-------|
| length     | &check;  |       |
| setadd     | &check;  |       |
| setremove  | &check;  |       |
| listappend | &check;  |       |
| listinsert | &check;  |       |
| listdelete | &check;  |       |
| listset    | &check;  |       |
| equal      | &check;  |       |
| is_member  | &check;  |       |

### Strings

| Name       | Complete | Notes                                          |
|------------|----------|------------------------------------------------|
| tostr      | &check;  |                                                |
| toliteral  | &check;  |                                                |
| match      | &check;  |                                                |
| rmatch     |          | Just alter indices for the regexp match range. |
| substitute | &check;  | Might need more testing.                       |
| crypt      | &check;  | DES                                            |
| index      | &check;  |                                                |
| rindex     | &check;  |                                                |
| strcmp     | &check;  |                                                |
| strsub     | &check;  |                                                |

### Numbers

| Name     | Complete | Notes |
|----------|----------|-------|
| toint    | &check;  |       |
| tonum    |          |       |
| tofloat  | &check;  |       |
| min      | &check;  |       |
| max      | &check;  |       |
| abs      | &check;  |       |
| random   | &check;  |       |
| time     | &check;  |       |
| ctime    |          |       |
| floatstr | &check;  |       |
| sqrt     | &check;  |       |
| sin      | &check;  |       |
| cos      | &check;  |       |
| tan      | &check;  |       |
| asin     | &check;  |       |
| acos     | &check;  |       |
| atan     | &check;  |       |
| sinh     | &check;  |       |
| cosh     | &check;  |       |
| tanh     | &check;  |       |
| exp      | &check;  |       |
| log      | &check;  |       |
| log10    | &check;  |       |
| ceil     | &check;  |       |
| floor    | &check;  |       |
| trunc    | &check;  |       |

### Objects

| Name            | Complete | Notes |
|-----------------|----------|-------|
| toobj           | &check;  |       |
| typeof          | &check;  |       |
| create          |          |       |
| recycle         |          |       |
| valid           | &check;  |       |
| parent          | &check;  |       |
| children        | &check;  |       |
| chparent        |          |       |
| max_object      |          |       |
| players         |          |       |
| is_player       | &check;  |       |
| set_player_flag |          |       |
| move            |          |       |

### Properties

| Name              | Complete | Notes |
|-------------------|----------|-------|
| properties        | &check;  |       |
| property_info     | &check;  |       |
| set_property_info | &check;  |       |
| add_property      |          |       |
| delete_property   |          |       |
| clear_property    | &check;  |       |
| is_clear_property | &check;  |       |

### Verbs

| Name          | Complete | Notes                                    |
|---------------|----------|------------------------------------------|
| verbs         | &check;  |                                          |
| verb_info     | &check;  |                                          |
| set_verb_info | &check   |                                          |
| verb_args     | &check;  |                                          |
| set_verb_args | &check;  |                                          |
| add_verb      |          |                                          |
| delete_verb   |          |                                          |
| verb_code     |          |                                          |
| set_verb_code |          |                                          |
| eval          |          |                                          |
| disassemble   |          | Requires implementation of decompilation |

### Values / encoding

| Name          | Complete | Notes |
|---------------|----------|-------|
| value_bytes   |          |       |
| value_hash    |          |       |
| string_hash   | &check;  |       |
| binary_hash   |          |       |
| decode_binary |          |       |
| encode_binary |          |       |
| object_bytes  |          |       |

### Server

| Name                | Complete | Notes                                         |
|---------------------|----------|-----------------------------------------------|
| server_version      | &check;  | Hardcoded value, should derive from bin crate |
| renumber            |          |                                               |
| reset_max_object    |          |                                               |
| memory_usage        |          |                                               |
| shutdown            |          |                                               |
| dump_database       |          |                                               |
| db_disk_size        |          |                                               |
| connected_players   | &check;  |                                               |
| connected_seconds   | &check   |                                               |
| idle_seconds        | &check   |                                               |
| connection_name     |          |                                               |
| notify              | &check;  |                                               |
| boot_player         |          |                                               |
| server_log          |          |                                               |
| load_server_options |          |                                               |
| function_info       |          |                                               |



### Tasks

| Name              | Complete | Notes |
|-------------------|----------|-------|
| task_id           | &check;  |       |
| queued_tasks      |          |       |
| kill_task         |          |       |
| output_delimiters |          |       |
| queue_info        |          |       |
| resume            |          |       |
| force_input       |          |       |
| flush_input       |          |       |


### Execution

| Name           | Complete | Notes                                        |
|----------------|----------|----------------------------------------------|
| call_function  |          |                                              |
| raise          | &check;  | Does not support message / value parameters. |
| suspend        |          |                                              |
| seconds_left   |          |                                              |
| ticks_left     |          |                                              |
| pass           | &check;  | Is an opcode                                 |
| set_task_perms | &check;  | Check correctness                            |
| caller_perms   | &check;  | Check correctness.                           |
| callers        | &check;  |                                              |
| task_stack     |          |                                              |

### Network connections

These will likely never be implemented. But should return, e.g. E_PERM or similar
to the caller if attempted.

| Name                    | Complete | Notes |
|-------------------------|----------|-------|
| set_connection_option   |          |       |
| connection_option       |          |       |
| connection_options      |          |       |
| open_network_connection |          |       |
| listen                  |          |       |
| unlisten                |          |       |
| read                    |          |       |
| listeners               |          |       |
| buffered_output_length  |          |       |