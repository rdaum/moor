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

"use strict";

const {
    createHost, attachToDaemon, listenHostEvents, shutdownHost,
    newConnection, connectionLogin, connectionSend, connectionDisconnect
} = require("./index.node");

class Host {
    // clientArgs: { public_key: string, private_key: string }
    constructor(clientArgs) {
        this.host = createHost(clientArgs);
    }

    // Attach to the daemon at the given ZMQ address.
    async attachToDaemon(rpcAddress, onGetListeners, onAddListener, onRemoveListener) {
        return await attachToDaemon(this.host, rpcAddress, onGetListeners, onAddListener, onRemoveListener)
    }

    listenHostEvents(eventsAddress, listenAddr) {
        listenHostEvents(this.host, eventsAddress, listenAddr);
    }

    shutdown() {
        shutdownHost(this.host);
    }

    async newConnection(rpcAddress, eventsAddress, peerAddr,
                        onSystemMessage, onNarrativeEvent, onRequestInput, onDisconnect, onTaskError, onTaskSuccess) {
        return new Connection(await newConnection(this.host, rpcAddress, eventsAddress, peerAddr,
            onSystemMessage, onNarrativeEvent, onRequestInput, onDisconnect, onTaskError, onTaskSuccess))
    }
}

class Connection {
    constructor(connection) {
        this.connection = connection;
    }

    async login(loginType, username, password) {
        return await connectionLogin(this.connection, loginType, username, password);
    }

    async send(msg) {
        return await connectionSend(this.connection, msg);
    }

    async disconnect() {
        return await connectionDisconnect(this.connection);
    }
}

module.exports =  { Host, Connection };
