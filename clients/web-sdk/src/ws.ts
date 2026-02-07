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

import type { WsAttachMessage, WsAttachOptions } from "./types";

export function buildWsAttach(baseWsUrl: string, options: WsAttachOptions): WsAttachMessage {
    const wsUrl = buildWsAttachUrl(baseWsUrl, options.mode);

    const protocols: string[] = ["moor", `paseto.${options.credentials.authToken}`];

    if (options.credentials.isInitialAttach) {
        protocols.push("initial_attach.true");
    }

    if (options.credentials.clientId && options.credentials.clientToken) {
        protocols.push(`client_id.${options.credentials.clientId}`);
        protocols.push(`client_token.${options.credentials.clientToken}`);
    }

    return {
        wsUrl,
        protocols,
    };
}

export function buildWsAttachUrl(baseWsUrl: string, mode: "connect" | "create"): string {
    const normalized = baseWsUrl.endsWith("/") ? baseWsUrl.slice(0, -1) : baseWsUrl;
    return `${normalized}/ws/attach/${mode}`;
}
