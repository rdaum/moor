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

import type { CharacterConfig } from "./config.js";
import type { DynamicTool, MoorWebClient } from "./moor-service.js";
import type {
    JsonRpcError,
    JsonRpcNotification,
    JsonRpcRequest,
    RequestId,
    ResourceDefinition,
    ToolCallResult,
    ToolDefinition,
} from "./mcp-types.js";

const JSONRPC_VERSION = "2.0";

interface ToolArgs extends Record<string, unknown> {
    character?: string;
    wizard?: boolean;
    include_metadata?: boolean;
}

function makeTextResult(text: string): ToolCallResult {
    return { content: [{ type: "text", text }] };
}

function makeErrorResult(text: string): ToolCallResult {
    return { content: [{ type: "text", text }], isError: true };
}

function parseObjectRef(raw: unknown): string {
    if (typeof raw !== "string") {
        throw new Error("object must be a string like '#123' or 'uuid:ABCDEF-1234567890'");
    }
    return raw;
}

function simpleToCurie(value: string): string | null {
    if (value.startsWith("oid:") || value.startsWith("uuid:")) {
        return value;
    }
    if (value.startsWith("#")) {
        return `oid:${value.slice(1)}`;
    }
    if (/^[0-9A-Fa-f]{6}-[0-9A-Fa-f]{10}$/.test(value)) {
        return `uuid:${value.toUpperCase()}`;
    }
    return null;
}

function toCurieFromMooString(value: unknown): string | null {
    if (typeof value !== "string") {
        return null;
    }
    const trimmed = value.trim();
    if (trimmed.startsWith("#")) {
        return `oid:${trimmed.slice(1)}`;
    }
    return simpleToCurie(trimmed);
}

function jsObjToCurie(value: unknown): string | null {
    if (!value || typeof value !== "object") {
        return null;
    }
    const obj = value as { oid?: number; uuid?: string };
    if (typeof obj.oid === "number") {
        return `oid:${obj.oid}`;
    }
    if (typeof obj.uuid === "string") {
        return `uuid:${obj.uuid.toUpperCase()}`;
    }
    return null;
}

export class McpServer {
    private readonly moor: MoorWebClient;
    private readonly characters: CharacterConfig[];
    private readonly defaultCharacterId: string;
    private dynamicTools: DynamicTool[] = [];
    private dynamicLoaded = false;

    constructor(moor: MoorWebClient, characters: CharacterConfig[], defaultCharacterId: string) {
        this.moor = moor;
        this.characters = characters;
        this.defaultCharacterId = defaultCharacterId;
    }

    async handleLine(line: string, write: (line: string) => void): Promise<void> {
        if (!line.trim()) {
            return;
        }

        let message: JsonRpcRequest | JsonRpcNotification;
        try {
            message = JSON.parse(line);
        } catch {
            return;
        }

        if (!("id" in message)) {
            return;
        }

        try {
            const result = await this.dispatch(message);
            write(JSON.stringify({
                jsonrpc: JSONRPC_VERSION,
                id: message.id,
                result,
            }));
        } catch (error) {
            const err = this.toJsonRpcError(error);
            write(JSON.stringify({
                jsonrpc: JSONRPC_VERSION,
                id: message.id,
                error: err,
            }));
        }
    }

    private async dispatch(request: JsonRpcRequest): Promise<unknown> {
        switch (request.method) {
            case "initialize":
                return {
                    protocolVersion: "2024-11-05",
                    capabilities: {
                        tools: {},
                        resources: {},
                    },
                    serverInfo: {
                        name: "moor-web-mcp",
                        version: "0.1.0",
                    },
                };
            case "tools/list":
                return { tools: await this.listTools() };
            case "tools/call":
                return await this.callTool(request.params as Record<string, unknown>);
            case "resources/list":
                return { resources: this.listResources() };
            case "resources/read":
                return this.readResource(request.params as Record<string, unknown>);
            default:
                throw this.methodNotFound(request.method);
        }
    }

