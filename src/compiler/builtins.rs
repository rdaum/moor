use std::collections::HashMap;

use crate::compiler::labels::Label;

pub const BUILTINS: &[&str] = &[
    // disassemble
    "disassemble",
    // functions
    "function_info",
    "load_server_options",
    // values
    "value_bytes",
    "value_hash",
    "string_hash",
    "binary_hash",
    "decode_binary",
    "encode_binary",
    // list
    "length",
    "setadd",
    "setremove",
    "listappend",
    "listinsert",
    "listdelete",
    "listset",
    "equal",
    "is_member",
    // string
    "tostr",
    "toliteral",
    "match",
    "rmatch",
    "substitute",
    "crypt",
    "index",
    "rindex",
    "strcmp",
    "strsub",
    // numbers
    "toint",
    "tonum",
    "tofloat",
    "min",
    "max",
    "abs",
    "random",
    "time",
    "ctime",
    "floatstr",
    "sqrt",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "sinh",
    "cosh",
    "tanh",
    "exp",
    "log",
    "log10",
    "ceil",
    "floor",
    "trunc",
    // objects
    "toobj",
    "typeof",
    "create",
    "recycle",
    "object_bytes",
    "valid",
    "parent",
    "children",
    "chparent",
    "max_object",
    "players",
    "is_player",
    "set_player_flag",
    "move",
    // property
    "properties",
    "property_info",
    "set_property_info",
    "add_property",
    "delete_property",
    "clear_property",
    "is_clear_property",
    // verbs
    "verbs",
    "verb_info",
    "set_verb_info",
    "verb_args",
    "set_verb_args",
    "add_verb",
    "delete_verb",
    "verb_code",
    "set_verb_code",
    "eval",
    // server
    "server_version",
    "renumber",
    "reset_max_object",
    "memory_usage",
    "shutdown",
    "dump_database",
    "db_disk_size",
    "open_network_connection",
    "connected_players",
    "connected_seconds",
    "idle_seconds",
    "connection_name",
    "notify",
    "boot_player",
    "set_connection_option",
    "connection_option",
    "connection_options",
    "listen",
    "unlisten",
    "listeners",
    "buffered_output_length",
    // tasks
    "task_id",
    "queued_tasks",
    "kill_task",
    "output_delimiters",
    "queue_info",
    "resume",
    "force_input",
    "flush_input",
    // log
    "server_log",
    // execute
    "call_function",
    "raise",
    "suspend",
    "read",
    "seconds_left",
    "ticks_left",
    "pass",
    "set_task_perms",
    "caller_perms",
    "callers",
    "task_stack",
];

pub fn make_builtin_labels() -> HashMap<String, Label> {
    let mut b = HashMap::new();
    for (i, builtin) in BUILTINS.iter().enumerate() {
        b.insert(builtin.to_string(), Label(i as u32));
    }

    b
}

pub fn offset_for_builtin(bf_name: &str) -> Option<usize> {
    BUILTINS.iter().position(|b| *b == bf_name)
}
