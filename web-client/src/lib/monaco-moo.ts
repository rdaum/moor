// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

// Monaco editor language support for MOO

import type { Monaco } from "@monaco-editor/react";

let isRegistered = false;

/**
 * Register MOO language support for Monaco editor
 * Safe to call multiple times - will only register once
 */
export function registerMooLanguage(monaco: Monaco): void {
    if (isRegistered) {
        return;
    }

    // Register MOO language
    monaco.languages.register({ id: "moo" });

    // Define MOO language tokens
    monaco.languages.setMonarchTokensProvider("moo", {
        tokenizer: {
            root: [
                // Control flow keywords
                [
                    /\b(if|elseif|else|endif|while|endwhile|for|endfor|try|except|endtry|finally|fork|endfork|begin|end)\b/,
                    "keyword.control",
                ],

                // Flow control
                [/\b(return|break|continue|pass|raise)\b/, "keyword.control"],

                // Declaration keywords
                [/\b(let|const|global|fn|endfn)\b/, "keyword.declaration"],

                // Special keywords
                [/\b(any|in)\b/, "keyword.operator"],

                // Built-in constants
                [/\b(true|false)\b/, "constant.language"],

                // Type constants
                [/\b(INT|NUM|FLOAT|STR|ERR|OBJ|LIST|MAP|BOOL|FLYWEIGHT|SYM)\b/, "type"],

                // Error constants (without parentheses, to allow string highlighting inside)
                [/\bE_[A-Z_]+\b/, "constant.other"],

                // Binary literals (base64-encoded)
                [/b"[A-Za-z0-9+/=_-]*"/, "string.binary"],

                // Object references (#123, #-1)
                [/#-?\d+/, "number.hex"],

                // System properties and verbs ($property)
                [/\$[a-zA-Z_][a-zA-Z0-9_]*/, "variable.predefined"],

                // Try expression start delimiter (backtick)
                [/`/, { token: "keyword.try", next: "@tryExpression", bracket: "@open" }],

                // Symbols ('symbol)
                [/'[a-zA-Z_][a-zA-Z0-9_]*/, "string.key"],

                // Range end marker ($)
                [/\$(?=\s*[\]})])/, "constant.numeric"],

                // Strings
                [/"([^"\\]|\\.)*$/, "string.invalid"],
                [/"/, "string", "@string"],

                // Numbers - floats first to avoid conflicts
                [/\d*\.\d+([eE][-+]?\d+)?/, "number.float"],
                [/\d+[eE][-+]?\d+/, "number.float"],
                [/\d+/, "number"],

                // Operators - order matters, specific to general
                [/\.\./, "keyword.operator"], // Range operator
                [/->/, "keyword.operator"], // Map arrow
                [/=>/, "keyword.operator"], // Lambda arrow
                [/>>>/, "operator.bitwise"],
                [/>>/, "operator.bitwise"],
                [/<<(?!=)/, "operator.bitwise"],
                [/&\./, "operator.bitwise"],
                [/\|\./, "operator.bitwise"],
                [/\^\./, "operator.bitwise"],
                [/(==|!=|<=|>=)/, "operator.comparison"],
                [/(&&|\|\|)/, "operator.logical"],
                [/[<>]/, "operator.comparison"],
                [/=/, "operator.assignment"],
                [/!/, "operator.logical"],
                [/~/, "operator.bitwise"],
                [/[+\-*/%^]/, "operator.arithmetic"],
                [/\?/, "operator.conditional"], // Ternary begin
                [/\|/, "operator.conditional"], // Ternary separator
                [/:/, "keyword.operator"], // Verb call
                [/\./, "operator.accessor"], // Property access
                [/@/, "keyword.operator"], // Scatter/splat operator

                // Comments
                [/\/\*/, "comment", "@comment"],
                [/\/\/.*$/, "comment"],

                // Identifiers
                [/[a-zA-Z_][a-zA-Z0-9_]*/, "identifier"],
            ],

            string: [
                [/[^\\"]+/, "string"],
                [/\\./, "string.escape"],
                [/"/, "string", "@pop"],
            ],

            comment: [
                [/[^/*]+/, "comment"],
                [/\*\//, "comment", "@pop"],
                [/[/*]/, "comment"],
            ],

            tryExpression: [
                [/'(?![a-zA-Z_])/, { token: "keyword.try", next: "@pop", bracket: "@close" }],
                [/=>/, "keyword.operator"],
                [/!/, "keyword.operator"],
                { include: "@root" },
            ],
        },
    });

    // Define MOO language configuration
    monaco.languages.setLanguageConfiguration("moo", {
        comments: {
            lineComment: "//",
            blockComment: ["/*", "*/"],
        },
        brackets: [
            ["{", "}"], // Lists and blocks
            ["[", "]"], // Maps and indexing
            ["(", ")"], // Function calls and grouping
            ["<", ">"], // Flyweights
        ],
        autoClosingPairs: [
            { open: "{", close: "}" },
            { open: "[", close: "]" },
            { open: "(", close: ")" },
            { open: "<", close: ">" },
            { open: "\"", close: "\"" },
            { open: "`", close: "'" }, // Try expressions
        ],
        surroundingPairs: [
            { open: "{", close: "}" },
            { open: "[", close: "]" },
            { open: "(", close: ")" },
            { open: "<", close: ">" },
            { open: "\"", close: "\"" },
        ],
    });

    isRegistered = true;
}
