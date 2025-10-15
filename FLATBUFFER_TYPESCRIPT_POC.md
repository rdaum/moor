# FlatBuffer TypeScript Proof of Concept

## Overview

This document describes our exploration of using FlatBuffers directly from TypeScript, eliminating
the JSON serialization layer between Rust and TypeScript.

## What We Built

### 1. Generated TypeScript from FlatBuffer Schemas

```bash
flatc --ts --gen-all -o web-client/src/generated \
  crates/schema/schema/moor_rpc.fbs \
  crates/schema/schema/var.fbs \
  crates/schema/schema/common.fbs
```

This generates ~200+ TypeScript files providing type-safe access to all RPC structures.

### 2. Ergonomic Wrapper: `MoorVar` Class

**File**: `web-client/src/lib/MoorVar.ts`

A wrapper class that mimics the Rust `moor_var::Var` API, providing:

- `.asInteger()`, `.asFloat()`, `.asString()`, `.asBool()`
- `.asObject()`, `.asSymbol()`, `.asList()`, `.asMap()`, `.asError()`
- `.toJS()` - Convert any Var to plain JavaScript values
- `.typeCode()` - Get the variant type

This abstracts away the complexity of navigating FlatBuffer unions.

### 3. RPC Client Functions

**File**: `web-client/src/lib/rpc-fb.ts`

Implemented two proof-of-concept functions:

- `performEvalFlatBuffer(authToken, expr)` - Evaluate MOO expressions
- `getSystemPropertyFlatBuffer(objectPath, propertyName)` - Fetch system properties

Both parse FlatBuffer responses directly and use `MoorVar` for clean value extraction.

## Advantages

### ✅ End-to-End Type Safety

TypeScript knows the exact structure of every RPC message. If the schema changes:

- TypeScript compiler immediately shows errors
- No runtime surprises
- Refactoring is safer

### ✅ Less Manual Code

**Before** (JSON approach):

```rust
// Rust: Manually construct JSON
Json(json!({
    "location": location.as_u64(),
    "owner": owner.as_u64(),
    "names": names,
    "code": code,
    // ... many more fields
}))
```

**After** (FlatBuffer approach):

```rust
// Rust: Just return the bytes
Response::builder()
    .header("Content-Type", "application/x-flatbuffer")
    .body(Body::from(reply_bytes))
    .unwrap()
```

### ✅ Binary Efficiency

- FlatBuffers are zero-copy
- Smaller payloads than JSON
- Faster parsing (no string→object conversion)

### ✅ Single Source of Truth

The `.fbs` schema files define the contract. Both Rust and TypeScript generate from the same source.

## Challenges

### ⚠️ Nested Unions Require Ceremony

Navigating deeply nested unions requires helper functions:

```typescript
const replyType = daemonReply.replyType();
const replyUnion = unionToDaemonToClientReplyUnion(
    replyType,
    (obj: any) => daemonReply.reply(obj),
);
```

**Solution**: The `MoorVar` wrapper class eliminates this for the most common case (Var values).

### ⚠️ Debugging

Binary data is harder to inspect than JSON in browser dev tools.

**Mitigation**:

- Could add dev mode that logs parsed structures
- Use `.toJS()` to convert to inspectable objects

### ⚠️ BigInt Handling

FlatBuffers uses `bigint` for 64-bit integers, which needs conversion:

```typescript
const val = varInt?.value();
return val !== null ? Number(val) : null;
```

## Next Steps

### Option 1: Hybrid Approach

- Use FlatBuffers for high-frequency endpoints (`/system_property_fb/*`, etc.)
- Keep JSON for admin/debug endpoints
- Migrate incrementally

### Option 2: Full Migration

1. Create FlatBuffer version of all REST endpoints (`*_fb`)
2. Update client to use FlatBuffer functions
3. Remove JSON endpoints once migration is complete

### Option 3: Generate More Helpers

Create a code generator that produces TypeScript helper functions for each RPC message type, similar
to how `rpc_common` provides helpers in Rust.

## Implementation Checklist

For the `system_property_handler` endpoint:

### Rust Side (web-host)

- [x] Add `/fb/system_property/{*path}` route
- [x] Handler just returns `reply_bytes` directly with `application/x-flatbuffer` content-type
- [x] No JSON serialization needed

### TypeScript Side

- [x] Update `retrieveWelcome()` in `rpc.ts` to use `getSystemPropertyFlatBuffer()`
- [x] Handles both string and array responses correctly
- [ ] Test with actual welcome message retrieval (requires running server)
- [ ] Verify content types work correctly

### Build Process

- [ ] Add `flatc` TypeScript generation to build scripts
- [ ] Ensure generated files are gitignored or committed (TBD)
- [ ] Document schema change workflow

## Files Created

1. `web-client/src/generated/` - All generated TypeScript (~200 files)
2. `web-client/src/lib/MoorVar.ts` - Ergonomic Var wrapper
3. `web-client/src/lib/rpc-fb.ts` - FlatBuffer RPC client functions

## Performance Expectations

While we haven't benchmarked yet, theoretical improvements:

1. **Network**: 20-40% smaller payloads (binary vs JSON)
2. **Parsing**: 2-5x faster (zero-copy vs JSON.parse)
3. **Type Safety**: Compile-time errors instead of runtime crashes

## Conclusion

The FlatBuffer TypeScript approach is **viable and promising**. The main benefits are:

1. Type safety end-to-end
2. Significant reduction in manual serialization code
3. Better performance characteristics

The `MoorVar` wrapper class makes the TypeScript API ergonomic and familiar to anyone who knows the
Rust API.

**Recommendation**: Start with `system_property_handler` as a proof-of-concept, measure the
benefits, then decide on migration strategy.
