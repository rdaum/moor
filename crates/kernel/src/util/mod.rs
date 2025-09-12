// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::task_context::{has_current_transaction, with_current_transaction};
use moor_compiler::{to_literal, to_literal_objsub};
use moor_var::{Obj, Var};
use std::collections::HashMap;
use tracing::error;

/// If there's an active task & world state, this will seek to find sysobj style names for objects.
/// If no transaction is active, falls back to regular to_literal
pub fn ws_to_literal(v: &Var) -> String {
    if !has_current_transaction() {
        return to_literal(v);
    }
    let Some(names) =
        with_current_transaction(|ws| ws.sysobj_name_cache(&Obj::mk_id(0)).ok().clone())
    else {
        return to_literal(v);
    };
    // Build object -> name mapping, preferring single-name matches
    let unambiguous_matches = names
        .into_iter()
        .filter_map(|(obj, symbols)| {
            if symbols.len() == 1 {
                // Unambiguous: exactly one name for this object
                Some((obj, format!("${}", symbols[0].as_arc_string())))
            } else {
                // Ambiguous: multiple names, pick the first for now
                // TODO: Could implement preference logic here
                symbols
                    .first()
                    .map(|symbol| (obj, format!("${}", symbol.as_arc_string())))
            }
        })
        .collect::<HashMap<_, _>>();
    to_literal_objsub(v, &unambiguous_matches, 0)
}

pub fn ws_obj_print(o: &Obj) -> String {
    if !has_current_transaction() {
        error!("No transaction for ws_to_literal");
        return format!("{o}");
    }
    let Some(names) =
        with_current_transaction(|ws| ws.sysobj_name_cache(&Obj::mk_id(0)).ok().clone())
    else {
        return format!("{o}");
    };
    let Some(symbols) = names.get(o) else {
        return format!("{o}");
    };

    // Pick the first symbol for this object
    match symbols.first() {
        Some(symbol) => format!("${}", symbol.as_arc_string()),
        None => format!("{o}"),
    }
}
