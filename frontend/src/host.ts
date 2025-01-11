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

import {Host, Connection} from 'moor-node-host';
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

// Create a new host, attach it to the daemon, and listen for events.
export function startupHost() : Host {
    let verifying_key = readFileSync('../moor-verifying-key.pem', 'utf8');
    let signing_key = readFileSync('../moor-signing-key.pem', 'utf8');
    let host = new Host({
        public_key: verifying_key,
        private_key: signing_key,
    });

    let l = new Listeners();
    host.attachToDaemon("ipc:///tmp/moor_rpc.sock", l.getListeners.bind(l), l.addListener.bind(l), l.removeListener.bind(l)).then(() => {
        host.listenHostEvents("ipc:///tmp/moor_events.sock", "frontend");
    });
    return host
}

// Start up the websocket server, and listen for incoming connections, and pipe them through a Host connection
export async function startWebSocketServer(host: Host) {
    const wss = new WebSocketServer({ port: 8080 });

    wss.on('connection', async function connection(ws : WebSocket) {
        console.log("WebSocket connection established");
        let connection : Connection = await host.newConnection("ipc:///tmp/moor_rpc.sock", "ipc:///tmp/moor_events.sock", "127.0.0.1:8080", (msg : String) => {
                console.log("onSystemMessage: ", msg);
                ws.send(msg);
                },
            (msg : String) => {
                ws.send(msg);
            },
            (msg : String) => {
                console.log("onRequestInput: ", msg);
            },
            (msg : String) => {
                console.log("onDisconnect: ", msg);
            },
            (msg : String) => {
                console.log("onTaskError: ", msg);
            },
            (msg : String) => {
                console.log("onTaskSuccess: ", msg);
            }
        );
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
                connection.send(jsonMsg.payload.message);
            }
        });
    })


}