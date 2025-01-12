// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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
//

import {Host, Connection, ConnectionArguments, ClientArguments, HostArguments} from 'moor-node-host';
import {readFileSync} from 'node:fs';
import WebSocket, { WebSocketServer } from 'ws';


// Track the set of listeners this host has exposed.
// The Daemon can request new listeners, and the current set of listeners, or remove listeners.
// A listener is a { obj: Object, addr: String } pair.
export class Listeners {
    listeners: Map<String, Object> = new Map();

    addListener(obj : Object, addr : String) {
        this.listeners.set(addr, obj);
    }

    removeListener(addr : String) {
        this.listeners.delete(addr);
    }

    getListeners() {
        // This needs to return as an array of { obj: Object, addr: String } pairs.
        let listeners :  { obj: Object, addr: String }[] = [];
        this.listeners.forEach((obj, addr) => {
            listeners.push({ obj: obj, addr: addr });
        });
        return listeners;
    }
}

// We have to turn
function peerAddrToConnectionId(address: string, port: number) {
    // If the address is ::1 we need to turn that into something Rust can parse as an IP address.
    if (address == "::1") {
        address = "0.0.0.0";
    }
    return address + ":" + port;
}

// Start up the websocket server, and listen for incoming connections, and pipe them to/from the Host Connection
export async function startWebSocketServer(host: Host) {
    const wss = new WebSocketServer({ port: 8080 });

    wss.on('connection', async function connection(ws : WebSocket, req) {
        console.log("WebSocket connection established");
        let clientArguments : ClientArguments = {
            rpcAddress: "ipc:///tmp/moor_rpc.sock",
            eventsAddress: "ipc:///tmp/moor_events.sock"
        };

        const peerAddr = peerAddrToConnectionId(req.connection.remoteAddress, req.connection.remotePort);
        let connectionArguments = {
            peerAddr: peerAddr,
            onSystemMessage:
                (msg : String) => {
                    ws.send(msg);
                },
            onNarrativeEvent: (msg : String) => {
                ws.send(msg);
            },
            onRequestInput: () => {},
            onDisconnect: () => {},
            onTaskError: () => {},
            onTaskSuccess: () => {}
        };

        let connection : Connection = await host.newConnection(clientArguments, connectionArguments);
        console.log("Connection established: ", connection);

        // TODO: This is a hack to get the connection to authenticate. We need to add a login screen to the frontend, and then
        //  use the login credentials to authenticate the connection.
        await connection.login("connect", "wizard", "");
        console.log("Connection authorized: ", connection);

        ws.on('close', () => {
            console.log("WebSocket connection closed");
        });

        ws.on('message', (message : string) => {
            let jsonMsg = JSON.parse(message);
            console.log("WebSocket message received: ", jsonMsg);
            if (jsonMsg.type == "connect") {
                let user = jsonMsg.payload.player;
                let password = jsonMsg.payload.password;
                connection.login("connect", user, password).then(() => {
                    console.log("Connection authorized: ", connection);
                });
            }

            if (jsonMsg.type == "input") {
                console.log("Sending input: ", jsonMsg.payload.message);
                connection.command(jsonMsg.payload.message);
            }
        });
    })

    return wss;
}

export class MoorHost {
    host: Host;
    listeners: Listeners;
    webSocketServer: WebSocketServer;

    constructor(privateKeyFileName : string, publicKeyFileName: string, daemonRpcAddr: string, daemonEventsAddr: string

    ) {
        let verifying_key = readFileSync(publicKeyFileName, 'utf8');
        let signing_key = readFileSync(privateKeyFileName, 'utf8');

        let hostArguments : HostArguments = {
            public_key: verifying_key,
            private_key: signing_key
        }
        let host = new Host(hostArguments);

        let l = new Listeners();
        host.attachToDaemon(daemonRpcAddr, l.getListeners.bind(l), l.addListener.bind(l), l.removeListener.bind(l)).then(() => {
            host.listenHostEvents(daemonEventsAddr, "frontend");
        });

        this.host = host;
        this.listeners = l;
        startWebSocketServer(host).then((wss) => {
            this.webSocketServer = wss;
        });
    }
}

