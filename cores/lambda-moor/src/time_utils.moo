object TIME_UTILS
  name: "time utilities"
  parent: GENERIC_UTILS
  owner: HACKER
  readable: true

  property corr (owner: HACKER, flags: "rc") = -122;
  property ct (owner: HACKER, flags: "rc") = 7934;
  property ctcd (owner: HACKER, flags: "rc") = 7276;
  property dayabbrs (owner: HACKER, flags: "rc") = {"Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"};
  property days (owner: HACKER, flags: "rc") = {"Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"};
  property monthabbrs (owner: HACKER, flags: "rc") = {
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec"
  };
  property monthlens (owner: HACKER, flags: "rc") = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
  property months (owner: HACKER, flags: "rc") = {
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December"
  };
  property stsd (owner: HACKER, flags: "rc") = 2427;
  property time_units (owner: HACKER, flags: "rc") = {
    {31536000, "year", "years", "yr", "yrs"},
    {2628000, "month", "months", "mo", "mos"},
    {604800, "week", "weeks", "wk", "wks"},
    {86400, "day", "days", "dy", "dys"},
    {3600, "hour", "hours", "hr", "hrs"},
    {60, "minute", "minutes", "min", "mins"},
    {1, "second", "seconds", "sec", "secs"}
  };
  property timezones (owner: HACKER, flags: "rc") = {
    {"AuEST", -10},
    {"AuCST", -9},
    {"AuWST", -8},
    {"WET", -1},
    {"GMT", 0},
    {"AST", 4},
    {"EDT", 4},
    {"EST", 5},
    {"CDT", 5},
    {"CST", 6},
    {"MDT", 6},
    {"MST", 7},
    {"PDT", 7},
    {"PST", 8},
    {"HST", 10}
  };
  property zones (owner: HACKER, flags: "rc") = {
    {{"est", "edt", "Massachusetts", "MA"}, 10800},
    {{"cst", "cdt"}, 7200},
    {{"mst", "mdt"}, 3600},
    {{"pst", "pdt", "California", "CA", "Lambda"}, 0},
    {{"gmt"}, 28800}
  };

  override aliases = {"time utilities", "time"};
  override description = {
    "This is the time utilities utility package.  See `help $time_utils' for more details."
  };
  override help_msg = {
    "    Converting from seconds-since-1970    ",
    "dhms          (time)                 => string ...DD:HH:MM:SS",
    "english_time  (time[, reference time)=> string of y, m, d, h, m, s",
    "",
    "    Converting to seconds",
    "to_seconds    (\"hh:mm:ss\")           => seconds since 00:00:00",
    "from_ctime    (ctime)                => corresponding time-since-1970",
    "from_day      (day_of_week, which)   => time-since-1970 for the given day*",
    "from_month    (month, which)         => time-since-1970 for the given month*",
    "    (* the first midnight of that day/month)",
    "parse_english_time_interval(\"n1 u1 n2 u2...\")",
    "                                     => seconds in interval",
    "seconds_until_time(\"hh:mm:ss\")       => number of seconds from now until then",
    "seconds_until_date(\"month\",day,\"hh:mm:ss\",flag ",
    "                                     => number of seconds from now until then",
    "                                        (see verb help for details)",
    "",
    "    Converting to some standard English formats",
    "day           ([c]time)              => what day it is",
    "month         ([c]time)              => what month it is",
    "ampm          ([c]time[, precision]) => what time it is, with am or pm",
    "mmddyy        ([c]time)              => date in format MM/DD/YY",
    "ddmmyy        ([c]time)              => date in format DD/MM/YY",
    "",
    "    Substitution",
    "time_sub      (string, time)         => substitute time information",
    "",
    "    Miscellaneous",
    "sun           ([time])               => angle between sun and zenith",
    "dst_midnight  (time)                 "
  };
  override import_export_id = "time_utils";
  override object_size = {22076, 1084848672};

  verb day (none none none) owner: HACKER flags: "rxd"
    "Given a time() or ctime()-style date, this returns the full name of the day.";
    if (typeof(args[1]) == INT)
      time = ctime(args[1]);
    elseif (typeof(args[1]) == STR)
      time = args[1];
    else
      return E_TYPE;
    endif
    dayabbr = $string_utils:explode(time)[1];
    return this.days[dayabbr in this.dayabbrs];
  endverb

  verb month (none none none) owner: HACKER flags: "rxd"
    "Given a time() or ctime()-style date, this returns the full name";
    "of the month.";
    if (typeof(args[1]) == INT)
      time = ctime(args[1]);
    elseif (typeof(args[1]) == STR)
      time = args[1];
    else
      return E_TYPE;
    endif
    monthabbr = $string_utils:explode(time)[2];
    return this.months[monthabbr in this.monthabbrs];
  endverb

  verb ampm (none none none) owner: HACKER flags: "rxd"
    "Return a time in the form [h]h[:mm[:ss]] {a.m.|p.m.}.  Args are";
    "[1]   either a time()- or a ctime()-style date, and";
    "[2]   (optional) the precision desired--1 for hours, 2 for minutes,";
    "        3 for seconds.  If not given, precision defaults to minutes";
    {time, ?precision = 2} = args;
    if (typeof(time) == INT)
      time = ctime(time);
    elseif (typeof(time) != STR)
      return E_TYPE;
    endif
    time = $string_utils:explode(time)[4];
    hour = toint(time[1..2]);
    if (hour == 0)
      time = "12" + time[3..precision * 3 - 1] + " a.m.";
    elseif (hour == 12)
      time = time[1..precision * 3 - 1] + " p.m.";
    elseif (hour > 12)
      time = tostr(hour - 12) + time[3..precision * 3 - 1] + " p.m.";
    else
      time = tostr(hour) + time[3..precision * 3 - 1] + " a.m.";
    endif
    return time;
  endverb

  verb to_seconds (this none this) owner: HACKER flags: "rxd"
    "Given string hh:mm:ss ($string_utils:explode(ctime(time))[4]), this returns";
    "the number of seconds elapsed since 00:00:00.  I can't remember why I";
    "created this verb, but I'm sure it serves some useful purpose.";
    return 60 * 60 * toint((args[1])[1..2]) + 60 * toint((args[1])[4..5]) + toint((args[1])[7..8]);
  endverb

  verb sun (this none this) owner: HACKER flags: "rxd"
    {?time = time()} = args;
    r = 10000;
    h = r * r + r / 2;
    t = (time + 120) % 86400 / 240;
    s = 5 * ((time - 14957676) % 31556952) / 438291;
    phi = s + t + this.corr;
    cs = $trig_utils:cos(s);
    spss = ($trig_utils:sin(phi) * $trig_utils:sin(s) + h) / r - r;
    cpcs = ($trig_utils:cos(phi) * cs + h) / r - r;
    return (this.stsd * cs - this.ctcd * cpcs - this.ct * spss + h) / r - r;
  endverb

  verb from_ctime (this none this) owner: HACKER flags: "rxd"
    "Given a string such as returned by ctime(), return the corresponding time-in-seconds-since-1970 time returned by time(), or E_DIV if the format is wrong in some essential way.";
    words = $string_utils:explode(args[1]);
    if (length(words) == 5)
      "Arrgh!  the old ctime() didn't return a time zone, yet it arbitrarily decides whether it's standard or daylight savings time.  URK!!!!!";
      words = listappend(words, "PST");
    endif
    if (length(words) != 6 || length(hms = $string_utils:explode(words[4], ":")) != 3 || !(month = words[2] in this.monthabbrs) || !(zone = $list_utils:assoc(words[6], this.timezones)))
      return E_DIV;
    endif
    year = toint(words[5]);
    day = {-1, 30, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334}[month] + toint(words[3]) + year * 366;
    zone = zone[2];
    return (((day - (day + 1038) / 1464 - (day + 672) / 1464 - (day + 306) / 1464 - (day + 109740) / 146400 - (day + 73140) / 146400 - (day + 36540) / 146400 - 719528) * 24 + toint(hms[1]) + zone) * 60 + toint(hms[2])) * 60 + toint(hms[3]);
  endverb

  verb "dhms dayshoursminutesseconds" (this none this) owner: HACKER flags: "rxd"
    s = args[1];
    if (s < 0)
      return "-" + this:(verb)(-s);
    endif
    m = s / 60;
    s = s % 60;
    if (m)
      ss = tostr(s < 10 ? ":0" | ":", s);
      h = m / 60;
      m = m % 60;
      if (h)
        ss = tostr(m < 10 ? ":0" | ":", m, ss);
        d = h / 24;
        h = h % 24;
        return tostr(@d ? {d, h < 10 ? ":0" | ":"} | {}, h, ss);
      else
        return tostr(m, ss);
      endif
    else
      return tostr(s);
    endif
  endverb

  verb english_time (this none this) owner: HACKER flags: "rxd"
    "english_time(time [,reference time]): returns the time as a string of";
    "years, months, days, hours, minutes and seconds using the reference time as";
    "the start time and incrementing forwards. it can be given in either ctime()";
    "or time() format. if a reference time is not given, it is set to time().";
    {_time, ?reftime = time()} = args;
    if (_time < 1)
      return "0 seconds";
    endif
    _ctime = typeof(reftime) == INT ? ctime(reftime) | reftime;
    seclist = {60, 60, 24};
    units = {"year", "month", "day", "hour", "minute", "second"};
    timelist = {};
    for unit in (seclist)
      timelist = {_time % unit, @timelist};
      _time = _time / unit;
    endfor
    months = 0;
    month = _ctime[5..7] in $time_utils.monthabbrs;
    year = toint(_ctime[21..24]);
    "attribution: the algorithm used is from the eminently eminent g7.";
    while (_time >= (days = this.monthlens[month] + (month == 2 && year % 4 == 0 && !(year % 400 in {100, 200, 300}))))
      _time = _time - days;
      months = months + 1;
      if ((month = month + 1) > 12)
        year = year + 1;
        month = 1;
      endif
      $command_utils:suspend_if_needed(0);
    endwhile
    timelist = {months / 12, months % 12, _time, @timelist};
    for unit in (units)
      i = unit in units;
      if (timelist[i] > 0)
        units[i] = tostr(timelist[i]) + " " + units[i] + (timelist[i] == 1 ? "" | "s");
      else
        units = listdelete(units, i);
        timelist = listdelete(timelist, i);
      endif
    endfor
    return $string_utils:english_list(units);
  endverb

  verb from_day (this none this) owner: HACKER flags: "rxd"
    "from_day(day_of_week,which [,reference time])";
    "numeric time (seconds since 1970) corresponding to midnight (PST) of the given weekday.  Use either the name of the day or a 1..7 number (1==Sunday,...)";
    "  which==-1 => use most recent such day.";
    "  which==+1 => use first upcoming such day.";
    "  which==0  => use closest such day.";
    "larger (absolute) values for which specify a certain number of weeks into the future or past.";
    {day, ?dir = 0, ?reftime = time()} = args;
    if (!(toint(day) || (day = $string_utils:find_prefix(day, this.days))))
      return E_DIV;
    endif
    delta = {288000, 374400, 460800, 547200, 28800, 115200, 201600}[toint(day)];
    time = reftime - delta;
    if (dir)
      time = time / 604800 + (dir > 0 ? dir | dir + 1);
    else
      time = (time + 302400) / 604800;
    endif
    return time * 604800 + delta;
  endverb

  verb from_month (this none this) owner: HACKER flags: "rxd"
    "from_month(month,which[,d])";
    "numeric time (seconds since 1970) corresponding to midnight (PST) of the dth (first) day of the given month.  Use either the month name or a 1..12 number (1==January,...)";
    "  which==-1 => use most recent such month.";
    "  which==+1 => use first upcoming such month.";
    "  which==0  => use closest such month.";
    "larger (absolute) values for which specify a certain number of years into the future or past.";
    {month, ?dir = 0, ?dth = 1} = args;
    if (!(toint(month) || (month = $string_utils:find_prefix(month, this.months))))
      return E_DIV;
    endif
    delta = {0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334}[month] + dth - 1;
    day = (time() - 28800) / 86400;
    day = day - (day + 672) / 1461 - delta;
    if (dir)
      day = day / 365 + dir + (dir <= 0);
    else
      day = (2 * day + 365) / 730;
    endif
    day = day * 365 + delta;
    day = day + (day + 671) / 1460;
    return day * 86400 + 28800;
  endverb

  verb dst_midnight (this none this) owner: HACKER flags: "rxd"
    "Takes a time that is midnight PST and converts it to the nearest PDT midnight time if it's during that part of the year where we use PDT.";
    time = args[1];
    return time - 3600 * ((toint(ctime(time)[12..13]) + 12) % 24 - 12);
  endverb

  verb time_sub (this none this) owner: HACKER flags: "rxd"
    "Works like pronoun substitution, but substitutes time stuff.";
    "Call with time_sub(string, time). returns a string.";
    "time is an optional integer in time() format.  If omitted, time() is used.";
    "Macros which are unknown are ignored. $Q -> the empty string.";
    "Terminal $ are ignored.";
    "$H -> hour #. $M -> min #. $S -> second #. 24-hour format, fixed width.";
    "$h, $m, $s same x/c have not-fixed format. 00:03:24 vs. 0:3:24";
    "$O/$o -> numeric hour in 12-hour format.";
    "$D -> long day name. $d -> short day name.";
    "$N -> long month name. $n -> short month name.";
    "$Y -> long year # (e.g. '1991'). $y -> short year # (e.g. '91')";
    "$Z -> the time zone    (added in by r'm later)";
    "$P/$p -> AM/PM, or am/pm.";
    "$T -> date number. $t -> date number with no extra whitespace etc.";
    "$1 -> Month in fixed-width numeric format (01-12)   (added by dpk)";
    "$2 -> Month in nonfixed numeric format (1-12)";
    "$3 -> Date in fixed-width format, 0-fill";
    "$$ -> $.";
    "";
    "This verb stolen from Ozymandias's #4835:time_subst.";
    res = "";
    {thestr, ?thetime = time()} = args;
    if (typeof(thestr) != STR || typeof(thetime) != INT)
      player:tell("Bad arguments to time_subst.");
      return;
    endif
    itslength = length(thestr);
    if (!itslength)
      return "";
    endif
    done = 0;
    cctime = ctime(thetime);
    while (dollar = index(thestr, "$"))
      res = res + thestr[1..dollar - 1];
      if (dollar == length(thestr))
        return res;
      endif
      thechar = thestr[dollar + 1];
      thestr[1..dollar + 1] = "";
      if (thechar == "$")
        res = res + "$";
      elseif (!strcmp(thechar, "h"))
        res = res + $string_utils:trim(tostr(toint(cctime[12..13])));
      elseif (thechar == "H")
        res = res + cctime[12..13];
      elseif (!strcmp(thechar, "m"))
        res = res + $string_utils:trim(tostr(toint(cctime[15..16])));
      elseif (thechar == "M")
        res = res + cctime[15..16];
      elseif (!strcmp(thechar, "s"))
        res = res + $string_utils:trim(tostr(toint(cctime[18..19])));
      elseif (thechar == "S")
        res = res + cctime[18..19];
      elseif (!strcmp(thechar, "D"))
        res = res + $time_utils:day(thetime);
      elseif (thechar == "d")
        res = res + cctime[1..3];
      elseif (!strcmp(thechar, "N"))
        res = res + $time_utils:month(thetime);
      elseif (thechar == "n")
        res = res + cctime[5..7];
      elseif (!strcmp(thechar, "T"))
        res = res + cctime[9..10];
      elseif (thechar == "t")
        res = res + $string_utils:trim(cctime[9..10]);
      elseif (!strcmp(thechar, "o"))
        res = tostr(res, (toint(cctime[12..13]) + 11) % 12 + 1);
      elseif (thechar == "O")
        res = res + $string_utils:right(tostr((toint(cctime[12..13]) + 11) % 12 + 1), 2, "0");
      elseif (!strcmp(thechar, "p"))
        res = res + (toint(cctime[12..13]) >= 12 ? "pm" | "am");
      elseif (thechar == "P")
        res = res + (toint(cctime[12..13]) >= 12 ? "PM" | "AM");
      elseif (!strcmp(thechar, "y"))
        res = res + cctime[23..24];
      elseif (thechar == "Y")
        res = res + cctime[21..24];
      elseif (thechar == "Z")
        res = res + cctime[26..$];
      elseif (thechar == "1")
        res = res + $string_utils:right(tostr($string_utils:explode(cctime)[2] in this.monthabbrs), 2, "0");
      elseif (thechar == "2")
        res = res + tostr($string_utils:explode(cctime)[2] in this.monthabbrs);
      elseif (thechar == "3")
        res = res + $string_utils:subst(cctime[9..10], {{" ", "0"}});
      endif
    endwhile
    return res + thestr;
  endverb

  verb "mmddyy ddmmyy" (this none this) owner: HACKER flags: "rxd"
    "Copied from Archer (#52775):mmddyy Tue Apr  6 17:04:26 1993 PDT";
    "Given a time() or ctime()-style date and an optional separator, this returns the MM/DD/YY or DD/MM/YY form of the date (depending on the verb called.)  The default seperator is '/'";
    {time, ?divstr = "/"} = args;
    if (typeof(time) == INT)
      time = ctime(time);
    elseif (typeof(time) != STR)
      return E_TYPE;
    endif
    date = $string_utils:explode(time);
    day = toint(date[3]);
    month = date[2] in $time_utils.monthabbrs;
    year = date[5];
    daystr = day < 10 ? "0" + tostr(day) | tostr(day);
    monthstr = month < 10 ? "0" + tostr(month) | tostr(month);
    yearstr = tostr(year)[3..4];
    if (verb == "mmddyy")
      return tostr(monthstr, divstr, daystr, divstr, yearstr);
    else
      return tostr(daystr, divstr, monthstr, divstr, yearstr);
    endif
  endverb

  verb parse_english_time_interval (this none this) owner: HACKER flags: "rxd"
    "$time_utils:parse_english_time_interval(n1,u1,n2,u2,...)";
    "or $time_utils:parse_english_time_interval(\"n1 u1[,] [and] n2[,] u2 [and] ...\")";
    "There must be an even number of arguments, all of which must be strings,";
    " or there must be just one argument which is the entire string to be parsed.";
    "The n's are are numeric strings, and the u's are unit names.";
    "The known units are in $time_utils.time_units,";
    " which must be kept sorted with bigger times at the head.";
    "Returns the time represented by those words.";
    "For example,";
    " $time_utils:parse_english_time_interval(\"30\",\"secs\",\"2\",\"minutes\",\"31\",\"seconds\") => 181";
    if (length(args) == 1 && index(args[1], " "))
      return $time_utils:parse_english_time_interval(@$string_utils:words(args[1]));
    endif
    a = $list_utils:setremove_all(args, "and");
    nargs = length(a);
    if (nargs % 2)
      return E_ARGS;
    endif
    nsec = 0;
    n = 0;
    for i in [1..nargs]
      if (i % 2 == 1)
        if ($string_utils:is_numeric(a[i]))
          n = toint(a[i]);
        elseif (a[i] in {"a", "an"})
          n = 1;
        elseif (a[i] in {"no"})
          n = 0;
        else
          return E_INVARG;
        endif
      else
        unit = a[i];
        if (unit[$] == ",")
          unit = unit[1..$ - 1];
        endif
        ok = 0;
        for entry in ($time_utils.time_units)
          if (!ok && unit in entry[2..$])
            nsec = nsec + entry[1] * n;
            ok = 1;
          endif
        endfor
        if (!ok)
          return E_INVARG;
        endif
      endif
    endfor
    return nsec;
  endverb

  verb seconds_until_date (this none this) owner: HACKER flags: "rx"
    "Copied from Ballroom Complex (#29992):from_date by Keelah! (#30246) Tue Jul 13 19:42:32 1993 PDT";
    ":seconds_until_date(month,day,time,which)";
    "month is a string or the numeric representation of the month, day is a number, time is a string in the following format, hh:mm:ss.";
    "which==-1 => use most recent such month.";
    "which==+1 => use first upcoming such month.";
    "which==0 => use closest such month.";
    "This will return the number of seconds until the month, day and time given to it.";
    "Written by Keelah, on July 5, 1993.";
    {month, day, time, which} = args;
    converted = 0;
    converted = converted + $time_utils:from_month(month, which, day);
    current = this:seconds_until_time("12:00:00");
    get_seconds = this:seconds_until_time(time);
    if (get_seconds < 0)
      get_seconds = get_seconds + 39600 - current;
    else
      get_seconds = get_seconds + 39600 - current;
    endif
    converted = converted + get_seconds - time();
    return converted;
  endverb

  verb seconds_until_time (this none this) owner: HACKER flags: "rx"
    "Copied from Ballroom Complex (#29992):seconds_until by Keelah! (#30246) Tue Jul 13 19:42:37 1993 PDT";
    ":seconds_until_time(hh:mm:ss)";
    "Given the string hh:mm:ss, this returns the number of seconds until that hh:mm:ss. If the hh:mm:ss is before the current time(), the number returned is a negative, else the number is a positive.";
    "Written by Keelah, on July 4, 1993.";
    current = $time_utils:to_seconds(ctime()[12..19]);
    time = $time_utils:to_seconds(args[1]);
    return toint(time) - toint(current);
  endverb

  verb rfc822_ctime (this none this) owner: #2 flags: "rxd"
    "Just like ctime(), but rfc-822 compliant.  I hope.";
    c = $string_utils:Explode(ctime(@args));
    return tostr(c[1], ", ", c[3], " ", c[2], " ", c[5], " ", c[4], " ", c[6]);
    "Last modified Fri Oct 17 23:17:25 1997 EDT by neuro (#3642) on opal moo.";
  endverb

  verb "mmddyyyy ddmmyyyy" (this none this) owner: HACKER flags: "rxd"
    "Given a time() or ctime()-style date and an optional separator, this returns the MM/DD/YYYY or DD/MM/YYYY form of the date (depending on the verb called.)  The default seperator is '/'";
    {time, ?divstr = "/"} = args;
    if (typeof(time) == INT)
      time = ctime(time);
    elseif (typeof(time) != STR)
      return E_TYPE;
    endif
    date = $string_utils:explode(time);
    day = toint(date[3]);
    month = date[2] in $time_utils.monthabbrs;
    year = date[5];
    daystr = day < 10 ? "0" + tostr(day) | tostr(day);
    monthstr = month < 10 ? "0" + tostr(month) | tostr(month);
    yearstr = tostr(year);
    if (verb == "mmddyyyy")
      return tostr(monthstr, divstr, daystr, divstr, yearstr);
    else
      return tostr(daystr, divstr, monthstr, divstr, yearstr);
    endif
  endverb
endobject