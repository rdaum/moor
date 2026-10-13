# Structured Data Matching and Pathfinding

These mooR extensions provide MOO-side algorithms for structured MOO values in the form of lists or
maps that would be typically used for games and/or logic programming.

They are intended for cases where MOO code would otherwise spend a large amount of time walking
nested lists, maps, or list-backed data structures in tight loops. They do not inspect the object
database or call back into MOO verbs; instead, callers supply ordinary values such as terms, tile
maps, and edge lists, and the server performs bounded search or traversal over those values more
efficiently than MOO code would.

The functions fall into three related groups. The `term_*` builtins match and transform nested list
and map structures, including small rule queries over caller-supplied facts. The grid functions work
with flat tile maps for pathfinding, line-of-sight checks, and flood fills. The graph functions work
over explicit edge lists for shortest paths, reachability, and topological ordering.

## Terms and small rule searches

The `term_*` functions are for working with _facts_ that you store as ordinary MOO values. A fact
might be an exit record like `{'exit, #10, 'north, #20}`, an event record, a permission rule, or any
other list or map that has a regular shape. Instead of writing loops that pull those records apart
one item at a time, you can describe the shape you want and let the server fill in the blanks.

These functions also let MOO authors use a limited form of
["logic programming"](https://en.wikipedia.org/wiki/Logic_programming) or
["declarative programming"](https://en.wikipedia.org/wiki/Declarative_programming), in the style of
systems like [Datalog](https://en.wikipedia.org/wiki/Datalog) or
[Prolog](https://en.wikipedia.org/wiki/Prolog). In ordinary MOO code, you usually tell the server
exactly what steps to run: loop over this list, check this field, recurse into that connection, and
append each answer yourself. With these functions, you can instead describe facts and rules, then
ask a question about them: "what exits leave this room?", "what rooms are reachable from here?",
"which actions are allowed by these conditions?" The server searches the facts you supplied and
returns the answers it can prove from them.

That style fits text worlds especially well. In
[interactive fiction](https://en.wikipedia.org/wiki/Interactive_fiction), MUDs, and MOOs, a lot of
world logic is about relationships: rooms connected by exits, objects contained by other objects,
locks opened by keys, quests unlocked by previous events, or commands allowed only in certain
situations. The term builtins do not turn MOO into Prolog, and they do not read the object database
for you, but they give MOO code a small, bounded way to work with that kind of relationship data
without spelling out every search loop by hand.

Suppose your world stores exits as records like `{'exit, #10, 'north, #20}`. To find every exit
leaving room `#10`, you might write something like this:

```moo
found = {};
for rec in (exit_records)
  if (rec[2] == #10)
    found = {@found, rec};
  endif
endfor
```

That works for a handful of records. But when the list grows long, or the record shape gets more
complicated, the per-record check slows down and the code to pick apart each record gets harder to
read.

The `term_unify()`, `term_match()`, `term_query()`, and `term_substitute()` builtins make that kind
of structured-data work faster and simpler. You write a term with placeholders such as
`{'var, 'Dir}` and `{'var, 'To}` where unknown values should go. When a stored value fits that
shape, the result tells you what each placeholder matched.

Use `term_unify()` to check one value, `term_match()` to scan a list of values, or `term_query()`
when you want the server to combine facts using simple rules:

```moo
term_query({'exit, #10, {'var, 'Dir}, {'var, 'To}}, exit_records)
=> {['Dir -> 'north, 'To -> #20], ...}
```

For larger cases, `term_query()` can follow simple positive rules over the facts you provide. That
lets MOO code ask questions like "which rooms are reachable from here?" or "which actions are
allowed by these rules?" without hand-writing the nested loops and recursive walks each time.

You can think of the simpler calls as pattern matching for structured data, similar to how
[`match()`](regex.md#match) searches for a regex pattern in a string. The difference is that these
functions work on lists and maps instead of text, and the "captures" are named placeholders rather
than numbered groups. `term_query()` goes one step further: it can use a small rule list to derive
answers from the facts you pass in.

A **pattern** is a MOO value with "holes" in it. Each hole is a placeholder, written as a two-item
list:

```moo
{'var, 'Name}
```

The symbol `'var` marks this list as a placeholder — it tells the server "this is not a normal list,
treat it as a variable". The second item, `'Name`, is the variable's name. Using a symbol like
`'var` as a tag means placeholders are always two-element lists whose first element is `'var`, so
they stand out from ordinary data and can never be confused with a real record that just happens to
contain a symbol called `'var`.

A **term** is just the MOO value you want to check against the pattern. When the structure matches,
every hole gets filled in and you receive a map of the filled-in values.

For example, this pattern:

```moo
{'edge, {'var, 'From}, {'var, 'To}}
```

matches this value:

```moo
{'edge, #10, #11}
```

and `term_unify()` returns this binding map:

```moo
['From -> #10, 'To -> #11]
```

The server does not know what your records mean. A list like `{'edge, #10, #11}` might mean "there
is an edge from `#10` to `#11`", and a map like `["action" -> "property_write", "object" -> #123]`
might mean "write a property on `#123`" — but the server only sees lists, maps, symbols, strings,
objects, and numbers. It matches structure, not meaning.

Common uses for the term functions include:

- Finding everywhere reachable through a graph of exits or connections.
- Searching a list of records for ones that match a particular shape.
- Checking whether a rule or policy matches the current situation.
- Matching event or command descriptions.
- Filling in an action structure from values that were matched.

For example, a builder tool might store exits as `{'exit, from, direction, to}` records, then ask
for all exits whose `from` value is the current room. Or a policy helper might match a validated
request and then fill in an action structure from the values that were matched.

Only lists and map values are searched recursively. Map keys are treated as fixed keys, not
placeholders.

These builtins do not inspect world state, object ownership, task permissions, or call MOO verbs.

## `term_unify`

```moo
MAP | BOOL term_unify(ANY pattern, ANY value [, MAP bindings [, MAP options]])
```

Compares a pattern with exactly one value. On success, returns the extended bindings map. On
mismatch, returns false.

Use `term_unify()` when you have a single value to check — for example, a record you pulled out of a
list with `listassoc()`, or a verb argument you want to match against a command template.

```moo
term_unify({'edge, {'var, 'From}, {'var, 'To}}, {'edge, #10, #11})
=> ['From -> #10, 'To -> #11]
```

Repeated variables must match the same value:

```moo
term_unify({'edge, {'var, 'From}, {'var, 'From}}, {'edge, #10, #11})
=> 0
```

Existing bindings are respected:

```moo
term_unify({'edge, {'var, 'From}, {'var, 'To}},
           {'edge, #10, #11},
           ['From -> #10])
=> ['From -> #10, 'To -> #11]
```

Lists are compared item by item and must have equal length. Maps must have the same keys; values
under those keys are compared recursively. Non-placeholder values compare with normal MOO equality.

## `term_match`

```moo
LIST term_match(ANY pattern, LIST values [, MAP bindings [, MAP options]])
```

Applies one pattern to each value in a list and returns the binding maps for successful matches.
This is the batch form of `term_unify()` for callers that have explicit candidate values and do not
need `term_query()` rules.

```moo
term_match({'edge, #10, {'var, 'To}},
           {
             {'edge, #10, #11},
             {'edge, #10, #12},
             {'edge, #20, #21}
           })
=> {['To -> #11], ['To -> #12]}
```

Initial bindings are applied to each candidate independently. By default duplicate binding maps are
removed.

## `term_query`

```moo
LIST term_query(ANY query, LIST facts [, LIST rules [, MAP bindings [, MAP options]]])
```

Searches caller-supplied facts and optional rules for every way to satisfy one query. This is useful
when MOO code would otherwise run nested loops or recursive walks over list-backed records, such as:

- finding every room reachable through an exit graph,
- expanding a small policy rule set,
- matching event records against rules with shared variables,
- deriving implied records from a supplied list of explicit records.

The server still does not know what your terms mean. It only matches list and map structure, fills
variables, and follows the rules you supplied in the function arguments.

For search over explicit facts, pass one or more facts and omit `rules`:

```moo
term_query({'edge, {'var, 'From}, {'var, 'To}},
           {{'edge, #10, #11}})
=> {['From -> #10, 'To -> #11]}
```

Ordinary mismatch returns an empty list:

```moo
term_query({'edge, {'var, 'From}, {'var, 'From}},
           {{'edge, #10, #11}})
=> {}
```

Repeated variables must match the same value. Initial bindings are respected:

```moo
term_query({'edge, {'var, 'From}, {'var, 'To}},
           {{'edge, #10, #11}},
           {},
           ['From -> #10])
=> {['From -> #10, 'To -> #11]}
```

Lists are compared item by item and must have equal length. Maps must have the same keys; values
under those keys are compared recursively. Non-placeholder values compare with normal MOO equality.

`facts` is a list of terms. Facts may be ground:

```moo
{'edge, #1, #2}
```

or partially ground:

```moo
{'edge, {'var, 'From}, #2}
```

Variables inside facts behave like wildcards for that fact. They are fresh for each attempted match
— each time the query engine tries a fact against a goal, that fact's variables get new internal
names — so they are never returned as output variables and cannot interfere with other matches.

`rules` is a list of `{head, body}` pairs. The head is one term. The body is a list of positive
terms. Each time a rule is tried, its variables are also given fresh internal names, so the same
rule can apply to different values in different search branches without conflict.

```moo
{
  {'reachable, {'var, 'A}, {'var, 'B}},
  {
    {'edge, {'var, 'A}, {'var, 'B}}
  }
}
```

This rule says that `A` reaches `B` when there is an `edge` from `A` to `B`.

Recursive rules are allowed, subject to the search limits and the running task's normal tick budget:

```moo
rules = {
  {
    {'reachable, {'var, 'A}, {'var, 'B}},
    {
      {'edge, {'var, 'A}, {'var, 'B}}
    }
  },
  {
    {'reachable, {'var, 'A}, {'var, 'B}},
    {
      {'edge, {'var, 'A}, {'var, 'C}},
      {'reachable, {'var, 'C}, {'var, 'B}}
    }
  }
};

term_query({'reachable, #1, {'var, 'Where}},
           {
             {'edge, #1, #2},
             {'edge, #2, #3},
             {'edge, #3, #4}
           },
           rules)
=> {['Where -> #2], ['Where -> #3], ['Where -> #4]}
```

Results are a list of binding maps keyed by the variable symbols from the query and any initial
bindings. The order is deterministic: facts and rules are tried in the order supplied.

Rules are restricted in v1:

- Positive Horn-style clauses only.
- No negation.
- No aggregation.
- No cuts.
- No arithmetic or comparison predicates.
- No implicit fact providers.
- No MOO callbacks.
- No world access or permission checks.

Every variable in a rule head must also appear in that rule's positive body. Use `facts` for base
cases rather than bodyless rules.

Malformed variable markers, such as `{'var}`, `{'var, 'A, 'B}`, or `{'var, "A"}`, and malformed
rules raise `E_INVARG`.

MOO symbols compare case-insensitively, so `'From` and `'from` are the same variable identifier.
Initial capitals are only a naming convention.

## `term_substitute`

```moo
ANY term_substitute(ANY template, MAP bindings [, MAP options])
```

Returns a copy of a structured value with every `{'var, name}` placeholder replaced by the
corresponding binding.

If `term_unify()` and `term_query()` are "match patterns and pull values out", then
`term_substitute()` is the companion operation: "push values into a template". You might match a
request with `term_unify()` or `term_query()`, then build a response with `term_substitute()`.

```moo
term_substitute({"property_write", {'var, 'Target}, "description"},
                ['Target -> #123])
=> {"property_write", #123, "description"}
```

By default, unbound variables raise `E_INVARG`. To leave unbound markers in place, pass
`['unbound -> 'leave]`:

```moo
term_substitute({"property_write", {'var, 'Missing}},
                [],
                ['unbound -> 'leave])
=> {"property_write", {'var, 'Missing}}
```

## `term_variables`

```moo
LIST term_variables(ANY term [, MAP options])
```

Returns the unique variable names in a term, in traversal order. Variable names are returned as
symbols.

```moo
term_variables({'edge, {'var, 'From}, {'var, 'To}, {'var, 'From}})
=> {'From, 'To}
```

## `term_ground`

```moo
BOOL term_ground(ANY term [, MAP options])
```

Returns true if the term contains no `{'var, symbol}` markers.

```moo
term_ground({'edge, #10, #11})
=> 1

term_ground({'edge, #10, {'var, 'To}})
=> 0
```

## `term_normalize`

```moo
ANY term_normalize(ANY term [, MAP bindings [, MAP options]])
```

Resolves any supplied bindings, then canonicalizes remaining variables by first encounter:
`{'var, 'V1}`, `{'var, 'V2}`, and so on. This is useful when comparing two terms while ignoring the
specific variable names chosen by the caller.

```moo
term_normalize({'edge, {'var, 'Y}, {'var, 'X}, {'var, 'Y}},
               ['X -> #10])
=> {'edge, {'var, 'V1}, #10, {'var, 'V1}}
```

## Term options

`term_unify()`, `term_substitute()`, `term_variables()`, `term_ground()`, and `term_normalize()`
accept:

```moo
[
  'max_depth -> 64,
  'max_bindings -> 256
]
```

`term_substitute()` also accepts:

```moo
['unbound -> 'raise]  "default"
['unbound -> 'leave]
```

`term_match()` accepts:

```moo
[
  'max_depth -> 64,
  'max_bindings -> 256,
  'max_solutions -> 256,
  'max_steps -> 10000,
  'dedupe -> true
]
```

`term_query()` accepts:

```moo
[
  'max_depth -> 32,
  'max_bindings -> 256,
  'max_solutions -> 256,
  'max_steps -> 10000,
  'dedupe -> true
]
```

`max_depth` limits recursive traversal and, for `term_query()`, recursive rule expansion.
`max_bindings` limits the number of variables that may be present in the binding set.
`max_solutions` stops the query search after that many answers. `max_steps` limits the total amount
of search work before the builtin raises `E_MAXREC`. `term_query()` also consumes the running task's
normal tick budget as it searches, so expensive queries can raise `E_MAXREC` even before `max_steps`
is reached. `dedupe` removes duplicate result maps and suppresses repeated active recursive states.

The term builtins also accept `['case_sensitive -> true]`. This option makes matching and result
deduplication preserve string case, including nested values and string map keys. Symbols retain
their normal case-insensitive identity. The default is `false`, which uses ordinary MOO equality.
Use the same setting when matching facts and checking bound values.

## Grid tile maps

The grid functions use a flat list to represent a two-dimensional tile map. They can be used for map
operations that are awkward or expensive to write as MOO loops: pathfinding, line generation,
line-of-sight checks, and flood fills. Callers supply the tile IDs and the set of solid tile IDs;
the server derives walkability from those values and performs the requested bounded traversal.

The map is laid out left-to-right, top-to-bottom. Think of it as reading the grid row by row, like
lines of text:

```moo
tile_map = {
  1, 1, 2,
  1, 3, 1
}
```

For a width of 3, that represents this 3-by-2 grid:

```text
(0,0)=1  (1,0)=1  (2,0)=2
(0,1)=1  (1,1)=3  (2,1)=1
```

Coordinates are zero-based: `(0, 0)` is the top-left tile, `(width-1, height-1)` is the
bottom-right.

To look up the tile at position `(x, y)` in the list, use:

```moo
index = y * width + x + 1
```

The `+ 1` accounts for MOO lists being one-based.

Each tile has an integer ID. You tell the grid functions which IDs are impassable by passing them in
`solid_tiles`. Any tile whose ID is in that list is treated as a wall; everything else is walkable.
Extra entries in `tile_map` past `width * height` are ignored. Missing entries are treated as
walkable.

## `grid_astar`

```moo
LIST | INT | MAP grid_astar(INT width, INT height,
                            INT start_x, INT start_y,
                            INT goal_x, INT goal_y,
                            LIST tile_map, LIST solid_tiles
                            [, MAP options])
```

Finds the shortest walkable path between two points on a tile grid. You might use it to route an NPC
around walls in a dungeon, or find a walkable route across an overworld. Writing a pathfinder in MOO
is possible but slow for large maps; `grid_astar()` does the search in native code.

By default, returns a list of `{x, y}` waypoints from the start to the goal, excluding the start
position. If no path exists, it returns `{}`. With `['return -> 'cost]`, it returns the path cost
instead, or `-1` when no path exists. With `['return -> 'detail]`, it returns a map with `'path`,
`'cost`, `'visited`, and `'found` entries.

Movement is 8-directional (including diagonals). Diagonal moves are only allowed when both adjacent
cardinal tiles are also walkable, so paths cannot cut through the corner of a wall:

```text
.  .  .
X  .  .          X = solid
.  .  .
```

Moving diagonally from `(0, 0)` to `(1, 1)` might look open — the destination is floor — but the
adjacent tile `(0, 1)` is solid, so the diagonal is blocked. You would have to step to `(1, 0)`
first, then down to `(1, 1)`.

Without that restriction, the path would cut through the corner of the wall tile.

The optional `options` map accepts:

```moo
[
  'directions -> 8,
  'corner_cutting -> false,
  'return -> 'path,
  'max_nodes -> 1000
]
```

`directions` may be `4` or `8`. Four-directional movement only uses north, south, east, and west.
Eight-directional movement is the default and includes diagonals. `corner_cutting` controls whether
diagonal movement may pass between two blocked cardinal neighbors. `return` may be `'path`, `'cost`,
or `'detail`. `max_nodes`, when present, limits the number of grid nodes the search may visit before
raising `E_MAXREC`.

A 3-by-3 open grid with start at `(0, 0)` and goal at `(2, 2)`:

```moo
grid_astar(3, 3,
           0, 0,
           2, 2,
           {
             1, 1, 1,
             1, 1, 1,
             1, 1, 1
           },
           {9})
=> {{1, 1}, {2, 2}}
```

The path reaches the goal in two diagonal steps. Diagonal moves cost the same as cardinal moves, so
this is a shortest valid path.

The exact path is deterministic for a given map, but callers should treat any shortest valid path as
acceptable. If the goal tile is solid, or no route reaches it, the function returns `{}`. Invalid
dimensions or out-of-bounds start/goal coordinates raise `E_INVARG`.

`grid_astar()` consumes the running task's normal tick budget as it searches. Large or difficult
path requests can therefore raise `E_MAXREC` if the task runs out of ticks before the search
finishes, or if the optional `max_nodes` limit is reached.

## `grid_line`

```moo
LIST grid_line(INT width, INT height,
               INT x0, INT y0,
               INT x1, INT y1)
```

Returns the inclusive Bresenham line between two in-bounds grid coordinates. Both endpoints are
included in the returned list.

```moo
grid_line(5, 5, 0, 0, 4, 2)
=> {{0, 0}, {1, 1}, {2, 1}, {3, 2}, {4, 2}}
```

`grid_line()` consumes the running task's normal tick budget as it generates the line.

## `grid_los`

```moo
BOOL grid_los(INT width, INT height,
              INT x0, INT y0,
              INT x1, INT y1,
              LIST tile_map, LIST solid_tiles)
```

Returns true if every tile on the inclusive line between the two points is walkable. A solid start
or end tile blocks line of sight.

`grid_los()` consumes the running task's normal tick budget as it checks the line.

## `grid_reachable`

```moo
LIST grid_reachable(INT width, INT height,
                    INT start_x, INT start_y,
                    LIST tile_map, LIST solid_tiles
                    [, MAP options])
```

Returns every coordinate reachable from the start tile, excluding the start tile itself. If the
start tile is solid, it returns `{}`.

`grid_reachable()` accepts:

```moo
[
  'directions -> 8,
  'corner_cutting -> false,
  'max_nodes -> 1000
]
```

`directions`, `corner_cutting`, and `max_nodes` have the same meaning as the `grid_astar()` options.
The search consumes the running task's normal tick budget and may raise `E_MAXREC` if the task runs
out of ticks or `max_nodes` is reached.

## `grid_flood`

```moo
LIST grid_flood(INT width, INT height,
                INT start_x, INT start_y,
                LIST tile_map, LIST solid_tiles
                [, MAP options])
```

Returns the flood-filled region reachable from the start tile, including the start tile. This is the
same search as `grid_reachable()`, but with the origin included for callers that want the full
component.

`grid_flood()` accepts:

```moo
[
  'directions -> 8,
  'corner_cutting -> false,
  'max_nodes -> 1000
]
```

`directions`, `corner_cutting`, and `max_nodes` have the same meaning as the `grid_astar()` options.
The search consumes the running task's normal tick budget and may raise `E_MAXREC` if the task runs
out of ticks or `max_nodes` is reached.

## Graph edge lists

The graph helpers are for data that describes things connected to other things. For example, rooms
may be connected by exits, tasks may depend on other tasks, or quests may unlock later quests. In
this section, each thing is called a **node**, and each one-way connection from one thing to another
is called an **edge**.

You supply the connections as a list of edges. Each edge says "from this node, you can go to that
node". Edges may be written as two-item lists, simple edge terms, or maps:

```moo
{from, to}
{'edge, from, to}
['from -> from, 'to -> to]
```

Node values may be any MOO value that can be compared as a map/list key. Edge order is preserved
when traversing neighbors, so results are deterministic for a fixed input list.

Graph helpers accept:

```moo
[
  'max_steps -> 10000,
  'max_nodes -> 1000
]
```

They consume the running task's normal tick budget as they traverse the graph and may raise
`E_MAXREC` when either explicit limit is reached.

## `graph_shortest_path`

```moo
LIST graph_shortest_path(LIST edges, ANY from, ANY to [, MAP options])
```

Returns an unweighted shortest path from `from` to `to`, including both endpoints. If no path
exists, it returns `{}`.

```moo
graph_shortest_path({{'A, 'B}, {'B, 'C}, {'A, 'C}}, 'A, 'C)
=> {'A, 'C}
```

## `graph_reachable`

```moo
LIST graph_reachable(LIST edges, ANY from [, MAP options])
```

Returns every node reachable from `from`, excluding `from` itself.

```moo
graph_reachable({{'A, 'B}, {'A, 'C}, {'B, 'D}}, 'A)
=> {'B, 'C, 'D}
```

## `graph_topsort`

```moo
LIST graph_topsort(LIST edges [, MAP options])
```

Returns a topological ordering of the directed graph. If the graph contains a cycle,
`graph_topsort()` raises `E_INVARG`.

```moo
graph_topsort({{'cook, 'eat}, {'shop, 'cook}})
=> {'shop, 'cook, 'eat}
```
