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

declare module "node:fs" {
    export function readFileSync(path: string, encoding: string): string;
}

declare module "node:path" {
    export function resolve(path: string): string;
}

declare module "node:readline" {
    export interface Interface {
        on(event: "line", cb: (line: string) => void | Promise<void>): this;
        close(): void;
    }
    export function createInterface(options: {
        input: any;
        crlfDelay?: number;
    }): Interface;
    const _default: { createInterface: typeof createInterface };
    export default _default;
}

declare module "yaml" {
    export function parse(src: string): unknown;
    const _default: { parse: typeof parse };
    export default _default;
}

declare module "ws" {
    export default class WebSocket {
        static OPEN: number;
        static CLOSED: number;
        readyState: number;
        constructor(url: string, protocols?: string[]);
        on(event: "message", cb: (data: any) => void): this;
        on(event: "error", cb: (error: Error) => void): this;
        on(event: "close", cb: () => void): this;
        once(event: "open", cb: () => void): this;
        once(event: "error", cb: (error: Error) => void): this;
        off(event: "message", cb: (data: any) => void): this;
        off(event: "open", cb: () => void): this;
        off(event: "error", cb: (error: Error) => void): this;
        off(event: "close", cb: () => void): this;
        send(data: string | Uint8Array | ArrayBuffer): void;
        close(): void;
    }
}

declare module "@moor/web-sdk" {
    export function buildWsAttach(
        baseWsUrl: string,
        options: {
            mode: "connect" | "create";
            credentials: {
                authToken: string;
                clientId?: string | null;
                clientToken?: string | null;
                isInitialAttach?: boolean;
            };
        },
    ): { wsUrl: string; protocols: string[] };

    export function dispatchClientEvent(
        bytes: Uint8Array,
        handlers: {
            onNarrativeEventMessage?: (narrative: unknown) => void;
            onTaskErrorEvent?: (event: { error(): unknown }) => void;
            onTaskSuccessEvent?: () => void;
        },
    ): void;

    export function parseWsNarrativeEventMessage(
        narrative: unknown,
        decodeVarToJs: (value: unknown) => unknown,
        decodeVarToString: (value: unknown) => string | null,
    ): { kind: "notify"; content: unknown } | null;

    export function schedulerErrorToNarrative(error: unknown): { message: string } | null;
    export function parseEvalResultVar(bytes: Uint8Array): unknown;

    export class MoorVar {
        constructor(value: unknown);
        toJS(): unknown;
        toLiteral(): string;
        asString(): string | null;
    }
}

declare const process: any;