    private async listTools(): Promise<ToolDefinition[]> {
        if (!this.dynamicLoaded) {
            await this.refreshDynamicTools();
        }

        const staticTools: ToolDefinition[] = [
            {
                name: "moo_eval",
                description: "Evaluate MOO code and return result.",
                inputSchema: this.withCharacter({
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
                inputSchema: this.withCharacter({
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
                inputSchema: this.withCharacter({
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
                inputSchema: this.withCharacter({
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
                inputSchema: this.withCharacter({
                    type: "object",
                    properties: {},
                }),
            },
            {
                name: "moo_resolve",
                description: "Resolve an object reference.",
                inputSchema: this.withCharacter({
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
                inputSchema: this.withCharacter({
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
                inputSchema: this.withCharacter({
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
                inputSchema: this.withCharacter({
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
                inputSchema: this.withCharacter({
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
                inputSchema: this.withCharacter({
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
                inputSchema: this.withCharacter({
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
                inputSchema: this.withCharacter({
                    type: "object",
                    properties: {},
                }),
            },
        ];

        const dynamic = this.dynamicTools.map((tool) => ({
            name: tool.name,
            description: tool.description,
            inputSchema: this.withCharacter(tool.inputSchema),
        }));

        return [...staticTools, ...dynamic];
    }

    private async callTool(params: Record<string, unknown>): Promise<ToolCallResult> {
        const name = params.name;
        if (typeof name !== "string") {
            throw new Error("tools/call params.name must be a string");
        }
        const argumentsObj = (params.arguments ?? {}) as ToolArgs;
        const character = this.resolveCharacter(argumentsObj);

        if (name === "moo_eval") {
            const expression = argumentsObj.expression;
            if (typeof expression !== "string") {
                return makeErrorResult("expression is required");
            }
            const result = await this.moor.evalExpression(character, expression);
            return makeTextResult(`${result.literal}\n\n${JSON.stringify(result.js, null, 2)}`);
        }

        if (name === "moo_command") {
            const command = argumentsObj.command;
            if (typeof command !== "string") {
                return makeErrorResult("command is required");
            }
            const includeMetadata = argumentsObj.include_metadata === true;
            const result = await this.moor.command(character, command);
            const output = includeMetadata
                ? `${result.lines.join("\n")}\n\n--- events ---\n${JSON.stringify(result.events, null, 2)}`
                : result.lines.join("\n");
            return result.error
                ? makeErrorResult(output)
                : makeTextResult(output);
        }

        if (name === "moo_invoke_verb") {
            const object = await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object));
            const verb = argumentsObj.verb;
            const args = Array.isArray(argumentsObj.args) ? argumentsObj.args : [];
            if (typeof verb !== "string") {
                return makeErrorResult("verb is required");
            }
            const result = await this.moor.invokeVerbViaEval(character, object, verb, args);
            return makeTextResult(`${result.literal}\n\n${JSON.stringify(result.js, null, 2)}`);
        }

        if (name === "moo_function_help") {
            const functionName = argumentsObj.function_name;
            if (typeof functionName !== "string") {
                return makeErrorResult("function_name is required");
            }
            const result = await this.moor.functionHelp(character, functionName);
            return makeTextResult(`${result.literal}\n\n${JSON.stringify(result.js, null, 2)}`);
        }

        if (name === "moo_list_objects") {
            const result = await this.moor.requestJson(character, "/v1/objects");
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_resolve") {
            const object = encodeURIComponent(await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object)));
            const result = await this.moor.requestJson(character, `/v1/objects/${object}`);
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_list_verbs") {
            const object = encodeURIComponent(await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object)));
            const inherited = argumentsObj.inherited === true ? "?inherited=true" : "";
            const result = await this.moor.requestJson(character, `/v1/verbs/${object}${inherited}`);
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_get_verb") {
            const object = encodeURIComponent(await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object)));
            const verbName = argumentsObj.name;
            if (typeof verbName !== "string") {
                return makeErrorResult("name is required");
            }
            const result = await this.moor.requestJson(character, `/v1/verbs/${object}/${encodeURIComponent(verbName)}`);
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_program_verb") {
            const object = encodeURIComponent(await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object)));
            const verbName = argumentsObj.name;
            const code = argumentsObj.code;
            if (typeof verbName !== "string" || typeof code !== "string") {
                return makeErrorResult("name and code are required");
            }
            const result = await this.moor.requestJson(character, `/v1/verbs/${object}/${encodeURIComponent(verbName)}`, {
                method: "POST",
                headers: { "Content-Type": "text/plain" },
                body: code,
            });
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_list_properties") {
            const object = encodeURIComponent(await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object)));
            const inherited = argumentsObj.inherited === true ? "?inherited=true" : "";
            const result = await this.moor.requestJson(character, `/v1/properties/${object}${inherited}`);
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_get_property") {
            const object = encodeURIComponent(await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object)));
            const propertyName = argumentsObj.name;
            if (typeof propertyName !== "string") {
                return makeErrorResult("name is required");
            }
            const result = await this.moor.requestJson(
                character,
                `/v1/properties/${object}/${encodeURIComponent(propertyName)}`,
            );
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_set_property") {
            const object = encodeURIComponent(await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object)));
            const propertyName = argumentsObj.name;
            const valueLiteral = argumentsObj.valueLiteral;
            if (typeof propertyName !== "string" || typeof valueLiteral !== "string") {
                return makeErrorResult("name and valueLiteral are required");
            }
            const result = await this.moor.requestJson(
                character,
                `/v1/properties/${object}/${encodeURIComponent(propertyName)}`,
                {
                    method: "POST",
                    headers: { "Content-Type": "text/plain" },
                    body: valueLiteral,
                },
            );
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_refresh_dynamic_tools") {
            const count = await this.refreshDynamicTools(character);
            return makeTextResult(`Loaded ${count} dynamic tools`);
        }

        const dynamic = this.dynamicTools.find((tool) => tool.name === name);
        if (dynamic) {
            const dynamicResult = await this.moor.executeDynamicTool(character, dynamic, argumentsObj);
            return makeTextResult(`${dynamicResult.literal}\n\n${JSON.stringify(dynamicResult.js, null, 2)}`);
        }

        return makeErrorResult(`Unknown tool: ${name}`);
    }

    private listResources(): ResourceDefinition[] {
        return [
            {
                uri: "moor://characters",
                name: "Characters",
                description: "Configured characters and LLM guidance metadata.",
                mimeType: "application/json",
            },
            ...this.characters.map((c) => ({
                uri: `moor://events/${c.id}`,
                name: `Recent events for ${c.id}`,
                description: "Recent websocket narrative events captured by the MCP process.",
                mimeType: "application/json",
            })),
        ];
    }

    private readResource(params: Record<string, unknown>): { contents: Array<Record<string, string>> } {
        const uri = params.uri;
        if (typeof uri !== "string") {
            throw new Error("resources/read params.uri must be string");
        }
        if (uri === "moor://characters") {
            return {
                contents: [{
                    uri,
                    mimeType: "application/json",
                    text: JSON.stringify(this.characters, null, 2),
                }],
            };
        }

        const match = /^moor:\/\/events\/([^/]+)$/.exec(uri);
        if (!match) {
            throw new Error(`Unknown resource URI: ${uri}`);
        }
        const characterId = match[1];
        const events = this.moor.getRecentEvents(characterId);
        return {
            contents: [{
                uri,
                mimeType: "application/json",
                text: JSON.stringify({ characterId, events }, null, 2),
            }],
        };
    }

    private withCharacter(schema: Record<string, unknown>): Record<string, unknown> {
        const source = schema as {
            properties?: Record<string, unknown>;
            required?: string[];
        };
        const properties = { ...(source.properties ?? {}) };
        properties.character = {
            type: "string",
            description: "Configured character id. Defaults to config.defaultCharacter.",
        };
        properties.wizard = {
            type: "boolean",
            description: "Use a character marked isWizard=true.",
            default: false,
        };
        return {
            ...source,
            properties,
        };
    }

    private resolveCharacter(args: ToolArgs): string {
        if (typeof args.character === "string") {
            return args.character;
        }
        if (args.wizard) {
            const wizard = this.characters.find((c) => c.isWizard);
            if (wizard) {
                return wizard.id;
            }
        }
        return this.defaultCharacterId;
    }

    private async normalizeObjectRef(character: string, objectRef: string): Promise<string> {
        const quick = simpleToCurie(objectRef);
        if (quick) {
            return quick;
        }

        if (objectRef === "me" || objectRef === "player") {
            const strPlayer = await this.moor.evalExpression(character, "tostr(player)");
            const playerCurie = toCurieFromMooString(strPlayer.js);
            if (playerCurie) {
                return playerCurie;
            }
            const player = await this.moor.evalExpression(character, "player");
            const fromEval = jsObjToCurie(player.js);
            if (fromEval) {
                return fromEval;
            }
        }

        if (objectRef === "here" || objectRef === "location") {
            const strLocation = await this.moor.evalExpression(character, "tostr(player.location)");
            const locationCurie = toCurieFromMooString(strLocation.js);
            if (locationCurie) {
                return locationCurie;
            }
            const location = await this.moor.evalExpression(character, "player.location");
            const fromEval = jsObjToCurie(location.js);
            if (fromEval) {
                return fromEval;
            }
        }

        return objectRef;
    }

    private async refreshDynamicTools(characterId?: string): Promise<number> {
        const sourceCharacter = characterId ?? this.defaultCharacterId;
        this.dynamicTools = await this.moor.refreshDynamicTools(sourceCharacter);
        this.dynamicLoaded = true;
        return this.dynamicTools.length;
    }

    private methodNotFound(method: string): JsonRpcError {
        return {
            code: -32601,
            message: `Method not found: ${method}`,
        };
    }

    private toJsonRpcError(error: unknown): JsonRpcError {
        if (typeof error === "object" && error !== null && "code" in error && "message" in error) {
            return error as JsonRpcError;
        }
        return {
            code: -32000,
            message: error instanceof Error ? error.message : String(error),
        };
    }
}
