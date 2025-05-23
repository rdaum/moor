This is mostly a port of (part of) the regression suite bundled with Stunt MOO:

https://github.com/toddsundsted/stunt/tree/master/test

| Status | `stunt` test(s)                                | `moor` test(s)               | Notes                                                                                       |
| ------ | ---------------------------------------------- | ---------------------------- | ------------------------------------------------------------------------------------------- |
| ✅     | `test_algorithms.rb`                           | `algorithms.moot`            | `stunt` added multiple hashing algorithms (not supported in `moor`). Fuzz tests not ported. |
| 🚫     | `test_anonymous.rb`                            | N/A                          | `moor` doesn't support anonymous objects.                                                   |
| ✅     | `test_basic.rb`, `basic/*`                     | `basic/*.moot`               |                                                                                             |
| 🔜     | `test_canned_dbs.rb`                           | N/A                          |                                                                                             |
| 🤔     | `test_collection_improvements.rb`              | N/A                          | Are these tests valuable / relevant for `moor`?                                             |
| ✅     | `test_create.rb`                               | `create.moot`                |                                                                                             |
| ✅     | `test_equality.rb`                             | `equality.moot`              |                                                                                             |
| ✅     | `test_eval.rb`                                 | `eval/*.moot`                |                                                                                             |
| 🚫     | `test_exec.rb`                                 | N/A                          | `moor` doesn't support this Stunt extension.                                                |
| 🚫     | `test_fileio.rb`                               | N/A                          | `moor` doesn't support this Stunt extension.                                                |
| 🚫     | `test_garbage_collection.rb`                   | N/A                          | `moor` doesn't support this Stunt extension.                                                |
| 🚫     | `test_http.rb`                                 | N/A                          | `moor` doesn't support this Stunt extension.                                                |
| ✅     | `test_huh.rb`                                  | `huh.moot`                   | See also `huh` test in the `telnet-host` crate.                                             |
| 🚫     | `test_index_and_range_extensions.rb`           | N/A                          | `moor` doesn't support this Stunt extension.                                                |
| 🚫     | `test_json.rb`                                 | N/A                          | `moor` doesn't support this Stunt extension.                                                |
| 🔜     | `test_limits.rb`                               | N/A                          |                                                                                             |
| ✅     | `test_looping.rb`                              | `looping.moot`               |                                                                                             |
| ✅     | ️`test_map.rb`                                  | `map.moot`                   |                                                                                             |
| ✅     | `test_math.rb`                                 | `math.moot`                  |                                                                                             |
| 🚫     | `test_miscellaneous.rb`                        | N/A                          | `moor` doesn't support this Stunt extension (`isa`)                                         |
| ✅     | `test_moocode_parsing.rb`                      | N/A                          | Dropped tests for Stunt extensions (`^` collection, bitwise operators)                      |
| ✅     | `test_objects.rb`                              | `objects/{test_method}.moot` | See `test_objects` heading below                                                            |
| 🔜     | `test_objects_and_properties.rb`               | N/A                          |                                                                                             |
| 🔜     | `test_objects_and_verbs.rb`                    | N/A                          |                                                                                             |
| 🔜     | `test_primitives.rb`                           | N/A                          |                                                                                             |
| ✅     | `test_recycle.rb`                              | `recycle.moot`               |                                                                                             |
| 🔜     | `test_stress_objects.rb`                       | N/A                          |                                                                                             |
| ✅     | `test_string_operations.rb`                    | `string_operations.moot`     | Extended with cases based on LambdaMOO Programmer's Manual                                  |
| 🚫     | `test_switch_player.rb`                        | N/A                          | `moor` doesn't support this Stunt extension.                                                |
| 🚫     | `test_system_builtins.rb`                      | N/A                          | `moor` doesn't support this Stunt extension (`getenv`).                                     |
| 🔜     | `test_task_local.rb`                           | N/A                          |                                                                                             |
| 🔜     | `test_things_that_used_to_crash_the_server.rb` | N/A                          | Probably useful to test these, since they were tricky for another server at somepoint       |
| 🚫     | `test_verb_cache.rb`                           | N/A                          | `moor` doesn't support this Stunt extension.                                                |

`.moot` files not mentioned above are not related to Stunt.

## `test_objects`

- `moor` doesn't support multiple inheritence.
- Stunt changes how `create()` behaves when arguments are not a valid object reference. `moor` follows the LambdaMOO behavior.
  - https://github.com/toddsundsted/stunt/blob/e83e946/test/test_objects.rb#L65-L75
  - https://github.com/toddsundsted/stunt/blob/e83e946/test/test_objects.rb#L84-L94
  - https://github.com/toddsundsted/stunt/blob/e83e946/test/test_objects.rb#L106-L111
  - https://github.com/toddsundsted/stunt/blob/e83e946/test/test_objects.rb#L164-L169
- `renumber` is not currently implemented in `moor`

## Useful Vim commands

To speed up migrating Stunt tests. These are pretty rough, but a good way to take care of ~80% of the lines.
