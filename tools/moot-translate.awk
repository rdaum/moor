#!/usr/bin/env -S awk -f
# Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.
#

/ *end$/ { next }

{
    # Array syntax, comments, strings
    gsub(/\[/, "{");
    gsub(/\]/, "}");
    gsub(/#/, "//");
    gsub(/'/, "\"");

    # Standard corified references
    gsub("NOTHING", "$nothing");
    gsub(":nothing", "$nothing");
    gsub("AMBIGUOUS_MATCH", "$ambiguous_match");
    gsub("FAILED_MATCH", "$failed_match");
    gsub("INVALID_OBJECT", "$invalid_object");

    # Other corified references
    gsub(":object", "$object");

    s = $0;

    # `kahuna` is a helper function used by the Stunt test suite in a few test files. It has different implementations in each.
    # The active implementation here is not privileged, it's the one I happened to be using last.

    # test_objects kahuna: https://github.com/toddsundsted/stunt/blob/e83e946/test/test_objects.rb#L2017
    # def kahuna(parent, location, name)
    #   object = create(parent)
    #   move(object, location)
    #   set(object, 'name', name)
    #   add_property(object, name, name, [player, ''])
    #   add_verb(object, [player, 'xd', name], ['this', 'none', 'this'])
    #    set_verb_code(object, name) do |vc|
    #     vc << %|return this.#{name};|
    #  end
    #  object
    # end
    s = gensub(/(.*) = kahuna\((.*), (.*), "(.*)"\)/, \
        "// \\1 = kahuna(\\2, \\3, '\\4')" ORS \
        "; add_property($system, \"\\1\", create(\\2), {player, \"wrc\"});" ORS \
        "; move($\\1, \\3);" ORS \
        "; $\\1.name = \"\\4\";" ORS \
        "; add_property($\\1, \"\\4\", \"\\4\", {player, \"\"});" ORS \
        "; add_verb($\\1, {player, \"xd\", \"\\4\"}, {\"this\", \"none\", \"this\"});" ORS \
        "; set_verb_code(\\$\\1, \"\\4\", {\"return this.\\4;\"});" ORS \
        "// EOF \\1 = kahuna(\\2, \\3, '\\4')" ORS, \
        "g", s \
    );

    # Assigment. Watch out: any variable names that are built-in MOO properties must be manually changed.
    s = gensub(/^([a-z_]*) = (.*)/, "; add_property($system, \"\\1\", \\2, {player, \"wrc\"});", "g", s);

    # assert_equal. Heuristics: LHS is the expected value. RHS is a function call.
    # Bunch of special cases to save time on manual postprocessing when we know(ish) that an argument will be
    # an object that gets corified in the `.moot` test
    s = gensub(/^assert_equal (.*), parent\((.*)\)/, "; return parent($\\2);\n\\1", "g", s);
    s = gensub(/^assert_equal (.*), children\((.*)\)/, "; return children($\\2);\n\\1", "g", s);
    s = gensub(/^assert_equal (.*), get\((.*), ['"](.*)['"]\)/, "; return $\\2.\\3;\n\\1", "g", s);
    s = gensub(/^assert_equal (.*), call\((.*), "(.*)"\)/, "; return $\\2:\\3();\n\\1", "g", s);
    s = gensub(/^assert_equal (.*), ([a-z_]+\(.+\))/, "; return \\2;\n\\1", "g", s);

    # assert_not_equal. Same heuristics as assert_equal.
    s = gensub(/^assert_not_equal (.*), ([a-z_]+\(.+\))/, "; return \\1 == \\2;\n0", "g", s);

    # set(obj, field, value)
    s = gensub(/^set\((.*), ['"](.*)['"], (.*)\)/, "; $\\1.\\2 = \\3;", "g", s);

    # return set(obj, field, value)
    s = gensub(/^return set\((.*), ['"](.*)['"], (.*)\)/, "; return \\1.\\2 = \\3;", "g", s);

    # get(obj, field)
    s = gensub(/^get\((.*), ['"](.*)['"]\)/, "; return $\\1.\\2;", "g", s);

    # function calls with parens
    s = gensub(/^([a-z_]+)\((.*)\)$/, "; \\1(\\2);", "g", s);

    # function calls without parens, because yay Ruby
    s = gensub(/^([a-z_]+) (.*)$/, "; \\1(\\2);", "g", s);

    # run_test_as
    s = gensub(/^run_test_as\(['"](.*)['"]\) do/, "@\\1", "g", s);

    print s
}
