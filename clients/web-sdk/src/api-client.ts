// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Lesser General Public License as published by the Free Software Foundation,
// version 3 or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Lesser General Public License for more
// details.
//
// You should have received a copy of the GNU Lesser General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export type MoorApiErrorKind = "transport" | "decode" | "protocol";

export class MoorApiError extends Error {
    kind: MoorApiErrorKind;
    status?: number;
    statusText?: string;
    context?: string;
    cause?: unknown;

    constructor(kind: MoorApiErrorKind, message: string, options?: {
        status?: number;
        statusText?: string;
        context?: string;
        cause?: unknown;
    }) {
        super(message);
        this.name = "MoorApiError";
        this.kind = kind;
        this.status = options?.status;
        this.statusText = options?.statusText;
        this.context = options?.context;
        this.cause = options?.cause;
    }
}

export interface MoorApiClientOptions {
    fetcher?: FetchLike;
    baseUrl?: string;
}

export interface MoorApiClient {
    request(path: string, init?: RequestInit): Promise<Response>;
    getFlatBuffer(path: string, init?: RequestInit): Promise<Uint8Array>;
    getFlatBufferOrNullOn404(path: string, init?: RequestInit): Promise<Uint8Array | null>;
}

function trimTrailingSlash(value: string): string {
    return value.endsWith("/") ? value.slice(0, -1) : value;
}

function resolvePath(path: string, baseUrl?: string): string {
    if (path.startsWith("http://") || path.startsWith("https://")) {
        return path;
    }
    if (!baseUrl) {
        return path;
    }
    const base = trimTrailingSlash(baseUrl);
    if (path.startsWith("/")) {
        return `${base}${path}`;
    }
    return `${base}/${path}`;
}

async function toBytes(response: Response): Promise<Uint8Array> {
    try {
        const arrayBuffer = await response.arrayBuffer();
        return new Uint8Array(arrayBuffer);
    } catch (cause) {
        throw new MoorApiError(
            "decode",
            `Failed to decode response body: ${response.status} ${response.statusText}`,
            { status: response.status, statusText: response.statusText, cause },
        );
    }
}

export function createMoorApiClient(options: MoorApiClientOptions = {}): MoorApiClient {
    const fetcher: FetchLike = options.fetcher ?? window.fetch.bind(window);
    const baseUrl = options.baseUrl;

    async function request(path: string, init?: RequestInit): Promise<Response> {
        const response = await fetcher(resolvePath(path, baseUrl), init);
        return response;
    }

    async function getFlatBuffer(path: string, init?: RequestInit): Promise<Uint8Array> {
        const response = await request(path, init);
        if (!response.ok) {
            throw new MoorApiError(
                "transport",
                `Request failed: ${response.status} ${response.statusText}`,
                { status: response.status, statusText: response.statusText, context: path },
            );
        }
        return toBytes(response);
    }

    async function getFlatBufferOrNullOn404(path: string, init?: RequestInit): Promise<Uint8Array | null> {
        const response = await request(path, init);
        if (response.status === 404) {
            return null;
        }
        if (!response.ok) {
            throw new MoorApiError(
                "transport",
                `Request failed: ${response.status} ${response.statusText}`,
                { status: response.status, statusText: response.statusText, context: path },
            );
        }
        return toBytes(response);
    }

    return {
        request,
        getFlatBuffer,
        getFlatBufferOrNullOn404,
    };
}
