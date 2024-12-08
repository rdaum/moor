// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use bincode::{Decode, Encode};
use lazy_static::lazy_static;
use moor_values::Symbol;
use moor_values::VarType;
use moor_values::VarType::{TYPE_FLYWEIGHT, TYPE_MAP};
/// Global registry of built-in function names.
use std::collections::HashMap;
use ArgCount::{Q, U};
use ArgType::{Any, AnyNum, Typed};
use VarType::{TYPE_FLOAT, TYPE_INT, TYPE_LIST, TYPE_OBJ, TYPE_STR};

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
fn mk_builtin_table() -> Vec<Builtin> {
    vec![
        Builtin {
            name: Symbol::mk("disassemble"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("log_cache_stats"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("verb_cache_stats"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("call_function"),
            min_args: Q(1),
            max_args: U,
            types: vec![Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("raise"),
            min_args: Q(1),
            max_args: Q(3),
            types: vec![Any, Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("suspend"),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("read"),
            min_args: Q(0),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("seconds_left"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("ticks_left"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("pass"),
            min_args: Q(0),
            max_args: U,
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("set_task_perms"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("caller_perms"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("callers"),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("task_stack"),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_INT), Any],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("function_info"),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_STR)],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("load_server_options"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("value_bytes"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("value_hash"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("string_hash"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_STR)],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("binary_hash"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_STR)],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("decode_binary"),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_STR), Any],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("encode_binary"),
            min_args: Q(0),
            max_args: U,
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("length"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("setadd"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_LIST), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("setremove"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_LIST), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("listappend"),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_LIST), Any, Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("listinsert"),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_LIST), Any, Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("listdelete"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_LIST), Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("listset"),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_LIST), Any, Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("equal"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Any, Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("is_member"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Any, Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("tostr"),
            min_args: Q(0),
            max_args: U,
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("toliteral"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("match"),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("rmatch"),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("substitute"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_STR), Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("crypt"),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("index"),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("rindex"),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("strcmp"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("strsub"),
            min_args: Q(3),
            max_args: Q(4),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR), Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("server_log"),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("toint"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("tonum"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("tofloat"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("min"),
            min_args: Q(1),
            max_args: U,
            types: vec![AnyNum],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("max"),
            min_args: Q(1),
            max_args: U,
            types: vec![AnyNum],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("abs"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![AnyNum],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("random"),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("time"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("ctime"),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("floatstr"),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_FLOAT), Typed(TYPE_INT), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("sqrt"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("sin"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("cos"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("tan"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("asin"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("acos"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("atan"),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_FLOAT), Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("sinh"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("cosh"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("tanh"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("exp"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("log"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("log10"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("ceil"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("floor"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("trunc"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("toobj"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("typeof"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("create"),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("recycle"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("object_bytes"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("valid"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("parent"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("children"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("chparent"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("max_object"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("players"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("is_player"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("set_player_flag"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("move"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("properties"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("property_info"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("set_property_info"),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("add_property"),
            min_args: Q(4),
            max_args: Q(4),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any, Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("delete_property"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("clear_property"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("is_clear_property"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("server_version"),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("renumber"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("reset_max_object"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("memory_usage"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("shutdown"),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("dump_database"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("db_disk_size"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("open_network_connection"),
            min_args: Q(0),
            max_args: U,
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("connected_players"),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("connected_seconds"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("idle_seconds"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("connection_name"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("notify"),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("boot_player"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("set_connection_option"),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("connection_option"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("connection_options"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("listen"),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Any, Any],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("unlisten"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("listeners"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("buffered_output_length"),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("task_id"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("queued_tasks"),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("kill_task"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("output_delimiters"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("queue_info"),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("resume"),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_INT), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("force_input"),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("flush_input"),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: false,
        },
        Builtin {
            name: Symbol::mk("verbs"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("verb_info"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("set_verb_info"),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Any, Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("verb_args"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("set_verb_args"),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Any, Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("add_verb"),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_LIST), Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("delete_verb"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("verb_code"),
            min_args: Q(2),
            max_args: Q(4),
            types: vec![Typed(TYPE_OBJ), Any, Any, Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("set_verb_code"),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Any, Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("eval"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("mapkeys"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_MAP)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("mapvalues"),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_MAP)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("mapdelete"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_MAP), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("maphaskey"),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_MAP), Any],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("xml_parse"),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_STR), Typed(TYPE_MAP)],
            implemented: true,
        },
        Builtin {
            name: Symbol::mk("to_xml"),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_FLYWEIGHT), Typed(TYPE_MAP)],
            implemented: true,
        },
    ]
}

#[derive(Clone, Copy, Debug, PartialOrd, PartialEq, Eq, Hash, Encode, Decode)]
pub struct BuiltinId(pub u16);

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

    pub fn len(&self) -> usize {
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
        .unwrap_or_else(|| panic!("Unknown builtin: {}", bf_name));
    builtin.0 as usize
}
