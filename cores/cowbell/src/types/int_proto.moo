object INT_PROTO [
  import_export_id -> "int_proto",
  import_export_hierarchy -> {"types"}
]
  name: "Integer Prototype"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Prototype object for integer utility methods.";

  method format_time_seconds owner: HACKER
    "Convert integer seconds to human-readable time format";
    "Usage: seconds_value:format_time_seconds()";
    {seconds} = args;
    typeof(seconds) != TYPE_INT && raise(E_TYPE, "Method must be called on integer");
    seconds < 0 && raise(E_INVARG, "Seconds cannot be negative");
    seconds < 60 && return tostr(seconds, "s");
    minutes = seconds / 60;
    remaining_seconds = seconds % 60;
    minutes < 60 && return tostr(minutes, "m", remaining_seconds, "s");
    hours = minutes / 60;
    remaining_minutes = minutes % 60;
    hours < 24 && return tostr(hours, "h", remaining_minutes, "m");
    days = hours / 24;
    remaining_hours = hours % 24;
    return tostr(days, "d", remaining_hours, "h");
  endmethod

  method test_format_time_seconds_basic owner: HACKER
    "Test basic time formatting for seconds, minutes, hours";
    "Test seconds only";
    30:format_time_seconds() != "30s" && return E_ASSERT;
    0:format_time_seconds() != "0s" && return E_ASSERT;
    59:format_time_seconds() != "59s" && return E_ASSERT;
    "Test minutes and seconds";
    60:format_time_seconds() != "1m0s" && return E_ASSERT;
    90:format_time_seconds() != "1m30s" && return E_ASSERT;
    3599:format_time_seconds() != "59m59s" && return E_ASSERT;
    "Test hours and minutes";
    3600:format_time_seconds() != "1h0m" && return E_ASSERT;
    7890:format_time_seconds() != "2h11m" && return E_ASSERT;
    return true;
  endmethod

  method test_format_time_seconds_edge_cases owner: HACKER
    "Test edge cases and error conditions";
    "Test days";
    86400:format_time_seconds() != "1d0h" && return E_ASSERT;
    90061:format_time_seconds() != "1d1h" && return E_ASSERT;
    "Test error conditions";
    try
      (-1):format_time_seconds();
      return E_ASSERT;
    except e (E_INVARG)
      "Expected error for negative seconds";
    endtry
    return true;
  endmethod

  method test_format_time_seconds_realistic owner: HACKER
    "Test realistic idle/connection times";
    "Test typical idle times";
    180:format_time_seconds() != "3m0s" && return E_ASSERT;
    1800:format_time_seconds() != "30m0s" && return E_ASSERT;
    5400:format_time_seconds() != "1h30m" && return E_ASSERT;
    "Test long connection times";
    28800:format_time_seconds() != "8h0m" && return E_ASSERT;
    172800:format_time_seconds() != "2d0h" && return E_ASSERT;
    return true;
  endmethod

  method format_bytes owner: HACKER
    "Format bytes as human-readable size string";
    "Usage: bytes_value:format_bytes()";
    {bytes} = args;
    typeof(bytes) != TYPE_INT && raise(E_TYPE, "Method must be called on integer");
    bytes < 0 && raise(E_INVARG, "Bytes cannot be negative");
    bytes < 1024 && return "<1K";
    kb = bytes / 1024;
    kb < 10 && return " " + tostr(kb) + "K";
    return tostr(kb) + "K";
  endmethod

  method test_format_bytes owner: HACKER
    "Test byte formatting";
    "Test less than 1KB";
    0:format_bytes() != "<1K" && return E_ASSERT;
    512:format_bytes() != "<1K" && return E_ASSERT;
    1023:format_bytes() != "<1K" && return E_ASSERT;
    "Test KB formatting";
    1024:format_bytes() != " 1K" && return E_ASSERT;
    5120:format_bytes() != " 5K" && return E_ASSERT;
    9216:format_bytes() != " 9K" && return E_ASSERT;
    "Test larger KB values";
    10240:format_bytes() != "10K" && return E_ASSERT;
    15360:format_bytes() != "15K" && return E_ASSERT;
    1048576:format_bytes() != "1024K" && return E_ASSERT;
    return true;
  endmethod
endobject
