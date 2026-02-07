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

export type ConnectMode = "connect" | "create";

export interface SessionCredentials {
    authToken: string;
    clientToken?: string | null;
    clientId?: string | null;
    isInitialAttach?: boolean;
}

export interface ClientCredentials {
    clientToken?: string | null;
    clientId?: string | null;
}

export interface WsAttachOptions {
    mode: ConnectMode;
    credentials: SessionCredentials;
}

export interface WsAttachMessage {
    wsUrl: string;
    protocols: string[];
}

export interface MoorHttpError {
    status: number;
    statusText: string;
    body?: string;
}

export interface TransportConfig {
    httpBaseUrl?: string;
    wsBaseUrl: string;
    secure?: boolean;
}
