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

interface HostBindingsSig {
    createHost(hostArguments: HostArguments): any;
    attachToDaemon(host: any, rpcAddress: string, onGetListeners: Function, onAddListener: Function, onRemoveListener: Function): any;
    listenHostEvents(host: any, eventsAddress: string, listenAddr: string): any;
    shutdownHost(host: any): any;
    newConnection(host: any, rpcAddress: string, eventsAddress: string, peerAddr: string, onSystemMessage: Function, onNarrativeEvent: Function, onRequestInput: Function, onDisconnect: Function, onTaskError: Function, onTaskSuccess: Function): any;
    connectionLogin(connection: any, loginType: string, username: string, password: string): any;
    connectionCommand(connection: any, msg: string): any;
    connectionDisconnect(connection: any): any;
    welcomeMessage(connection: any): any;
}

const hostBindings : HostBindingsSig = require("../index.node");

export interface HostArguments {
    public_key: string;
    private_key: string;
}

export interface ClientArguments {
    rpcAddress: string;
    eventsAddress: string;
}

export interface ConnectionArguments {
    peerAddr: string;
    onSystemMessage: (msg: any) => void;
    onNarrativeEvent: (msg: any) => void;
    onRequestInput: Function;
    onDisconnect: Function;
    onTaskError: Function;
    onTaskSuccess: Function;
}

export class Host {
    host: any;

    // clientArgs: { public_key: string, private_key: string }
    constructor(hostArguments: HostArguments) {
        this.host = hostBindings.createHost(hostArguments);
    }

    // Attach to the daemon at the given ZMQ address.
    async attachToDaemon(rpcAddress: string, onGetListeners: Function, onAddListener: Function, onRemoveListener: Function) {
        return await hostBindings.attachToDaemon(this.host, rpcAddress, onGetListeners, onAddListener, onRemoveListener);
    }

    listenHostEvents(eventsAddress: string, listenAddr: string) {
        hostBindings.listenHostEvents(this.host, eventsAddress, listenAddr);
    }

    shutdown() {
        hostBindings.shutdownHost(this.host);
    }

    async newConnection(clientArgs: ClientArguments, connectionArgs: ConnectionArguments) {
        let connection = await hostBindings.newConnection(
            this.host,
            clientArgs.rpcAddress,
            clientArgs.eventsAddress,
            connectionArgs.peerAddr,
            connectionArgs.onSystemMessage,
            connectionArgs.onNarrativeEvent,
            connectionArgs.onRequestInput,
            connectionArgs.onDisconnect,
            connectionArgs.onTaskError,
            connectionArgs.onTaskSuccess
        );
        return new Connection(connection);
    }
}

export class Connection {
    connection: any;

    constructor(connection: any) {
        this.connection = connection;
    }

    async login(loginType: string, username: string, password: string) {
        return await hostBindings.connectionLogin(this.connection, loginType, username, password);
    }

    async command(msg: string) {
        return await hostBindings.connectionCommand(this.connection, msg);
    }

    async welcomeMessage() {
        return await hostBindings.welcomeMessage(this.connection);
    }

    async disconnect() {
        return await hostBindings.connectionDisconnect(this.connection);
    }
}

