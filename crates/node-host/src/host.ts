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

class BindingConnection {};
class BindingHost {};

type Listener = { obj: Object, addr: string };
type GetListenersFunction = () => [Listener];
type AddListenerFunction = (obj: Object, addr: string) => void;
type RemoveListenerFunction = (addr: string) => void;
type TaskId = number;

interface HostBindingsSig {
    createHost(hostArguments: HostArguments): BindingHost;
    attachToDaemon(host: BindingHost, rpcAddress: string,
                   onGetListeners: GetListenersFunction, onAddListener: AddListenerFunction,
                   onRemoveListener: RemoveListenerFunction): Promise<void>;
    listenHostEvents(host: BindingHost, eventsAddress: string, listenAddr: string): any;
    shutdownHost(host: BindingHost): any;
    newConnection(host: BindingHost, rpcAddress: string, eventsAddress: string, peerAddr: string,
                  onSystemMessage: Function, onNarrativeEvent: Function, onRequestInput: Function,
                  onDisconnect: Function, onTaskError: Function, onTaskSuccess: Function): Promise<BindingConnection>;
    connectionLogin(connection: BindingConnection, loginType: string, username: string, password: string): Promise<[string, string]>;
    connectionCommand(connection: BindingConnection, msg: string): Promise<TaskId>;
    connectionDisconnect(connection: BindingConnection): Promise<void>;
    welcomeMessage(connection: BindingConnection): Promise<string>;
    connectionGetPlayerId(connection: BindingConnection): any;
    connectionIsAuthenticated(connection: BindingConnection): any;
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
    host: BindingHost;

    // clientArgs: { public_key: string, private_key: string }
    constructor(hostArguments: HostArguments) {
        this.host = hostBindings.createHost(hostArguments);
    }

    // Attach to the daemon at the given ZMQ address.
    async attachToDaemon(rpcAddress: string, onGetListeners: GetListenersFunction, onAddListener: AddListenerFunction, onRemoveListener: RemoveListenerFunction) {
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
    connection: BindingConnection;

    constructor(connection: any) {
        this.connection = connection;
    }

    async login(loginType: string, username: string, password: string) {
        return await hostBindings.connectionLogin(this.connection, loginType, username, password);
    }

    async command(msg: string): Promise<TaskId> {
        return await hostBindings.connectionCommand(this.connection, msg);
    }

    async welcomeMessage() {
        return await hostBindings.welcomeMessage(this.connection);
    }

    async disconnect() {
        return await hostBindings.connectionDisconnect(this.connection);
    }

    isAuthenticated() {
        return hostBindings.connectionIsAuthenticated(this.connection);
    }

    getPlayerId() {
        return hostBindings.connectionGetPlayerId(this.connection);
    }
}

