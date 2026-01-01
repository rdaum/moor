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

// Prism.js language definition for MOO
// Based on the Monaco editor MOO language definition

import Prism from "prismjs";

// Define MOO language for Prism.js
Prism.languages.moo = {
    // Comments
    comment: [
        {
            pattern: /\/\*[\s\S]*?\*\//,
            greedy: true,
        },
        {
            pattern: /\/\/.*/,
            greedy: true,
        },
    ],

    // Strings
    string: {
        pattern: /"(?:[^"\\]|\\.)*"/,
        greedy: true,
    },

    // Binary literals
    binary: {
        pattern: /b"[A-Za-z0-9+/=_-]*"/,
        alias: "string",
    },

    // Symbols
    symbol: {
        pattern: /'[a-zA-Z_][a-zA-Z0-9_]*/,
        alias: "string",
    },

    // Control flow keywords
    keyword:
        /\b(?:if|elseif|else|endif|while|endwhile|for|endfor|try|except|endtry|finally|fork|endfork|begin|end|return|break|continue|pass|raise|let|const|global|fn|endfn|any|in)\b/,

    // Built-in constants
    boolean: /\b(?:true|false)\b/,

    // Type constants
    builtin: /\b(?:INT|NUM|FLOAT|STR|ERR|OBJ|LIST|MAP|BOOL|FLYWEIGHT|SYM)\b/,

    // Error constants
    constant: /\bE_[A-Z_]+\b/,

    // Object references (#123, #-1)
    "object-ref": {
        pattern: /#-?\d+/,
        alias: "number",
    },

    // System properties and verbs ($property)
    "system-var": {
        pattern: /\$[a-zA-Z_][a-zA-Z0-9_]*/,
        alias: "variable",
    },

    // Numbers - floats and integers
    number: [
        /\b\d*\.\d+(?:[eE][-+]?\d+)?\b/,
        /\b\d+[eE][-+]?\d+\b/,
        /\b\d+\b/,
    ],

    // Operators - order matters
    operator: [
        /\.\./, // Range operator
        /->/, // Map arrow
        /=>/, // Lambda arrow
        />>>/, // Unsigned right shift
        />>/, // Right shift
        /<</, // Left shift
        /&\./, // Bitwise AND
        /\|\./, // Bitwise OR
        /\^\./, // Bitwise XOR
        /==|!=|<=|>=/, // Comparison
        /&&|\|\|/, // Logical
        /[<>]/, // Comparison
        /!/, // Logical NOT
        /~/, // Bitwise NOT
        /[+\-*/%^]/, // Arithmetic
        /\?/, // Ternary begin
        /\|/, // Ternary separator or verb call
        /:/, // Verb call
        /\./, // Property access
        /@/, // Scatter/splat
        /=/, // Assignment
    ],

    // Punctuation
    punctuation: /[{}[\]();,]/,
    // Identifiers (must be last to avoid conflicts)
    // This will catch anything not matched above
};

// Add aliases for backward compatibility
Prism.languages.MOO = Prism.languages.moo;
