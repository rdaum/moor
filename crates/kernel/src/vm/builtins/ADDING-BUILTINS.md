<!--
Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
software: you can redistribute it and/or modify it under the terms of the GNU
General Public License as published by the Free Software Foundation, version
3.

This program is distributed in the hope that it will be useful, but WITHOUT
ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with
this program. If not, see <https://www.gnu.org/licenses/>.
-->

# Adding Builtins Safely

Builtin IDs are allocated in fixed-size groups to avoid renumbering existing IDs. Each group maps to
a `bf_*.rs` module and is padded with reserved entries.

## Why Stable IDs Matter

Compiled programs store builtin IDs directly. Those IDs are persisted in the database and replayed
when a world is loaded. Reordering or removing entries changes IDs and invalidates existing compiled
bytecode, which forces a full recompile/import. The grouping scheme keeps IDs stable while allowing
growth.

## Steps

1. Pick the right group.
   - Group order and padding are defined in `crates/common/src/builtins.rs`.
   - Each group has 256 slots. Do not exceed the group size.

2. Add the builtin in its group block.
   - Insert a `mk_builtin(...)` entry in the appropriate group in `crates/common/src/builtins.rs`.
   - Do not add `mk_reserved_builtin()` entries manually.

3. Implement and register the builtin.
   - Add the implementation to the matching `crates/kernel/src/vm/builtins/bf_*.rs`.
   - Register it in that module's `register_bf_*` using `offset_for_builtin`.

4. Add or update docs.
   - Builtin docs are generated from Rustdoc in the `bf_*.rs` modules.
   - Ensure the new builtin has appropriate doc comments.

## Notes

- Reserved entries are hidden from name lookup and `function_info()`, but still occupy IDs. This is
  what keeps group IDs stable.
- If a group gets close to 256 entries, we should decide how to split it before adding new builtins.
