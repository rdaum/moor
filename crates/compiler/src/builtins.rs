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

use lazy_static::lazy_static;
use moor_values::var::VarType;
/// Global registry of built-in function names.
use std::collections::HashMap;
use ArgCount::{Q, U};
use ArgType::{Any, AnyNum, Typed};
use VarType::{TYPE_FLOAT, TYPE_INT, TYPE_LIST, TYPE_OBJ, TYPE_STR};

use crate::labels::Name;

lazy_static! {
    pub static ref BUILTIN_DESCRIPTORS: Vec<Builtin> = mk_builtin_table();
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
    pub name: String,
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
            name: "disassemble".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: true,
        },
        Builtin {
            name: "log_cache_stats".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: "verb_cache_stats".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: "call_function".to_string(),
            min_args: Q(1),
            max_args: U,
            types: vec![Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: "raise".to_string(),
            min_args: Q(1),
            max_args: Q(3),
            types: vec![Any, Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: "suspend".to_string(),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: "read".to_string(),
            min_args: Q(0),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: false,
        },
        Builtin {
            name: "seconds_left".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: "ticks_left".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: "pass".to_string(),
            min_args: Q(0),
            max_args: U,
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: "set_task_perms".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "caller_perms".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: "callers".to_string(),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: "task_stack".to_string(),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_INT), Any],
            implemented: false,
        },
        Builtin {
            name: "function_info".to_string(),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_STR)],
            implemented: false,
        },
        Builtin {
            name: "load_server_options".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: "value_bytes".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: false,
        },
        Builtin {
            name: "value_hash".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: false,
        },
        Builtin {
            name: "string_hash".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_STR)],
            implemented: false,
        },
        Builtin {
            name: "binary_hash".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_STR)],
            implemented: false,
        },
        Builtin {
            name: "decode_binary".to_string(),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_STR), Any],
            implemented: false,
        },
        Builtin {
            name: "encode_binary".to_string(),
            min_args: Q(0),
            max_args: U,
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: "length".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: "setadd".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_LIST), Any],
            implemented: true,
        },
        Builtin {
            name: "setremove".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_LIST), Any],
            implemented: true,
        },
        Builtin {
            name: "listappend".to_string(),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_LIST), Any, Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: "listinsert".to_string(),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_LIST), Any, Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: "listdelete".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_LIST), Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: "listset".to_string(),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_LIST), Any, Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: "equal".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Any, Any],
            implemented: true,
        },
        Builtin {
            name: "is_member".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Any, Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: "tostr".to_string(),
            min_args: Q(0),
            max_args: U,
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: "toliteral".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: "match".to_string(),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: "rmatch".to_string(),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            implemented: false,
        },
        Builtin {
            name: "substitute".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_STR), Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: "crypt".to_string(),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: "index".to_string(),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: "rindex".to_string(),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: "strcmp".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: "strsub".to_string(),
            min_args: Q(3),
            max_args: Q(4),
            types: vec![Typed(TYPE_STR), Typed(TYPE_STR), Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: "server_log".to_string(),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: "toint".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: "tonum".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: "tofloat".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: "min".to_string(),
            min_args: Q(1),
            max_args: U,
            types: vec![AnyNum],
            implemented: true,
        },
        Builtin {
            name: "max".to_string(),
            min_args: Q(1),
            max_args: U,
            types: vec![AnyNum],
            implemented: true,
        },
        Builtin {
            name: "abs".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![AnyNum],
            implemented: true,
        },
        Builtin {
            name: "random".to_string(),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: "time".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: "ctime".to_string(),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: "floatstr".to_string(),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_FLOAT), Typed(TYPE_INT), Any],
            implemented: true,
        },
        Builtin {
            name: "sqrt".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "sin".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "cos".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "tan".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "asin".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "acos".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "atan".to_string(),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_FLOAT), Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "sinh".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "cosh".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "tanh".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "exp".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "log".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "log10".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "ceil".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "floor".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "trunc".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_FLOAT)],
            implemented: true,
        },
        Builtin {
            name: "toobj".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: "typeof".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: "create".to_string(),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "recycle".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "object_bytes".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: "valid".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "parent".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "children".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "chparent".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "max_object".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: "players".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: "is_player".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "set_player_flag".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: true,
        },
        Builtin {
            name: "move".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "properties".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "property_info".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: "set_property_info".to_string(),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: "add_property".to_string(),
            min_args: Q(4),
            max_args: Q(4),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any, Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: "delete_property".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: "clear_property".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: "is_clear_property".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: "server_version".to_string(),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: "renumber".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: "reset_max_object".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: "memory_usage".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: "shutdown".to_string(),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_STR)],
            implemented: true,
        },
        Builtin {
            name: "dump_database".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: "db_disk_size".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: "open_network_connection".to_string(),
            min_args: Q(0),
            max_args: U,
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: "connected_players".to_string(),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Any],
            implemented: true,
        },
        Builtin {
            name: "connected_seconds".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "idle_seconds".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "connection_name".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "notify".to_string(),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any],
            implemented: true,
        },
        Builtin {
            name: "boot_player".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "set_connection_option".to_string(),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any],
            implemented: false,
        },
        Builtin {
            name: "connection_option".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR)],
            implemented: false,
        },
        Builtin {
            name: "connection_options".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: "listen".to_string(),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Any, Any],
            implemented: false,
        },
        Builtin {
            name: "unlisten".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Any],
            implemented: false,
        },
        Builtin {
            name: "listeners".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: false,
        },
        Builtin {
            name: "buffered_output_length".to_string(),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: "task_id".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: "queued_tasks".to_string(),
            min_args: Q(0),
            max_args: Q(0),
            types: vec![],
            implemented: true,
        },
        Builtin {
            name: "kill_task".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_INT)],
            implemented: true,
        },
        Builtin {
            name: "output_delimiters".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: "queue_info".to_string(),
            min_args: Q(0),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: false,
        },
        Builtin {
            name: "resume".to_string(),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_INT), Any],
            implemented: true,
        },
        Builtin {
            name: "force_input".to_string(),
            min_args: Q(2),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_STR), Any],
            implemented: false,
        },
        Builtin {
            name: "flush_input".to_string(),
            min_args: Q(1),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: false,
        },
        Builtin {
            name: "verbs".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_OBJ)],
            implemented: true,
        },
        Builtin {
            name: "verb_info".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: true,
        },
        Builtin {
            name: "set_verb_info".to_string(),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Any, Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: "verb_args".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: true,
        },
        Builtin {
            name: "set_verb_args".to_string(),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Any, Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: "add_verb".to_string(),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Typed(TYPE_LIST), Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: "delete_verb".to_string(),
            min_args: Q(2),
            max_args: Q(2),
            types: vec![Typed(TYPE_OBJ), Any],
            implemented: true,
        },
        Builtin {
            name: "verb_code".to_string(),
            min_args: Q(2),
            max_args: Q(4),
            types: vec![Typed(TYPE_OBJ), Any, Any, Any],
            implemented: true,
        },
        Builtin {
            name: "set_verb_code".to_string(),
            min_args: Q(3),
            max_args: Q(3),
            types: vec![Typed(TYPE_OBJ), Any, Typed(TYPE_LIST)],
            implemented: true,
        },
        Builtin {
            name: "eval".to_string(),
            min_args: Q(1),
            max_args: Q(1),
            types: vec![Typed(TYPE_STR)],
            implemented: true,
        },
    ]
}

pub fn make_builtin_labels() -> HashMap<String, Name> {
    let mut b = HashMap::new();
    for (i, builtin) in BUILTIN_DESCRIPTORS.iter().enumerate() {
        b.insert(builtin.name.clone(), Name(i as u16));
    }

    b
}
pub fn make_labels_builtins() -> HashMap<Name, String> {
    let mut b = HashMap::new();
    for (i, builtin) in BUILTIN_DESCRIPTORS.iter().enumerate() {
        b.insert(Name(i as u16), builtin.name.clone());
    }

    b
}

pub fn offset_for_builtin(bf_name: &str) -> usize {
    BUILTIN_DESCRIPTORS
        .iter()
        .position(|b| b.name == bf_name)
        .unwrap()
}
