# `.moot` file syntax

| Syntax                       | Meaning                                                                            |
| ---------------------------- | ---------------------------------------------------------------------------------- |
| `@programmer`, `@wizard`     | Execute everything after this as a programmer / as a wizard                        |
| `; return 42;`               | `eval("return 42")`                                                                |
| <pre>; return<br>> 42;</pre> | Multi-line `eval`                                                                  |
| <pre>% look<br>> here</pre>  | Multi-line command                                                                 |
| `% look`                     | Execute `look` as a command                                                        |
| `& return read();`           | `eval("return read();")`, but don't expect any output                              |
| `42`, `< 42`                 | Assert that we receive `42` from the server as a response to the `eval` or command |
| `=foobar`                    | Assert that we received a line containing exactly the string `foobar`              |
| `// comment`                 | It's a comment!                                                                    |

## Notes: `42`, `< 42`

For this style of assertion we send the read string through an extra round of `eval`. This means assertions can use variables; for example `player` is a valid assertion that will resolve to the active player object. This _also_ means that assertions must be valid MOO expressions; in particular, strings must be quoted.

Consecutive lines in this style are treated as a single MOO expression; this allows for better readability if the expected output is complex.

## Notes: `=foobar`

Unlike `42` / `< 42`, there's no extra evaluation happening. We just look at raw string lines, and compare those.

This style of assertion is only supported on tests using a network connection. That is: this works on `TelnetMootRunner`, but is unimplemented on the `SchedulerMootRunner` used in the `moor-kernel` crate. _STRONGLY_ prefer using this kind of assertion in these networked tests!

## Notes: extraneous command output

Assertions are evaluated _exactly_ when the relevant line is read. This means commands may be interspersed with output assertions arbitrarily:

```
; return 42;
; return 101;
42
101
```

This is required to implement tests for more complex flows, for example those involving `read()`. Unfortunately this allows some non-trivial failure modes:

- We _think_ we're asserting the output of a command, but are in fact still processing extraneous the output from the previous one
- Output not consumed by any assertions cause a test failure at the very end of the test file
