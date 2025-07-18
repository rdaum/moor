# Improving MOOT Test Framework for Less Flaky Error Testing

## Current Issues

MOOT tests expect exact error message matches, which makes them fragile when:
1. Different parsers generate slightly different error messages
2. Error positions might be calculated differently
3. Error text formatting varies between implementations

Example from `looping.moot`:
```
; return eval("x = {}; for in ({1, 2, 3}); endfor; return x;");
{0, "Failure to parse program @ 1/13: expected ident"}
```

This expects the EXACT string, including the specific position and message.

## Proposed Improvements

### 1. Pattern-Based Error Matching

Add a new syntax for pattern-based matching:
```
; return eval("x = {}; for in ({1, 2, 3}); endfor; return x;");
~{0, "Failure to parse program @ */: expected ident"}
```

The `~` prefix indicates pattern matching, where:
- `*` matches any line/column position
- Regular expressions could be supported for message text

### 2. Error Code Matching

For many tests, we only care that a parse error occurred, not the exact message:
```
; return eval("x = {}; for in ({1, 2, 3}); endfor; return x;");
{0, *}  // Any parse error
```

### 3. Separate Parse Error Tests

Create a dedicated syntax for testing parse errors:
```
;! x = {}; for in ({1, 2, 3}); endfor; return x;
PARSE_ERROR
```

This would test that the code fails to parse, without checking the exact error message.

### 4. Parser-Specific Expected Results

Allow different expected results for different parsers:
```
; return eval("x = {}; for in ({1, 2, 3}); endfor; return x;");
@cst: {0, "Failure to parse program @ 1/13: expected ident"}
@tree-sitter: {0, "Failure to parse program @ 1/13: expected identifier"}
@default: {0, *}
```

### 5. Structured Error Matching

Instead of string matching, match on error structure:
```
; return eval("x = {}; for in ({1, 2, 3}); endfor; return x;");
{
  success: 0,
  error_type: "parse",
  position: {line: 1, column: 13},
  message_contains: "expected ident"
}
```

## Implementation Suggestions

### Quick Fix: Relaxed String Matching

As a quick fix, modify the MOOT test runner to support partial matches:
1. If expected output starts with `{0, "`, treat it as a parse error test
2. Extract the key parts (error position, key words) and match flexibly
3. Allow minor variations in wording ("ident" vs "identifier")

### Better Fix: New Test Syntax

Add new test syntax that's explicitly for error testing:
```
;; Test parse errors
; parse_error("x = {}; for in ({1, 2, 3}); endfor; return x;")
@position: 1/13
@message_contains: "expected ident"
```

### Best Fix: Semantic Error Matching

Match on the semantic meaning of errors rather than exact text:
- Parse errors at specific positions
- Missing identifier errors
- Type errors
- Runtime errors

This would make tests resilient to changes in error message wording while still ensuring the correct errors are detected.

## Example Updated Test

Original:
```
; return eval("x = {}; for in ({1, 2, 3}); endfor; return x;");
{0, "Failure to parse program @ 1/13: expected ident"}
```

Improved (multiple options):
```
;; Option 1: Pattern matching
; return eval("x = {}; for in ({1, 2, 3}); endfor; return x;");
~{0, "Failure to parse program @ 1/*: expected ident*"}

;; Option 2: Error code only
; return eval("x = {}; for in ({1, 2, 3}); endfor; return x;");
{0, _}  // Any error message

;; Option 3: Structured matching
; return eval("x = {}; for in ({1, 2, 3}); endfor; return x;");
ERROR(parse, position: 1/*, contains: "ident")
```

These improvements would make MOOT tests more maintainable and less sensitive to minor changes in error formatting while still ensuring errors are properly detected.