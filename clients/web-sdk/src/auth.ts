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

import type { ClientCredentials, SessionCredentials } from "./types";

export function readClientCredentialsFromSessionStorage(): ClientCredentials {
    try {
        return {
            clientToken: sessionStorage.getItem("client_token"),
            clientId: sessionStorage.getItem("client_id"),
        };
    } catch {
        return {};
    }
}

export function buildAuthHeaders(credentials: SessionCredentials | string): Record<string, string> {
    const normalized: SessionCredentials = typeof credentials === "string"
        ? {
            authToken: credentials,
            ...readClientCredentialsFromSessionStorage(),
        }
        : credentials;

    const headers: Record<string, string> = {
        "X-Moor-Auth-Token": normalized.authToken,
    };

    if (normalized.clientToken) {
        headers["X-Moor-Client-Token"] = normalized.clientToken;
    }

    if (normalized.clientId) {
        headers["X-Moor-Client-Id"] = normalized.clientId;
    }

    return headers;
}
