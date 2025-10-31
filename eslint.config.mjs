import { FlatCompat } from "@eslint/eslintrc";
import js from "@eslint/js";
import typescriptEslint from "@typescript-eslint/eslint-plugin";
import tsParser from "@typescript-eslint/parser";
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig, globalIgnores } from "eslint/config";
import globals from "globals";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const compat = new FlatCompat({
    baseDirectory: __dirname,
    recommendedConfig: js.configs.recommended,
    allConfig: js.configs.all,
});

export default defineConfig([
    globalIgnores(["**/dist/", "**/node_modules/", "**/crates/", "**/target/", "**/generated/", "**/public/"]),
    {
        extends: compat.extends(
            "eslint:recommended",
            "plugin:@typescript-eslint/recommended",
        ),

        plugins: {
            "@typescript-eslint": typescriptEslint,
            "react-hooks": reactHooks,
        },

        languageOptions: {
            globals: {
                ...globals.browser,
                ...globals.es2020,
            },

            parser: tsParser,
            ecmaVersion: "latest",
            sourceType: "module",
        },

        rules: {
            // Note: you must disable the base rule as it can report incorrect errors
            "no-unused-vars": "off",
            "@typescript-eslint/no-unused-vars": [
                "error",
                {
                    argsIgnorePattern: "^_",
                    caughtErrors: "none",
                },
            ],
            "@typescript-eslint/no-explicit-any": "warn",
            "prefer-const": "error",
            "no-var": "error",
            "react-hooks/rules-of-hooks": "error",
            "react-hooks/exhaustive-deps": "warn",
        },
    },
]);
