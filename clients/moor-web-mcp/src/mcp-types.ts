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

export type RequestId = string | number;

export interface JsonRpcRequest {
    jsonrpc: "2.0";
    id: RequestId;
    method: string;
    params?: unknown;
}

export interface JsonRpcNotification {
    jsonrpc: "2.0";
    method: string;
    params?: unknown;
}

export interface JsonRpcError {
    code: number;
    message: string;
    data?: unknown;
}

export interface ToolDefinition {
    name: string;
    description: string;
    inputSchema: Record<string, unknown>;
}

export interface ToolContentText {
    type: "text";
    text: string;
}

export interface ToolCallResult {
    content: ToolContentText[];
    isError?: boolean;
}

export interface ResourceDefinition {
    uri: string;
    name: string;
    description?: string;
    mimeType?: string;
}
