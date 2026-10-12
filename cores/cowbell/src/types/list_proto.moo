object LIST_PROTO [
  import_export_id -> "list_proto",
  import_export_hierarchy -> {"types"}
]
  name: "List Utilities"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") =
    "Prototype object for list utility methods and functional programming operations.";

  method append owner: HACKER
    "Return a single list containing every element from each argument list, in order.";
    "Example: append({a, b, c}, {d, e}, {}, {f, g, h}) => {a, b, c, d, e, f, g, h}.";
    const n = length(args);
    if (n > 50)
      return {@this:append(@args[1..n / 2]), @this:append(@args[n / 2 + 1..n])};
    endif
    let result = {};
    for items in (args)
      result = {@result, @items};
    endfor
    return result;
  endmethod

  method assoc owner: HACKER
    "Return the first list element whose index-th value equals target; index defaults to 1.";
    "Returns {} when no matching nested list is found.";
    const {lst, target, ?indx = 1} = args;
    for t in (lst)
      if (typeof(t) != TYPE_LIST || length(t) < indx)
        continue;
      endif
      if (t[indx] == target)
        return t;
      endif
    endfor
    return {};
  endmethod

  method assoc_prefix owner: HACKER
    "Return the first list element whose index-th string value starts with target; index defaults to 1.";
    "Returns {} when no matching nested list is found.";
    const {lst, target, ?indx = 1} = args;
    for t in (lst)
      if (typeof(t) != TYPE_LIST || length(t) < indx)
        continue;
      endif
      if (typeof(t[indx]) == TYPE_STR && index(t[indx], target) == 1)
        return t;
      endif
    endfor
    return {};
  endmethod

  method check_type owner: HACKER
    "Return true if every list element has one of the requested MOO type constants.";
    "The type argument may be a single type constant or a list of type constants.";
    const {items, types} = args;
    const typelist = typeof(types) == TYPE_LIST ? types | {types};
    for element in (items)
      if (!(typeof(element) in typelist))
        return false;
      endif
    endfor
    return true;
  endmethod

  method compress owner: HACKER
    "Return a list with consecutive repeated elements collapsed to one occurrence.";
    "Example: compress({a, b, b, c, b, b, b, d, d, e}) => {a, b, c, b, d, e}.";
    const {items} = args;
    !items && return {};
    let result = {items[1]};
    for i in [2..length(items)]
      if (items[i] != result[$])
        result = listappend(result, items[i]);
      endif
    endfor
    return result;
  endmethod

  method join owner: HACKER
    "Return the list elements converted to strings and joined by separator, which defaults to a space.";
    const {l, ?separator = " "} = args;
    typeof(separator) == TYPE_STR
      || raise(E_TYPE("join() separator must be string; got " + toliteral(separator)));
    length(l) == 0 && return "";
    length(l) == 1 && return tostr(l[1]);
    let result = tostr(l[1]);
    for i in [2..length(l)]
      result = result + separator + tostr(l[i]);
    endfor
    return result;
  endmethod

  method english_list owner: #184
    "Return a human-readable English list such as \"a, b, and c\" or \"a and b\".";
    "Optional arguments customize the empty-list text, conjunction, comma separator, and final comma separator.";
    const {things, ?nothingstr = "nothing", ?andstr = " and ",
      ?commastr = ", ", ?finalcommastr = ","} = args;
    const nthings = length(things);
    nthings == 0 && return nothingstr;
    nthings == 1 && return tostr(things[1]);
    nthings == 2 && return tostr(things[1], andstr, things[2]);
    let ret = "";
    for k in [1..nthings - 1]
      const separator = k == nthings - 1 ? finalcommastr | commastr;
      ret = tostr(ret, things[k], separator);
    endfor
    return tostr(ret, andstr, things[nthings]);
  endmethod

  method map owner: ARCH_WIZARD
    "Return a new list containing func(item) for each item in the input list.";
    "Example: {1, 2, 3}:map({x} => x * 2) => {2, 4, 6}.";
    set_task_perms(caller_perms());
    const {lst, func} = args;
    let result = {};
    for item in (lst)
      result = {@result, func(item)};
    endfor
    return result;
  endmethod

  method filter owner: ARCH_WIZARD
    "Return a new list containing only items for which pred(item) is true.";
    "Example: {1, 2, 3, 4, 5}:filter({x} => x % 2 == 0) => {2, 4}.";
    set_task_perms(caller_perms());
    const {lst, pred} = args;
    let result = {};
    for item in (lst)
      if (pred(item))
        result = {@result, item};
      endif
    endfor
    return result;
  endmethod

  method reduce owner: ARCH_WIZARD
    "Fold the list from left to right by repeatedly calling func(accumulator, item).";
    "Returns initial when the input list is empty.";
    set_task_perms(caller_perms());
    const {lst, func, initial} = args;
    let accumulator = initial;
    for item in (lst)
      accumulator = func(accumulator, item);
    endfor
    return accumulator;
  endmethod

  method find owner: ARCH_WIZARD
    "Return the first item for which pred(item) is true, or 0 when no item matches.";
    "Use find_index() to distinguish a matching zero from no match.";
    set_task_perms(caller_perms());
    const {lst, pred} = args;
    for item in (lst)
      if (pred(item))
        return item;
      endif
    endfor
    return 0;
  endmethod

  method find_index owner: ARCH_WIZARD
    "Return the 1-based index of the first matching item, or 0 when none match.";
    "Call pred(item) in order with the caller's permissions. Stop at the first match.";
    set_task_perms(caller_perms());
    const {items, pred} = args;
    for i in [1..length(items)]
      pred(items[i]) && return i;
    endfor
    return 0;
  endmethod

  method any owner: ARCH_WIZARD
    "Return true if pred(item) is true for at least one item in the list.";
    set_task_perms(caller_perms());
    const {lst, pred} = args;
    for item in (lst)
      if (pred(item))
        return true;
      endif
    endfor
    return false;
  endmethod

  method all owner: ARCH_WIZARD
    "Return true if pred(item) is true for every item in the list.";
    "The empty list returns true.";
    set_task_perms(caller_perms());
    const {lst, pred} = args;
    for item in (lst)
      if (!(pred(item)))
        return false;
      endif
    endfor
    return true;
  endmethod

  method unique owner: HACKER
    "Return a list with duplicate elements removed while preserving first-seen order.";
    "Example: {1, 2, 2, 3, 1, 4}:unique() => {1, 2, 3, 4}.";
    const {lst} = args;
    let result = {};
    for item in (lst)
      result = setadd(result, item);
    endfor
    return result;
  endmethod

  method group_by owner: ARCH_WIZARD
    "Return a map from key_func(item) to the list of items with that key.";
    "Each group preserves the original item order. Keys must be valid map keys.";
    set_task_perms(caller_perms());
    const {lst, key_func} = args;
    let groups = [];
    for item in (lst)
      const key = key_func(item);
      if (maphaskey(groups, key))
        groups[key] = {@groups[key], item};
      else
        groups[key] = {item};
      endif
    endfor
    return groups;
  endmethod

  method compose owner: HACKER
    "Compose a list of fragments, joining text and preserving HTML structure.";
    const {lst, render_for, content_type, ?event = false} = args;
    const {parts, annotations} = $format:compose_parts(lst, render_for, content_type, event);
    return $format:result(content_type == 'text_html ? parts | parts:join(""), annotations);
  endmethod

  method test_core_list_helpers owner: HACKER
    "Cover append, assoc, assoc_prefix, check_type, join, and english_list.";
    $test_utils:assert_eq(this:append({"a", "b"}, {"c"}, {}, {"d", "e"}),
      {"a", "b", "c", "d", "e"}, "append concatenates argument lists");
    const nested = {{"id", 1}, {"name", "cowbell"}, "skip", {"name", "moor"}};
    $test_utils:assert_eq(this:assoc(nested, "name"), {"name", "cowbell"},
      "assoc finds first matching nested list");
    $test_utils:assert_eq(this:assoc(nested, "missing"), {},
      "assoc returns empty list when missing");
    $test_utils:assert_eq(this:assoc(nested, "cowbell", 2), {"name", "cowbell"},
      "assoc supports custom index");
    $test_utils:assert_eq(this:assoc_prefix({{"alpha", 1}, {"beta", 2}}, "al"), {"alpha", 1},
      "assoc_prefix finds string prefix");
    $test_utils:assert_eq(this:assoc_prefix({{"alpha", 1}, {42, 2}}, "4"), {},
      "assoc_prefix ignores non-string indexed values");
    $test_utils:assert_true({"a", "b"}:check_type(TYPE_STR), "check_type accepts a single type");
    $test_utils:assert_true({"a", 1}:check_type({TYPE_STR, TYPE_INT}),
      "check_type accepts multiple types");
    $test_utils:assert_false({"a", 1}:check_type(TYPE_STR), "check_type rejects unmatched types");
    $test_utils:assert_eq({}:join(), "", "join empty list");
    $test_utils:assert_eq({"a", "b", "c"}:join(", "), "a, b, c", "join custom separator");
    $test_utils:assert_eq({1, 2, 3}:join("-"), "1-2-3", "join coerces non-string items");
    $test_utils:assert_eq({}:english_list(), "nothing", "english_list empty list");
    $test_utils:assert_eq({"red"}:english_list(), "red", "english_list single item");
    $test_utils:assert_eq({"red", "blue"}:english_list(), "red and blue", "english_list two items");
    $test_utils:assert_eq({"red", "blue", "green"}:english_list(), "red, blue, and green",
      "english_list three items");
    $test_utils:assert_eq({"red", "blue", "green"}:english_list("nothing", " or ", "; ", ";"),
      "red; blue; or green", "english_list custom separators");
    return true;
  endmethod

  method test_functional_list_helpers owner: HACKER
    "Cover map, filter, reduce, find, any, and all.";
    $test_utils:assert_eq({1, 2, 3}:map({x} => x * 2), {2, 4, 6}, "map transforms every item");
    $test_utils:assert_eq({}:map({x} => x + 1), {}, "map preserves empty lists");
    $test_utils:assert_eq({"a", "b", "c"}:map({x} => x + "X"), {"aX", "bX", "cX"},
      "map handles strings");
    $test_utils:assert_eq({1, 2, 3, 4, 5}:filter({x} => x % 2 == 0), {2, 4},
      "filter keeps matching items");
    $test_utils:assert_eq({1, 3, 5}:filter({x} => x % 2 == 0), {}, "filter can return empty list");
    $test_utils:assert_eq({1, 2, 3, 4}:reduce({acc, x} => acc + x, 0), 10, "reduce sums values");
    $test_utils:assert_eq({}:reduce({acc, x} => acc + x, 42), 42,
      "reduce returns initial for empty lists");
    $test_utils:assert_eq({1, 2, 3, 4, 5}:find({x} => x > 3), 4, "find returns first match");
    $test_utils:assert_eq({1, 2, 3}:find({x} => x > 10), 0, "find returns 0 when missing");
    $test_utils:assert_eq({0, false, "", #-1}:find_index({x} => x == 0), 1, "Find zero");
    $test_utils:assert_eq({0, false, "", #-1}:find_index({x} => typeof(x) == TYPE_OBJ), 4,
      "Find a false-valued object reference");
    $test_utils:assert_eq({1}:find_index({x} => x == 0), 0, "Missing index");
    $test_utils:assert_true({1, 2, 3}:any({x} => x > 2), "any true case");
    $test_utils:assert_false({1, 2, 3}:any({x} => x > 5), "any false case");
    $test_utils:assert_true({2, 4, 6}:all({x} => x % 2 == 0), "all true case");
    $test_utils:assert_false({1, 2, 3}:all({x} => x % 2 == 0), "all false case");
    $test_utils:assert_true({}:all({x} => x > 100), "all empty list is true");
    const expected = task_perms()[1];
    $test_utils:assert_false(expected.wizard, "Permission checks need a non-wizard caller");
    {1}:map({x} => caller_perms()) == {expected}
      || raise(E_ASSERT, "map callback should run with caller perms");
    {1}:filter({x} => caller_perms() == expected) == {1}
      || raise(E_ASSERT, "filter callback should run with caller perms");
    {1}:reduce({acc, x} => caller_perms(), 0) == expected
      || raise(E_ASSERT, "reduce callback should run with caller perms");
    {1}:find({x} => caller_perms() == expected) == 1
      || raise(E_ASSERT, "find callback should run with caller perms");
    {1}:find_index({x} => caller_perms() == expected) == 1
      || raise(E_ASSERT, "find_index callback should run with caller perms");
    {1}:any({x} => caller_perms() == expected)
      || raise(E_ASSERT, "any callback should run with caller perms");
    {1}:all({x} => caller_perms() == expected)
      || raise(E_ASSERT, "all callback should run with caller perms");
    {"a"}:group_by({x} => caller_perms()) == [expected -> {"a"}]
      || raise(E_ASSERT, "group_by callback should run with caller perms");
    return true;
  endmethod

  method test_set_group_and_compose_helpers owner: HACKER
    "Cover unique, group_by, compress, and compose.";
    $test_utils:assert_eq({1, 2, 2, 3, 1, 4}:unique(), {1, 2, 3, 4},
      "unique preserves first-seen order");
    $test_utils:assert_eq({}:unique(), {}, "unique preserves empty list");
    const nested = {{1}, ['a -> 2], {1}, ['a -> 2], <$root>, <$root>, "A", "a"};
    const expected = {{1}, ['a -> 2], <$root>, "A"};
    $test_utils:assert_eq(strcmp(toliteral(nested:unique()), toliteral(expected)), 0,
      "Deduplicate nested values and preserve the first spelling");
    $test_utils:assert_eq({"a", "b", "b", "c", "b", "b", "d"}:compress(),
      {"a", "b", "c", "b", "d"}, "compress collapses consecutive duplicates");
    $test_utils:assert_eq({}:compress(), {}, "compress preserves empty list");
    let result = {"apple", "ant", "bee", "bear"}:group_by({x} => x[1]);
    $test_utils:assert_eq(result, ["a" -> {"apple", "ant"}, "b" -> {"bee", "bear"}],
      "group_by groups by key function");
    result = {"a", "bb", "c", "dd", "eee"}:group_by({x} => length(x));
    $test_utils:assert_eq(result, [1 -> {"a", "c"}, 2 -> {"bb", "dd"}, 3 -> {"eee"}],
      "group_by preserves item order within groups");
    $test_utils:assert_eq({}:group_by({x} => x), [], "group_by empty list");
    $test_utils:assert_eq({"a", "b"}:compose(player, 'text), "ab", "compose joins text results");
    $test_utils:assert_eq({"a", "b"}:compose(player, 'text_html), {"a", "b"},
      "compose preserves HTML result list");
    return true;
  endmethod

  method test_callback_control_flow owner: HACKER
    "Callbacks stop at the promised boundary and propagate errors.";
    const fail = {item} => raise(E_INVARG, "callback failure");
    for operation in ({"map", "filter", "find", "find_index", "any", "all", "group_by"})
      $test_utils:assert_raises(E_INVARG, {1}, operation, {fail}, "Propagate callback errors");
    endfor
    $test_utils:assert_raises(E_INVARG, {1}, "reduce",
      {{acc, item} => raise(E_INVARG, "callback failure"), 0}, "Propagate fold errors");
    $test_utils:assert_eq({}:map(fail), {}, "Empty map skips callback");
    $test_utils:assert_eq({}:filter(fail), {}, "Empty filter skips callback");
    $test_utils:assert_eq({}:group_by(fail), [], "Empty grouping skips callback");
    $test_utils:assert_eq({}:find(fail), 0, "Empty find skips callback");
    $test_utils:assert_eq({}:find_index(fail), 0, "Empty index skips callback");
    $test_utils:assert_false({}:any(fail), "Empty any skips callback");
    $test_utils:assert_true({}:all(fail), "Empty all skips callback");
    const first = {item} => item == 1 ? true | fail(item);
    $test_utils:assert_eq({1, 2}:find(first), 1, "Find stops at first match");
    $test_utils:assert_eq({1, 2}:find_index(first), 1, "Index stops at first match");
    $test_utils:assert_true({1, 2}:any(first), "Any stops at first match");
    $test_utils:assert_false({1, 2}:all({item} => item == 1 ? false | fail(item)),
      "All stops at first rejection");
    $test_utils:assert_eq({1, 2, 3}:reduce({acc, item} => acc * 10 + item, 0), 123,
      "Fold preserves argument and iteration order");
    return true;
  endmethod
endobject
