# From LambdaMOO 1.8.x `function_info()` invocation.
bf_string = """
{{"disassemble", 2, 2, {1, -1}}, {"log_cache_stats", 0, 0, {}}, {"verb_cache_stats", 0, 0, {}}, 
{"call_function", 1, -1, {2}}, {"raise", 1, 3, {-1, 2, -1}}, {"suspend", 0, 1, {0}}, {"read", 0, 2, {1, 
-1}}, {"seconds_left", 0, 0, {}}, {"ticks_left", 0, 0, {}}, {"pass", 0, -1, {}}, {"set_task_perms", 1, 
1, {1}}, {"caller_perms", 0, 0, {}}, {"callers", 0, 1, {-1}}, {"task_stack", 1, 2, {0, -1}}, 
{"function_info", 0, 1, {2}}, {"load_server_options", 0, 0, {}}, {"value_bytes", 1, 1, {-1}}, 
{"value_hash", 1, 1, {-1}}, {"string_hash", 1, 1, {2}}, {"binary_hash", 1, 1, {2}}, {"decode_binary", 
1, 2, {2, -1}}, {"encode_binary", 0, -1, {}}, {"length", 1, 1, {-1}}, {"setadd", 2, 2, {4, -1}}, 
{"setremove", 2, 2, {4, -1}}, {"listappend", 2, 3, {4, -1, 0}}, {"listinsert", 2, 3, {4, -1, 0}}, 
{"listdelete", 2, 2, {4, 0}}, {"listset", 3, 3, {4, -1, 0}}, {"equal", 2, 2, {-1, -1}}, {"is_member", 
2, 2, {-1, 4}}, {"tostr", 0, -1, {}}, {"toliteral", 1, 1, {-1}}, {"match", 2, 3, {2, 2, -1}}, 
{"rmatch", 2, 3, {2, 2, -1}}, {"substitute", 2, 2, {2, 4}}, {"crypt", 1, 2, {2, 2}}, {"index", 2, 3, 
{2, 2, -1}}, {"rindex", 2, 3, {2, 2, -1}}, {"strcmp", 2, 2, {2, 2}}, {"strsub", 3, 4, {2, 2, 2, -1}}, 
{"server_log", 1, 2, {2, -1}}, {"toint", 1, 1, {-1}}, {"tonum", 1, 1, {-1}}, {"tofloat", 1, 1, {-1}}, 
{"min", 1, -1, {-2}}, {"max", 1, -1, {-2}}, {"abs", 1, 1, {-2}}, {"random", 0, 1, {0}}, {"time", 0, 0, 
{}}, {"ctime", 0, 1, {0}}, {"floatstr", 2, 3, {9, 0, -1}}, {"sqrt", 1, 1, {9}}, {"sin", 1, 1, {9}}, 
{"cos", 1, 1, {9}}, {"tan", 1, 1, {9}}, {"asin", 1, 1, {9}}, {"acos", 1, 1, {9}}, {"atan", 1, 2, {9, 
9}}, {"sinh", 1, 1, {9}}, {"cosh", 1, 1, {9}}, {"tanh", 1, 1, {9}}, {"exp", 1, 1, {9}}, {"log", 1, 1, 
{9}}, {"log10", 1, 1, {9}}, {"ceil", 1, 1, {9}}, {"floor", 1, 1, {9}}, {"trunc", 1, 1, {9}}, {"toobj", 
1, 1, {-1}}, {"typeof", 1, 1, {-1}}, {"create", 1, 2, {1, 1}}, {"recycle", 1, 1, {1}}, {"object_bytes", 
1, 1, {1}}, {"valid", 1, 1, {1}}, {"parent", 1, 1, {1}}, {"children", 1, 1, {1}}, {"chparent", 2, 2, 
{1, 1}}, {"max_object", 0, 0, {}}, {"players", 0, 0, {}}, {"is_player", 1, 1, {1}}, {"set_player_flag", 
2, 2, {1, -1}}, {"move", 2, 2, {1, 1}}, {"properties", 1, 1, {1}}, {"property_info", 2, 2, {1, 2}}, 
{"set_property_info", 3, 3, {1, 2, 4}}, {"add_property", 4, 4, {1, 2, -1, 4}}, {"delete_property", 2, 
2, {1, 2}}, {"clear_property", 2, 2, {1, 2}}, {"is_clear_property", 2, 2, {1, 2}}, {"server_version", 
0, 1, {-1}}, {"renumber", 1, 1, {1}}, {"reset_max_object", 0, 0, {}}, {"memory_usage", 0, 0, {}}, 
{"shutdown", 0, 1, {2}}, {"dump_database", 0, 0, {}}, {"db_disk_size", 0, 0, {}}, 
{"open_network_connection", 0, -1, {}}, {"connected_players", 0, 1, {-1}}, {"connected_seconds", 1, 1, 
{1}}, {"idle_seconds", 1, 1, {1}}, {"connection_name", 1, 1, {1}}, {"notify", 2, 3, {1, 2, -1}}, 
{"boot_player", 1, 1, {1}}, {"set_connection_option", 3, 3, {1, 2, -1}}, {"connection_option", 2, 2, 
{1, 2}}, {"connection_options", 1, 1, {1}}, {"listen", 2, 3, {1, -1, -1}}, {"unlisten", 1, 1, {-1}}, 
{"listeners", 0, 0, {}}, {"buffered_output_length", 0, 1, {1}}, {"task_id", 0, 0, {}}, {"queued_tasks", 
0, 0, {}}, {"kill_task", 1, 1, {0}}, {"output_delimiters", 1, 1, {1}}, {"queue_info", 0, 1, {1}}, 
{"resume", 1, 2, {0, -1}}, {"force_input", 2, 3, {1, 2, -1}}, {"flush_input", 1, 2, {1, -1}}, {"verbs", 
1, 1, {1}}, {"verb_info", 2, 2, {1, -1}}, {"set_verb_info", 3, 3, {1, -1, 4}}, {"verb_args", 2, 2, {1, 
-1}}, {"set_verb_args", 3, 3, {1, -1, 4}}, {"add_verb", 3, 3, {1, 4, 4}}, {"delete_verb", 2, 2, {1, 
-1}}, {"verb_code", 2, 4, {1, -1, -1, -1}}, {"set_verb_code", 3, 3, {1, -1, 4}}, {"eval", 1, 1, {2}}}
"""

types = ["TYPE_INT", "TYPE_OBJ", "TYPE_STR", "TYPE_ERR", "TYPE_LIST", None, "TYPE_NONE", None, None, "TYPE_FLOAT"]


def argform(num):
    if num == -1:
        return "ArgCount::U"
    else:
        return "ArgCount::Q(%d)" % num


def typeform(num):
    if num == -1:
        return "ArgType::Any"
    elif num == -2:
        return "ArgType::AnyNum"
    else:
        return "ArgType::Typed(VarType::%s)" % types[num]


def output(bfs):
    output = []
    for x in bfs:
        format = '''
        Builtin {
            name: "%s".to_string(),
            min_args: %s,
            max_args: %s,
            types: vec![%s],
            implemented: true,
        }
        '''
        name = x[0]
        min_args = argform(x[1])
        max_args = argform(x[2])

        typeforms = ",".join([typeform(x) for x in x[3]])
        output.append(format % (name, min_args, max_args, typeforms))
    return output

bfs = eval(bf_string.replace("{", "[").replace("}", "]"))
print("vec![%s]" % ", \n".join(output(bfs)))
