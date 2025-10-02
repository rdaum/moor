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

use ArgCount::{Q, U};
use ArgType::{Any, AnyNum, Typed};
use VarType::{TYPE_FLOAT, TYPE_INT, TYPE_LIST, TYPE_OBJ, TYPE_STR};
use lazy_static::lazy_static;
use moor_var::{
    Symbol, VarType,
    VarType::{TYPE_BOOL, TYPE_ERR, TYPE_FLYWEIGHT, TYPE_MAP, TYPE_SYMBOL},
};
/// Global registry of built-in function names.
use std::collections::HashMap;

lazy_static! {
    static ref BUILTIN_DESCRIPTORS: Vec<Builtin> = mk_builtin_table();
    pub static ref BUILTINS: Builtins = Builtins::new();
}
pub enum ArgCount {
    Q(usize),
    U,
}

pub enum ArgType {
    Typed(VarType),
    Any,
    AnyNum,
}

pub struct Builtin {
    pub name: Symbol,
    pub bf_override_name: Symbol,
    pub min_args: ArgCount,
    pub max_args: ArgCount,
    pub types: Vec<ArgType>,
    pub implemented: bool,
}

// Originally generated using ./generate_bf_list.py
// TODO: this list is inconsistently used, and falls out of date. It's only used for generating
//  the list of functions for the `function_info` built-in right now. It could be used for
//  validating arguments, and could be part of the registration process for the actual builtin
//  implementations.
// NOTE: only add new functions to the end of this table or you will throw off function indexes on
//  existing (binary) databases, causing severe incompatibility.

// Helper function to create a Builtin with automatic bf_override_name generation
fn mk_builtin(
    name: &str,
    min_args: ArgCount,
    max_args: ArgCount,
    types: Vec<ArgType>,
    implemented: bool,
) -> Builtin {
    Builtin {
        name: Symbol::mk(name),
        bf_override_name: Symbol::mk(&format!("bf_{name}")),
        min_args,
        max_args,
        types,
        implemented,
    }
}

