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

const Host = require("../index");

const fs = require('node:fs');

let verifying_key = fs.readFileSync('moor-verifying-key.pem', 'utf8');
let signing_key = fs.readFileSync('moor-signing-key.pem', 'utf8');
let host = new Host({
    public_key: verifying_key,
    private_key: signing_key,
});

class Listeners {
    constructor() {
        this.listeners = [];
    }

    getListeners() {
        return this.listeners;
    }

    addListener(l) {
        this.listeners.push(l);
    }

    removeListener(s) {
        this.listeners = this.listeners.filter(l => l !== s);
    }
}

async function establishConnection(host) {
    let connection = await host.newConnection("ipc:///tmp/moor_rpc.sock", "ipc:///tmp/moor_events.sock", "192.168.0.1:8080",
        (msg) => {
            console.log("onSystemMessage: ", msg);
        },
        (msg) => {
            console.log("onNarrativeEvent: ", msg);
        },
        (msg) => {
            console.log("onRequestInput: ", msg);
        },
        (msg) => {
            console.log("onDisconnect: ", msg);
        },
        (msg) => {
            console.log("onTaskError: ", msg);
        },
        (msg) => {
            console.log("onTaskSuccess: ", msg);
        }
    );
    console.log("newConnection =>: ", connection);
    await connection.login("connect", "wizard", "");
    let task_id = await connection.send("say hi");
    console.log("task_id: ", task_id);
    // await connection.disconnect();
}

let l = new Listeners();
host.attachToDaemon("ipc:///tmp/moor_rpc.sock", l.getListeners.bind(l), l.addListener.bind(l), l.removeListener.bind(l)).then(() => {
    host.listenHostEvents("ipc:///tmp/moor_events.sock", "nodejs");
    establishConnection(host).then(() => {
        console.log("Connection established");
    });
})


