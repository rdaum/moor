// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

import type { Tool } from "@modelcontextprotocol/server";

export function withCharacter(schema: Record<string, unknown>): Tool["inputSchema"] {
    const source = schema as {
        properties?: Tool["inputSchema"]["properties"];
        required?: string[];
    };
    const properties = { ...(source.properties ?? {}) };
    properties.moo = {
        type: "string",
        description: "Configured moo id. Defaults to config.defaultMoo.",
    };
    properties.character = {
        type: "string",
        description: "Configured player id inside the selected moo.",
    };
    properties.wizard = {
        type: "boolean",
        description: "Use a player marked isWizard=true in the selected moo.",
        default: false,
    };
    return {
        additionalProperties: false,
        ...source,
        type: "object",
        properties,
    };
}

export const staticTools: Tool[] = [
    {
        name: "moo_eval",
        description: "Evaluate MOO code and return result.",
        inputSchema: withCharacter({
            type: "object",
            properties: {
                expression: { type: "string" },
            },
            required: ["expression"],
        }),
    },
    {
        name: "moo_command",
        description: "Execute a command through the websocket session and return captured narrative output.",
        inputSchema: withCharacter({
            type: "object",
            properties: {
                command: { type: "string" },
                include_metadata: {
                    type: "boolean",
                    description: "Include parsed websocket event metadata JSON in the response.",
                    default: false,
                },
            },
            required: ["command"],
        }),
    },
    {
        name: "moo_function_help",
        description: "Get documentation for a MOO builtin function.",
        inputSchema: withCharacter({
            type: "object",
            properties: {
                function_name: { type: "string" },
            },
            required: ["function_name"],
        }),
    },
    {
        name: "moo_invoke_verb",
        description: "Invoke an object verb using eval-based call semantics.",
        inputSchema: withCharacter({
            type: "object",
            properties: {
                object: { type: "string" },
                verb: { type: "string" },
                args: { type: "array", items: {} },
            },
            required: ["object", "verb"],
        }),
    },
    {
        name: "moo_list_objects",
        description: "List objects visible to the selected character.",
        inputSchema: withCharacter({
            type: "object",
            properties: {},
        }),
    },
    {
        name: "moo_resolve",
        description: "Resolve an object reference.",
        inputSchema: withCharacter({
            type: "object",
            properties: {
                object: { type: "string" },
            },
            required: ["object"],
        }),
    },
    {
        name: "moo_list_verbs",
        description: "List verbs on an object.",
        inputSchema: withCharacter({
            type: "object",
            properties: {
                object: { type: "string" },
                inherited: { type: "boolean" },
            },
            required: ["object"],
        }),
    },
    {
        name: "moo_get_verb",
        description: "Get a specific verb.",
        inputSchema: withCharacter({
            type: "object",
            properties: {
                object: { type: "string" },
                name: { type: "string" },
            },
            required: ["object", "name"],
        }),
    },
    {
        name: "moo_program_verb",
        description: "Program verb source code.",
        inputSchema: withCharacter({
            type: "object",
            properties: {
                object: { type: "string" },
                name: { type: "string" },
                code: { type: "string" },
            },
            required: ["object", "name", "code"],
        }),
    },
    {
        name: "moo_list_properties",
        description: "List properties on an object.",
        inputSchema: withCharacter({
            type: "object",
            properties: {
                object: { type: "string" },
                inherited: { type: "boolean" },
            },
            required: ["object"],
        }),
    },
    {
        name: "moo_get_property",
        description: "Get one property value.",
        inputSchema: withCharacter({
            type: "object",
            properties: {
                object: { type: "string" },
                name: { type: "string" },
            },
            required: ["object", "name"],
        }),
    },
    {
        name: "moo_set_property",
        description: "Set property value using MOO literal syntax.",
        inputSchema: withCharacter({
            type: "object",
            properties: {
                object: { type: "string" },
                name: { type: "string" },
                valueLiteral: { type: "string" },
            },
            required: ["object", "name", "valueLiteral"],
        }),
    },
    {
        name: "moo_refresh_dynamic_tools",
        description: "Refresh dynamic tools from #0:external_agent_tools().",
        inputSchema: withCharacter({
            type: "object",
            properties: {},
        }),
    },
];
