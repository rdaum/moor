#!/usr/bin/env -S awk -f
{
    # Array syntax, comments, strings
    gsub(/\[/, "{");
    gsub(/\]/, "}");
    gsub(/#/, "//");
    gsub(/'/, "\"");

    # Standard corified references
    gsub("NOTHING", "$nothing");
    gsub("AMBIGUOUS_MATCH", "$ambiguous_match");
    gsub("FAILED_MATCH", "$failed_match");
    gsub("INVALID_OBJECT", "$invalid_object");

    # Assigment. Watch out: any variable names that are built-in MOO properties must be manually changed.
    s = gensub(/^(.*) = (.*)/, "; add_property($system, \"\\1\", \\2, {player, \"wrc\"});", "g", $0);

    # assert_equal. Heuristics: LHS is the expected value. RHS is a function call.
    s = gensub(/^assert_equal (.*), ([a-z_]+\(.+\))/, "; return \\2;\n\\1", "g", s); 

    # assert_not_equal. Same heuristics as assert_equal.
    s = gensub(/^assert_not_equal (.*), ([a-z_]+\(.+\))/, "; return \\1 == \\2;\n0", "g", s); 

    # set(obj, field, value)
    s = gensub(/^set\((.*), ['"](.*)['"], (.*)\)/, "; \\1.\\2 = \\3;", "g", s);

    # get(obj, field)
    s = gensub(/^get\((.*), ['"](.*)['"]\)/, "; return \\1.\\2;", "g", s);

    # function calls with parens
    s = gensub(/^([a-z_]+)\((.*)\)$/, "; \\1(\\2);", "g", s);

    # function calls without parens, because yay Ruby
    s = gensub(/^([a-z_]+) (.*)$/, "; \\1(\\2);", "g", s);

    # TODO: somehow rewrite common variables like `a` into `$a`, but only when used as a variable?
    #       this might be too hard to do without a full parser

    print s
}