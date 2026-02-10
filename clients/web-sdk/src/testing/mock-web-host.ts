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

export interface MockWebHostConnectRequest {
    url: string;
    protocols: string[];
}

export interface MockWebHostConnection {
    readonly request: MockWebHostConnectRequest;
    serverOpen(): void;
    serverSendBinary(data: Uint8Array): void;
    serverClose(code?: number, reason?: string): void;
}

interface SocketState {
    url: string;
    protocols: string[];
    readyState: number;
    onopen: ((event: Event) => void) | null;
    onclose: ((event: CloseEvent) => void) | null;
    onerror: ((event: Event) => void) | null;
    onmessage: ((event: MessageEvent) => void) | null;
}

function mkCloseEvent(code: number, reason: string): CloseEvent {
    return {
        code,
        reason,
        wasClean: code === 1000,
    } as CloseEvent;
}

function mkMessageEvent(bytes: Uint8Array): MessageEvent {
    return {
        data: bytes.buffer,
    } as MessageEvent;
}

class MockSocket {
    static readonly CONNECTING = 0;
    static readonly OPEN = 1;
    static readonly CLOSING = 2;
    static readonly CLOSED = 3;

    url: string;
    readyState = MockSocket.CONNECTING;
    onopen: ((event: Event) => void) | null = null;
    onclose: ((event: CloseEvent) => void) | null = null;
    onerror: ((event: Event) => void) | null = null;
    onmessage: ((event: MessageEvent) => void) | null = null;
    protocol = "moor";
    binaryType: BinaryType = "blob";
    extensions = "";

    private readonly _protocols: string[];
    private readonly sentFrames: Array<string | ArrayBuffer | Uint8Array> = [];

    constructor(url: string, protocols?: string | string[]) {
        this.url = url;
        this._protocols = Array.isArray(protocols)
            ? protocols
            : protocols
            ? [protocols]
            : [];
    }

    get protocols(): string[] {
        return [...this._protocols];
    }

    send(data: string | ArrayBuffer | Uint8Array): void {
        this.sentFrames.push(data);
    }

    close(code = 1000, reason = ""): void {
        if (this.readyState === MockSocket.CLOSED) {
            return;
        }
        this.readyState = MockSocket.CLOSING;
        this.readyState = MockSocket.CLOSED;
        this.onclose?.(mkCloseEvent(code, reason));
    }

    openFromServer(): void {
        if (this.readyState !== MockSocket.CONNECTING) {
            return;
        }
        this.readyState = MockSocket.OPEN;
        this.onopen?.({} as Event);
    }

    messageFromServer(data: Uint8Array): void {
        if (this.readyState !== MockSocket.OPEN) {
            return;
        }
        this.onmessage?.(mkMessageEvent(data));
    }

    closeFromServer(code = 1000, reason = ""): void {
        if (this.readyState === MockSocket.CLOSED) {
            return;
        }
        this.readyState = MockSocket.CLOSED;
        this.onclose?.(mkCloseEvent(code, reason));
    }

    getState(): SocketState {
        return {
            url: this.url,
            protocols: [...this._protocols],
            readyState: this.readyState,
            onopen: this.onopen,
            onclose: this.onclose,
            onerror: this.onerror,
            onmessage: this.onmessage,
        };
    }
}

export interface MockWebHostController {
    readonly connections: MockWebHostConnectRequest[];
    takeConnection(index?: number): MockWebHostConnection | null;
    restore(): void;
}

export function installMockWebHostWebSocket(): MockWebHostController {
    const root = globalThis as typeof globalThis & {
        WebSocket: any;
    };
    const original = root.WebSocket;
    const sockets: MockSocket[] = [];
    const requests: MockWebHostConnectRequest[] = [];

    class TestWebSocket extends MockSocket {
        static readonly CONNECTING = MockSocket.CONNECTING;
        static readonly OPEN = MockSocket.OPEN;
        static readonly CLOSING = MockSocket.CLOSING;
        static readonly CLOSED = MockSocket.CLOSED;

        constructor(url: string, protocols?: string | string[]) {
            super(url, protocols);
            sockets.push(this);
            requests.push({
                url,
                protocols: this.protocols,
            });
        }
    }

    root.WebSocket = TestWebSocket;

    return {
        get connections() {
            return [...requests];
        },
        takeConnection(index = 0): MockWebHostConnection | null {
            const socket = sockets[index];
            if (!socket) {
                return null;
            }
            return {
                request: {
                    url: socket.url,
                    protocols: socket.protocols,
                },
                serverOpen: () => socket.openFromServer(),
                serverSendBinary: (data: Uint8Array) => socket.messageFromServer(data),
                serverClose: (code?: number, reason?: string) => socket.closeFromServer(code, reason),
            };
        },
        restore(): void {
            root.WebSocket = original;
        },
    };
}