fn mk_builtin_table() -> Vec<Builtin> {
    vec![
        mk_builtin("disassemble", Q(2), Q(2), vec![Typed(TYPE_OBJ), Any], true),
        mk_builtin("log_cache_stats", Q(0), Q(0), vec![], true),
        mk_builtin("verb_cache_stats", Q(0), Q(0), vec![], true),
        mk_builtin("property_cache_stats", Q(0), Q(0), vec![], true),
        mk_builtin("ancestry_cache_stats", Q(0), Q(0), vec![], true),
        mk_builtin("call_function", Q(1), U, vec![Typed(TYPE_STR)], true),
        mk_builtin("raise", Q(1), Q(3), vec![Any, Typed(TYPE_STR), Any], true),
        mk_builtin("suspend", Q(0), Q(1), vec![Typed(TYPE_INT)], true),
        mk_builtin("read", Q(0), Q(2), vec![Typed(TYPE_OBJ), Any], true),
        mk_builtin("seconds_left", Q(0), Q(0), vec![], true),
        mk_builtin("ticks_left", Q(0), Q(0), vec![], true),
        mk_builtin("pass", Q(0), U, vec![], true),
        mk_builtin("set_task_perms", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("caller_perms", Q(0), Q(0), vec![], true),
        mk_builtin("callers", Q(0), Q(1), vec![Any], true),
        mk_builtin("task_stack", Q(1), Q(2), vec![Typed(TYPE_INT), Any], false),
        mk_builtin("function_info", Q(0), Q(1), vec![Typed(TYPE_STR)], true),
        mk_builtin("load_server_options", Q(0), Q(0), vec![], false),
        mk_builtin("value_bytes", Q(1), Q(1), vec![Any], true),
        mk_builtin("value_hash", Q(1), Q(1), vec![Any], true),
        mk_builtin("string_hash", Q(1), Q(1), vec![Typed(TYPE_STR)], true),
        mk_builtin("binary_hash", Q(1), Q(1), vec![Typed(TYPE_STR)], true),
        mk_builtin(
            "decode_binary",
            Q(1),
            Q(2),
            vec![Typed(TYPE_STR), Any],
            false,
        ),
        mk_builtin("encode_binary", Q(0), U, vec![], false),
        mk_builtin("length", Q(1), Q(1), vec![Any], true),
        mk_builtin("setadd", Q(2), Q(2), vec![Typed(TYPE_LIST), Any], true),
        mk_builtin("setremove", Q(2), Q(2), vec![Typed(TYPE_LIST), Any], true),
        mk_builtin(
            "listappend",
            Q(2),
            Q(3),
            vec![Typed(TYPE_LIST), Any, Typed(TYPE_INT)],
            true,
        ),
        mk_builtin(
            "listinsert",
            Q(2),
            Q(3),
            vec![Typed(TYPE_LIST), Any, Typed(TYPE_INT)],
            true,
        ),
        mk_builtin(
            "listdelete",
            Q(2),
            Q(2),
            vec![Typed(TYPE_LIST), Typed(TYPE_INT)],
            true,
        ),
        mk_builtin(
            "listset",
            Q(3),
            Q(3),
            vec![Typed(TYPE_LIST), Any, Typed(TYPE_INT)],
            true,
        ),
        mk_builtin(
            "locations",
            Q(1),
            Q(3),
            vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ), Typed(TYPE_INT)],
            true,
        ),
        mk_builtin("equal", Q(2), Q(2), vec![Any, Any], true),
        mk_builtin("is_member", Q(2), Q(2), vec![Any, Typed(TYPE_LIST)], true),
        mk_builtin("tostr", Q(0), U, vec![], true),
        mk_builtin("toliteral", Q(1), Q(1), vec![Any], true),
        mk_builtin(
            "match",
            Q(2),
            Q(3),
            vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            true,
        ),
        mk_builtin(
            "rmatch",
            Q(2),
            Q(3),
            vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            true,
        ),
        mk_builtin(
            "substitute",
            Q(2),
            Q(2),
            vec![Typed(TYPE_STR), Typed(TYPE_LIST)],
            true,
        ),
        mk_builtin(
            "pcre_match",
            Q(2),
            Q(4),
            vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            true,
        ),
        mk_builtin(
            "pcre_replace",
            Q(2),
            Q(4),
            vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            true,
        ),
        mk_builtin(
            "crypt",
            Q(1),
            Q(2),
            vec![Typed(TYPE_STR), Typed(TYPE_STR)],
            true,
        ),
        mk_builtin(
            "index",
            Q(2),
            Q(3),
            vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            true,
        ),
        mk_builtin(
            "rindex",
            Q(2),
            Q(3),
            vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            true,
        ),
        mk_builtin(
            "strcmp",
            Q(2),
            Q(2),
            vec![Typed(TYPE_STR), Typed(TYPE_STR)],
            true,
        ),
        mk_builtin(
            "strsub",
            Q(3),
            Q(4),
            vec![Typed(TYPE_STR), Typed(TYPE_STR), Typed(TYPE_STR), Any],
            true,
        ),
        mk_builtin("server_log", Q(1), Q(2), vec![Typed(TYPE_STR), Any], true),
        mk_builtin("toint", Q(1), Q(1), vec![Any], true),
        mk_builtin("tonum", Q(1), Q(1), vec![Any], true),
        mk_builtin("tofloat", Q(1), Q(1), vec![Any], true),
        mk_builtin("min", Q(1), U, vec![AnyNum], true),
        mk_builtin("max", Q(1), U, vec![AnyNum], true),
        mk_builtin("abs", Q(1), Q(1), vec![AnyNum], true),
        mk_builtin("random", Q(0), Q(1), vec![Typed(TYPE_INT)], true),
        mk_builtin("time", Q(0), Q(0), vec![], true),
        mk_builtin("ftime", Q(0), Q(1), vec![Typed(TYPE_INT)], true),
        mk_builtin("ctime", Q(0), Q(1), vec![Typed(TYPE_INT)], true),
        mk_builtin(
            "floatstr",
            Q(2),
            Q(3),
            vec![Typed(TYPE_FLOAT), Typed(TYPE_INT), Any],
            true,
        ),
        mk_builtin("sqrt", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("sin", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("cos", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("tan", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("asin", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("acos", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin(
            "atan",
            Q(1),
            Q(2),
            vec![Typed(TYPE_FLOAT), Typed(TYPE_FLOAT)],
            true,
        ),
        mk_builtin("sinh", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("cosh", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("tanh", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("exp", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("log", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("log10", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("ceil", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("floor", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("trunc", Q(1), Q(1), vec![Typed(TYPE_FLOAT)], true),
        mk_builtin("toobj", Q(1), Q(1), vec![Any], true),
        mk_builtin("typeof", Q(1), Q(1), vec![Any], true),
        mk_builtin(
            "create",
            Q(1),
            Q(2),
            vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ)],
            true,
        ),
        mk_builtin("recycle", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("object_bytes", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("valid", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("parent", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("children", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin(
            "chparent",
            Q(2),
            Q(2),
            vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ)],
            true,
        ),
        mk_builtin("max_object", Q(0), Q(0), vec![], true),
        mk_builtin("players", Q(0), Q(0), vec![], true),
        mk_builtin("is_player", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin(
            "set_player_flag",
            Q(2),
            Q(2),
            vec![Typed(TYPE_OBJ), Any],
            true,
        ),
        mk_builtin(
            "move",
            Q(2),
            Q(2),
            vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ)],
            true,
        ),
        mk_builtin("properties", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin(
            "property_info",
            Q(2),
            Q(2),
            vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            true,
        ),
        mk_builtin(
            "set_property_info",
            Q(3),
            Q(3),
            vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Typed(TYPE_LIST)],
            true,
        ),
        mk_builtin(
            "add_property",
            Q(4),
            Q(4),
            vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any, Typed(TYPE_LIST)],
            true,
        ),
        mk_builtin(
            "delete_property",
            Q(2),
            Q(2),
            vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            true,
        ),
        mk_builtin(
            "clear_property",
            Q(2),
            Q(2),
            vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            true,
        ),
        mk_builtin(
            "is_clear_property",
            Q(2),
            Q(2),
            vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            true,
        ),
        mk_builtin("server_version", Q(0), Q(1), vec![Any], true),
        mk_builtin(
            "renumber",
            Q(1),
            Q(2),
            vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ)],
            true,
        ),
        mk_builtin("reset_max_object", Q(0), Q(0), vec![], false),
        mk_builtin("memory_usage", Q(0), Q(0), vec![], true),
        mk_builtin("shutdown", Q(0), Q(1), vec![Typed(TYPE_STR)], true),
        mk_builtin("dump_database", Q(0), Q(0), vec![], true),
        mk_builtin("db_disk_size", Q(0), Q(0), vec![], false),
        mk_builtin("open_network_connection", Q(0), U, vec![], false),
        mk_builtin("connected_players", Q(0), Q(1), vec![Any], true),
        mk_builtin("connected_seconds", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("idle_seconds", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("connection_name", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin(
            "notify",
            Q(2),
            Q(3),
            vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any],
            true,
        ),
        mk_builtin("boot_player", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin(
            "set_connection_option",
            Q(3),
            Q(3),
            vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any],
            true,
        ),
        mk_builtin(
            "connection_option",
            Q(2),
            Q(2),
            vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            true,
        ),
        mk_builtin(
            "connection_options",
            Q(1),
            Q(1),
            vec![Typed(TYPE_OBJ)],
            true,
        ),
        mk_builtin("listen", Q(2), Q(3), vec![Typed(TYPE_OBJ), Any, Any], true),
        mk_builtin("unlisten", Q(1), Q(1), vec![Any], true),
        mk_builtin("listeners", Q(0), Q(0), vec![], true),
        mk_builtin(
            "buffered_output_length",
            Q(0),
            Q(1),
            vec![Typed(TYPE_OBJ)],
            false,
        ),
        mk_builtin("task_id", Q(0), Q(0), vec![], true),
        mk_builtin("queued_tasks", Q(0), Q(0), vec![], true),
        mk_builtin("kill_task", Q(1), Q(1), vec![Typed(TYPE_INT)], true),
        mk_builtin("output_delimiters", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("queue_info", Q(0), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("resume", Q(1), Q(2), vec![Typed(TYPE_INT), Any], true),
        mk_builtin(
            "force_input",
            Q(2),
            Q(3),
            vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any],
            true,
        ),
        mk_builtin("flush_input", Q(1), Q(2), vec![Typed(TYPE_OBJ), Any], false),
        mk_builtin("verbs", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("verb_info", Q(2), Q(2), vec![Typed(TYPE_OBJ), Any], true),
        mk_builtin(
            "set_verb_info",
            Q(3),
            Q(3),
            vec![Typed(TYPE_OBJ), Any, Typed(TYPE_LIST)],
            true,
        ),
        mk_builtin("verb_args", Q(2), Q(2), vec![Typed(TYPE_OBJ), Any], true),
        mk_builtin(
            "set_verb_args",
            Q(3),
            Q(3),
            vec![Typed(TYPE_OBJ), Any, Typed(TYPE_LIST)],
            true,
        ),
        mk_builtin(
            "add_verb",
            Q(3),
            Q(3),
            vec![Typed(TYPE_OBJ), Typed(TYPE_LIST), Typed(TYPE_LIST)],
            true,
        ),
        mk_builtin("delete_verb", Q(2), Q(2), vec![Typed(TYPE_OBJ), Any], true),
        mk_builtin(
            "verb_code",
            Q(2),
            Q(4),
            vec![Typed(TYPE_OBJ), Any, Any, Any],
            true,
        ),
        mk_builtin(
            "set_verb_code",
            Q(3),
            Q(3),
            vec![Typed(TYPE_OBJ), Any, Typed(TYPE_LIST)],
            true,
        ),
        mk_builtin("eval", Q(1), Q(1), vec![Typed(TYPE_STR)], true),
        mk_builtin("mapkeys", Q(1), Q(1), vec![Typed(TYPE_MAP)], true),
        mk_builtin("mapvalues", Q(1), Q(1), vec![Typed(TYPE_MAP)], true),
        mk_builtin("mapdelete", Q(2), Q(2), vec![Typed(TYPE_MAP), Any], true),
        mk_builtin("maphaskey", Q(2), Q(2), vec![Typed(TYPE_MAP), Any], true),
        mk_builtin(
            "xml_parse",
            Q(2),
            Q(3),
            vec![Typed(TYPE_STR), Typed(TYPE_INT), Typed(TYPE_MAP)],
            true,
        ),
        mk_builtin("to_xml", Q(1), Q(2), vec![Any, Typed(TYPE_MAP)], true),
        mk_builtin(
            "present",
            Q(2),
            Q(6),
            vec![
                Typed(TYPE_OBJ),
                Typed(TYPE_STR),
                Typed(TYPE_STR),
                Typed(TYPE_STR),
                Typed(TYPE_STR),
                Any,
            ],
            true,
        ),
        mk_builtin(
            "argon2",
            Q(2),
            Q(5),
            vec![
                Typed(TYPE_STR),
                Typed(TYPE_STR),
                Typed(TYPE_INT),
                Typed(TYPE_INT),
                Typed(TYPE_INT),
            ],
            true,
        ),
        mk_builtin(
            "argon2_verify",
            Q(2),
            Q(2),
            vec![Typed(TYPE_STR), Typed(TYPE_STR)],
            true,
        ),
        mk_builtin("tosym", Q(1), Q(1), vec![Any], true),
        mk_builtin("salt", Q(0), Q(0), vec![], true),
        mk_builtin("slots", Q(1), Q(1), vec![Typed(TYPE_FLYWEIGHT)], true),
        mk_builtin(
            "remove_slot",
            Q(2),
            Q(2),
            vec![Typed(TYPE_FLYWEIGHT), Typed(TYPE_SYMBOL)],
            true,
        ),
        mk_builtin(
            "add_slot",
            Q(3),
            Q(3),
            vec![Typed(TYPE_FLYWEIGHT), Typed(TYPE_SYMBOL), Any],
            true,
        ),
        mk_builtin(
            "age_encrypt",
            Q(2),
            Q(2),
            vec![Typed(TYPE_STR), Typed(TYPE_STR)],
            true,
        ),
        mk_builtin(
            "age_decrypt",
            Q(2),
            Q(2),
            vec![Typed(TYPE_STR), Typed(TYPE_STR)],
            true,
        ),
        mk_builtin("age_generate_keypair", Q(0), Q(0), vec![], true),
        mk_builtin("encode_base64", Q(1), Q(1), vec![Typed(TYPE_STR)], true),
        mk_builtin("decode_base64", Q(1), Q(1), vec![Typed(TYPE_STR)], true),
        mk_builtin("slice", Q(1), Q(3), vec![Any, Any, Any], true),
        mk_builtin("generate_json", Q(1), Q(1), vec![Any], true),
        mk_builtin("parse_json", Q(1), Q(1), vec![Typed(TYPE_STR)], true),
        mk_builtin("ancestors", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("descendants", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin(
            "isa",
            Q(2),
            Q(2),
            vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ)],
            true,
        ),
        mk_builtin("bf_counters", Q(0), Q(0), vec![], true),
        mk_builtin("db_counters", Q(0), Q(0), vec![], true),
        mk_builtin("vm_counters", Q(0), Q(0), vec![], true),
        mk_builtin("sched_counters", Q(0), Q(0), vec![], true),
        mk_builtin("wait_task", Q(1), Q(1), vec![Typed(TYPE_INT)], true),
        mk_builtin("commit", Q(0), Q(0), vec![], true),
        mk_builtin("rollback", Q(0), Q(1), vec![Typed(TYPE_BOOL)], true),
        mk_builtin("respond_to", Q(2), Q(2), vec![Typed(TYPE_OBJ), Any], true),
        mk_builtin("active_tasks", Q(0), Q(0), vec![], true),
        mk_builtin(
            "worker_request",
            Q(2),
            U,
            vec![Typed(TYPE_SYMBOL), Any],
            true,
        ),
        mk_builtin("connections", Q(0), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("error_message", Q(1), Q(1), vec![Typed(TYPE_ERR)], true),
        mk_builtin("error_code", Q(1), Q(1), vec![Typed(TYPE_ERR)], true),
        mk_builtin(
            "string_hmac",
            Q(2),
            Q(4),
            vec![
                Typed(TYPE_STR),
                Typed(TYPE_STR),
                Typed(TYPE_STR),
                Typed(TYPE_BOOL),
            ],
            true,
        ),
        mk_builtin("owned_objects", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("switch_player", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin(
            "complex_match",
            Q(2),
            Q(4),
            vec![
                Typed(TYPE_STR),
                Typed(TYPE_LIST),
                Typed(TYPE_LIST),
                Typed(TYPE_INT),
            ],
            true,
        ),
        mk_builtin(
            "dump_object",
            Q(1),
            Q(2),
            vec![Typed(TYPE_OBJ), Typed(TYPE_MAP)],
            true,
        ),
        mk_builtin(
            "load_object",
            Q(1),
            Q(3),
            vec![Typed(TYPE_LIST), Typed(TYPE_OBJ), Typed(TYPE_MAP)],
            true,
        ),
        mk_builtin(
            "create_at",
            Q(2),
            Q(4),
            vec![
                Typed(TYPE_OBJ),
                Typed(TYPE_OBJ),
                Typed(TYPE_OBJ),
                Typed(TYPE_LIST),
            ],
            true,
        ),
        mk_builtin("workers", Q(0), Q(0), vec![], true),
        mk_builtin("gc_collect", Q(0), Q(0), vec![], true),
        mk_builtin("is_anonymous", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
        mk_builtin("is_uuobjid", Q(1), Q(1), vec![Typed(TYPE_OBJ)], true),
    ]
}

// BuiltinId is now defined in moor_var::program::opcode and re-exported
pub use moor_var::program::opcode::BuiltinId;

/// The dictionary of all builtins indexed by their name, and by their unique ID.
pub struct Builtins {
    pub offsets: HashMap<Symbol, BuiltinId>,
    pub names: HashMap<BuiltinId, Symbol>,
}

impl Default for Builtins {
    fn default() -> Self {
        Self::new()
    }
}

impl Builtins {
    pub fn new() -> Self {
        let offsets = make_builtin_offsets();
        let names = make_offsets_builtins();
        Self { offsets, names }
    }

    pub fn find_builtin(&self, bf_name: Symbol) -> Option<BuiltinId> {
        self.offsets.get(&bf_name).cloned()
    }

    pub fn name_of(&self, offset: BuiltinId) -> Option<Symbol> {
        self.names.get(&offset).cloned()
    }

    pub fn number_of(&self) -> usize {
        self.offsets.len()
    }

    pub fn description_for(&self, offset: BuiltinId) -> Option<&Builtin> {
        BUILTIN_DESCRIPTORS.get(offset.0 as usize)
    }

    pub fn descriptions(&self) -> impl Iterator<Item = &Builtin> {
        BUILTIN_DESCRIPTORS.iter()
    }
}

fn make_builtin_offsets() -> HashMap<Symbol, BuiltinId> {
    let mut b = HashMap::new();
    for (offset, builtin) in BUILTIN_DESCRIPTORS.iter().enumerate() {
        b.insert(builtin.name, BuiltinId(offset as u16));
    }

    b
}
pub fn make_offsets_builtins() -> HashMap<BuiltinId, Symbol> {
    let mut b = HashMap::new();
    for (offset, builtin) in BUILTIN_DESCRIPTORS.iter().enumerate() {
        b.insert(BuiltinId(offset as u16), builtin.name);
    }

    b
}

pub fn offset_for_builtin(bf_name: &str) -> usize {
    let bf_name = Symbol::mk(bf_name);
    let builtin = BUILTINS
        .find_builtin(bf_name)
        .unwrap_or_else(|| panic!("Unknown builtin: {bf_name}"));
    builtin.0 as usize
}
