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

import { CredentialsUpdatedEvent } from "@moor/schema/generated/moor-rpc/credentials-updated-event";

export const WS_KEEPALIVE_MARKER = 0x00;
export const WS_HEARTBEAT_RESPONSE_MARKER = 0x01;
export const WS_HEARTBEAT_REQUEST_MARKER = 0x02;

export interface SessionCredentialsUpdate {
    clientId: string;
    clientToken: string;
}

function concatByteArrays(chunks: Uint8Array[]): Uint8Array {
    const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
    const merged = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) {
        merged.set(chunk, offset);
        offset += chunk.length;
    }
    return merged;
}

function uuidBytesToString(bytes: Uint8Array): string | null {
    if (bytes.length !== 16) {
        return null;
    }
    const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
}

export function bytesFromWebSocketMessage(data: unknown): Uint8Array | null {
    if (data instanceof Uint8Array) {
        return data;
    }
    if (data instanceof ArrayBuffer) {
        return new Uint8Array(data);
    }
    if (ArrayBuffer.isView(data)) {
        return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    }
    if (Array.isArray(data)) {
        const chunks: Uint8Array[] = [];
        for (const part of data) {
            const bytes = bytesFromWebSocketMessage(part);
            if (!bytes) {
                return null;
            }
            chunks.push(bytes);
        }
        return concatByteArrays(chunks);
    }
    return null;
}

export function handleWebSocketControlFrame(
    bytes: Uint8Array,
    send: (payload: Uint8Array) => void,
): boolean {
    if (bytes.length !== 1) {
        return false;
    }
    if (bytes[0] === WS_KEEPALIVE_MARKER || bytes[0] === WS_HEARTBEAT_RESPONSE_MARKER) {
        return true;
    }
    if (bytes[0] === WS_HEARTBEAT_REQUEST_MARKER) {
        send(Uint8Array.of(WS_HEARTBEAT_RESPONSE_MARKER));
        return true;
    }
    return false;
}

export function decodeCredentialsUpdatedEvent(
    credentials: CredentialsUpdatedEvent,
): SessionCredentialsUpdate | null {
    const clientIdBytes = credentials.clientId()?.dataArray();
    const clientToken = credentials.clientToken()?.token();
    if (!clientIdBytes || !clientToken) {
        return null;
    }
    const clientId = uuidBytesToString(clientIdBytes);
    if (!clientId) {
        return null;
    }
    return { clientId, clientToken };
}
