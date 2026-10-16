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

import {
    type CallToolResult,
    type JsonSchemaType,
    ProtocolError,
    type ReadResourceResult,
    type Resource,
    Server,
    type Tool,
} from "@modelcontextprotocol/server";
import { AjvJsonSchemaValidator } from "@modelcontextprotocol/server/validators/ajv";
import { readFileSync } from "node:fs";
import { type CharacterRef, type MooConfig, resolveDefaultCharacter } from "./config.js";
import { type MoorWebClient } from "./moor-service.js";
import { staticTools, withCharacter } from "./static-tools.js";
import { ToolCatalog } from "./tool-catalog.js";

interface ToolArgs extends Record<string, unknown> {
    moo?: string;
    character?: string;
    wizard?: boolean;
    include_metadata?: boolean;
}

function makeTextResult(text: string): CallToolResult {
    return { content: [{ type: "text", text }] };
}

function makeErrorResult(text: string): CallToolResult {
    return { content: [{ type: "text", text }], isError: true };
}

function parseObjectRef(raw: unknown): string {
    if (typeof raw !== "string") {
        throw new Error("object must be a string like '#123' or 'uuid:ABCDEF-1234567890'");
    }
    return raw;
}

function simpleToCurie(value: string): string | null {
    if (/^(oid:-?\d+|uuid:[0-9a-f]{6}-[0-9a-f]{10})$/i.test(value)) {
        return value;
    }
    if (value.startsWith("#")) {
        const raw = value.slice(1);
        const upper = raw.toUpperCase();
        if (/^[0-9A-F]{6}-[0-9A-F]{10}$/.test(upper)) {
            return `uuid:${upper}`;
        }
        return /^-?\d+$/.test(raw) ? `oid:${raw}` : null;
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

function isSystemObjectRef(value: string): boolean {
    return /^\$[A-Za-z_][A-Za-z0-9_]*$/.test(value);
}

export class McpServer extends Server {
    private readonly catalog: ToolCatalog;
    private readonly validator = new AjvJsonSchemaValidator();

    constructor(
        private readonly moor: MoorWebClient,
        private readonly moos: MooConfig[],
        private readonly defaultCharacter: CharacterRef,
    ) {
        const { version } = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
        super({ name: "moor-web-mcp", version }, { capabilities: { tools: {}, resources: {} } });
        this.catalog = new ToolCatalog(moor);
        this.setRequestHandler("tools/list", async () => ({ tools: await this.listTools() }));
        this.setRequestHandler("tools/call", async (request, ctx) => this.callTool(request.params, ctx.mcpReq.signal));
        this.setRequestHandler("resources/list", async () => ({ resources: await this.listResources() }));
        this.setRequestHandler("resources/read", async request => this.readResource(request.params));
    }

    private characters(): CharacterRef[] {
        return this.moos.map(moo =>
            moo.id === this.defaultCharacter.mooId
                ? this.defaultCharacter
                : resolveDefaultCharacter({ moos: [moo] })
        );
    }

    private async listTools(): Promise<Tool[]> {
        const tools = new Map<string, Tool>();
        for (const character of this.characters()) {
            for (const tool of await this.catalog.tools(character)) {
                if (staticTools.some(entry => entry.name === tool.name)) {
                    throw new Error(`Reserved tool name: ${tool.name}`);
                }
                const schema = withCharacter(tool.inputSchema);
                schema.properties = { ...schema.properties, moo: { type: "string", const: character.mooId } };
                if (character.mooId !== this.defaultCharacter.mooId) {
                    schema.required = [...(schema.required ?? []), "moo"];
                }
                const previous = tools.get(tool.name);
                if (previous) (previous.inputSchema.anyOf as unknown[]).push(schema);
                else {tools.set(tool.name, {
                        name: tool.name,
                        description: tool.description,
                        inputSchema: { type: "object", anyOf: [schema] },
                    });}
            }
        }
        return [...staticTools, ...tools.values()];
    }

    private async callTool(
        params: { name: string; arguments?: Record<string, unknown> },
        signal?: AbortSignal,
    ): Promise<CallToolResult> {
        const { name } = params;
        try {
            const argumentsObj = params.arguments ?? {};
            const character = this.resolveCharacter(argumentsObj);
            const builtIn = staticTools.find(tool => tool.name === name);
            const dynamic = builtIn
                ? undefined
                : (await this.catalog.tools(character)).find(tool => tool.name === name);
            if (!builtIn && !dynamic) throw new ProtocolError(-32602, `Unknown tool: ${name}`);
            const schema = builtIn?.inputSchema ?? withCharacter(dynamic!.inputSchema);
            const checked = this.validator.getValidator(schema as JsonSchemaType)(argumentsObj);
            if (!checked.valid) {
                const content = { ok: false, error: { code: "invalid_request", message: checked.errorMessage } };
                return { ...makeErrorResult(JSON.stringify(content)), structuredContent: content };
            }
            return await this.executeTool(name, argumentsObj, character, signal);
        } catch (error) {
            if (error instanceof ProtocolError) throw error;
            return makeErrorResult(error instanceof Error ? error.message : String(error));
        }
    }

    private async executeTool(
        name: string,
        argumentsObj: ToolArgs,
        character: CharacterRef,
        signal?: AbortSignal,
    ): Promise<CallToolResult> {
        if (name === "moo_eval") {
            const expression = argumentsObj.expression;
            if (typeof expression !== "string") {
                return makeErrorResult("expression is required");
            }
            const result = await this.moor.evalExpression(character, expression, signal);
            return makeTextResult(`${result.literal}\n\n${JSON.stringify(result.js, null, 2)}`);
        }

        if (name === "moo_command") {
            const command = argumentsObj.command;
            if (typeof command !== "string") {
                return makeErrorResult("command is required");
            }
            const includeMetadata = argumentsObj.include_metadata === true;
            const result = await this.moor.command(character, command, signal);
            const output = includeMetadata
                ? `${result.lines.join("\n")}\n\n--- events ---\n${JSON.stringify(result.events, null, 2)}`
                : result.lines.join("\n");
            return result.error
                ? makeErrorResult(output)
                : makeTextResult(output);
        }

        if (name === "moo_invoke_verb") {
            const object = await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object), signal);
            const verb = argumentsObj.verb;
            const args = Array.isArray(argumentsObj.args) ? argumentsObj.args : [];
            if (typeof verb !== "string") {
                return makeErrorResult("verb is required");
            }
            const result = await this.moor.invokeVerbViaEval(character, object, verb, args, signal);
            return makeTextResult(`${result.literal}\n\n${JSON.stringify(result.js, null, 2)}`);
        }

        if (name === "moo_function_help") {
            const functionName = argumentsObj.function_name;
            if (typeof functionName !== "string") {
                return makeErrorResult("function_name is required");
            }
            const result = await this.moor.functionHelp(character, functionName, signal);
            return makeTextResult(`${result.literal}\n\n${JSON.stringify(result.js, null, 2)}`);
        }

        if (name === "moo_list_objects") {
            const result = await this.moor.requestJson(character, "/v1/objects", undefined, signal);
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_resolve") {
            const object = encodeURIComponent(
                await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object), signal),
            );
            const result = await this.moor.requestJson(character, `/v1/objects/${object}`, undefined, signal);
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_list_verbs") {
            const object = encodeURIComponent(
                await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object), signal),
            );
            const inherited = argumentsObj.inherited === true ? "?inherited=true" : "";
            const result = await this.moor.requestJson(character, `/v1/verbs/${object}${inherited}`, undefined, signal);
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_get_verb") {
            const object = encodeURIComponent(
                await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object), signal),
            );
            const verbName = argumentsObj.name;
            if (typeof verbName !== "string") {
                return makeErrorResult("name is required");
            }
            const result = await this.moor.requestJson(
                character,
                `/v1/verbs/${object}/${encodeURIComponent(verbName)}`,
                undefined,
                signal,
            );
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_program_verb") {
            const object = encodeURIComponent(
                await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object), signal),
            );
            const verbName = argumentsObj.name;
            const code = argumentsObj.code;
            if (typeof verbName !== "string" || typeof code !== "string") {
                return makeErrorResult("name and code are required");
            }
            const result = await this.moor.requestJson(
                character,
                `/v1/verbs/${object}/${encodeURIComponent(verbName)}`,
                {
                    method: "POST",
                    headers: { "Content-Type": "text/plain" },
                    body: code,
                },
                signal,
            );
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_list_properties") {
            const object = encodeURIComponent(
                await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object), signal),
            );
            const inherited = argumentsObj.inherited === true ? "?inherited=true" : "";
            const result = await this.moor.requestJson(
                character,
                `/v1/properties/${object}${inherited}`,
                undefined,
                signal,
            );
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_get_property") {
            const object = await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object), signal);
            const result = await this.moor.propertyValue(character, object, argumentsObj.name as string, signal);
            return makeTextResult(`${result.literal}\n\n${JSON.stringify(result.js, null, 2)}`);
        }

        if (name === "moo_set_property") {
            const object = encodeURIComponent(
                await this.normalizeObjectRef(character, parseObjectRef(argumentsObj.object), signal),
            );
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
                signal,
            );
            return makeTextResult(JSON.stringify(result, null, 2));
        }

        if (name === "moo_refresh_dynamic_tools") {
            const count = await this.catalog.refresh(character);
            return makeTextResult(`Loaded ${count} dynamic tools`);
        }

        const dynamic = (await this.catalog.tools(character)).find((tool) => tool.name === name);
        if (dynamic) {
            const input = { ...argumentsObj };
            delete input.moo;
            delete input.character;
            delete input.wizard;
            const dynamicResult = await this.moor.executeDynamicTool(character, dynamic, input, signal);
            const structured = dynamicResult.js !== null && typeof dynamicResult.js === "object"
                    && !Array.isArray(dynamicResult.js)
                ? dynamicResult.js as Record<string, unknown>
                : { value: dynamicResult.js };
            return {
                content: [{ type: "text", text: JSON.stringify(structured) }],
                structuredContent: structured,
                isError: structured.ok === false,
            };
        }

        return makeErrorResult(`Unknown tool: ${name}`);
    }

    private async listResources(): Promise<Resource[]> {
        const eventResources = this.moos.flatMap((moo) => {
            return moo.players.map((player) => ({
                uri: `moor://events/${encodeURIComponent(moo.id)}/${encodeURIComponent(player.id)}`,
                name: `Recent events for ${moo.id}/${player.id}`,
                description: "Recent websocket narrative events captured by the MCP process.",
                mimeType: "application/json",
            }));
        });
        return [
            {
                uri: "moor://moos",
                name: "MOOs",
                description: "Configured MOOs with descriptions and player metadata.",
                mimeType: "application/json",
            },
            ...eventResources,
            ...await this.catalog.listResources(this.characters(), this.defaultCharacter),
        ];
    }

    private async readResource(params: Record<string, unknown>): Promise<ReadResourceResult> {
        const uri = params.uri;
        if (typeof uri !== "string") {
            throw new Error("resources/read params.uri must be string");
        }
        if (uri === "moor://moos") {
            return {
                contents: [{
                    uri,
                    mimeType: "application/json",
                    text: JSON.stringify(
                        this.moos.map(({ players, ...moo }) => ({
                            ...moo,
                            players: players.map(({ password: _password, ...player }) => player),
                        })),
                        null,
                        2,
                    ),
                }],
            };
        }

        const match = /^moor:\/\/events\/([^/]+)\/([^/]+)$/.exec(uri);
        if (!match) {
            const dynamic = await this.catalog.readResource(uri, this.defaultCharacter);
            if (dynamic) return { contents: [{ uri, mimeType: dynamic.mimeType, text: dynamic.content }] };
            throw new ProtocolError(-32602, `Unknown resource URI: ${uri}`);
        }
        const character: CharacterRef = {
            mooId: decodeURIComponent(match[1]),
            playerId: decodeURIComponent(match[2]),
        };
        this.moor.findCharacter(character);
        const events = this.moor.getRecentEvents(character);
        return {
            contents: [{
                uri,
                mimeType: "application/json",
                text: JSON.stringify(
                    {
                        mooId: character.mooId,
                        playerId: character.playerId,
                        events,
                    },
                    null,
                    2,
                ),
            }],
        };
    }

    private resolveCharacter(args: ToolArgs): CharacterRef {
        const moo = this.resolveMoo(args);

        if (typeof args.character === "string") {
            const byId = moo.players.find((player) => player.id === args.character);
            if (!byId) {
                throw new Error(`Unknown player id ${args.character} in moo ${moo.id}`);
            }
            return {
                mooId: moo.id,
                playerId: byId.id,
            };
        }
        if (args.wizard) {
            const wizard = moo.players.find((player) => player.isWizard);
            if (wizard) {
                return {
                    mooId: moo.id,
                    playerId: wizard.id,
                };
            }
            throw new Error(`No wizard account configured for ${moo.id}`);
        }

        if (moo.id === this.defaultCharacter.mooId) {
            return this.defaultCharacter;
        }
        return this.defaultCharacterForMoo(moo);
    }

    private resolveMoo(args: ToolArgs): MooConfig {
        if (typeof args.moo === "string") {
            const byId = this.moos.find((moo) => moo.id === args.moo);
            if (!byId) {
                throw new Error(`Unknown moo id: ${args.moo}`);
            }
            return byId;
        }

        const byDefault = this.moos.find((moo) => moo.id === this.defaultCharacter.mooId);
        if (!byDefault) {
            throw new Error(`Unknown default moo id: ${this.defaultCharacter.mooId}`);
        }
        return byDefault;
    }

    private defaultCharacterForMoo(moo: MooConfig): CharacterRef {
        return resolveDefaultCharacter({ moos: [moo] });
    }

    private async normalizeObjectRef(
        character: CharacterRef,
        objectRef: string,
        signal?: AbortSignal,
    ): Promise<string> {
        const quick = simpleToCurie(objectRef);
        if (quick) {
            return quick;
        }

        if (isSystemObjectRef(objectRef)) {
            const systemObj = await this.moor.evalExpression(character, `tostr(${objectRef})`, signal);
            const systemCurie = toCurieFromMooString(systemObj.js);
            if (systemCurie) {
                return systemCurie;
            }
        }

        if (objectRef === "me" || objectRef === "player") {
            const strPlayer = await this.moor.evalExpression(character, "tostr(player)", signal);
            const playerCurie = toCurieFromMooString(strPlayer.js);
            if (playerCurie) {
                return playerCurie;
            }
            const player = await this.moor.evalExpression(character, "player", signal);
            const fromEval = jsObjToCurie(player.js);
            if (fromEval) {
                return fromEval;
            }
        }

        if (objectRef === "here" || objectRef === "location") {
            const strLocation = await this.moor.evalExpression(character, "tostr(player.location)", signal);
            const locationCurie = toCurieFromMooString(strLocation.js);
            if (locationCurie) {
                return locationCurie;
            }
            const location = await this.moor.evalExpression(character, "player.location", signal);
            const fromEval = jsObjToCurie(location.js);
            if (fromEval) {
                return fromEval;
            }
        }

        throw new Error(`Invalid object reference: ${objectRef}`);
    }
}
