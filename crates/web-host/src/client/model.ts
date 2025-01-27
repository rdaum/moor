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

import van, { State } from "vanjs-core";
import { Notice } from "./moor";
import { ObjectRef } from "./var";

import * as vanX from "vanjs-ext";

export class Player {
    connected: boolean;
    oid: string;
    auth_token: string;

    constructor(oid, auth_token, connected) {
        this.oid = oid;
        this.auth_token = auth_token;
        this.connected = connected;
    }
}

export enum SpoolType {
    Verb,
    Property,
}

// A spool is a temporary buffer for collecting lines of text sent from the server, usually from
// an MCP-style feed.
export class Spool {
    type: SpoolType;
    name: string;
    object: ObjectRef;
    entity: string;
    content: Array<string>;
    uploadAction: string;

    constructor(type, name: string, object, entity, uploadAction) {
        this.type = type;
        this.name = name;
        this.object = object;
        this.entity = entity;
        this.content = [];
        this.uploadAction = uploadAction;
    }

    append(line) {
        this.content.push(line);
    }

    take() {
        let content = this.content;
        this.content = [];
        return content;
    }
}

export interface PresentationModel {
    id: string;
    target: string;
    closed: State<boolean>;
    content: HTMLElement;
    attrs: { [key: string]: string };
}

// Global context holding the state of the session.
export class Context {
    ws: WebSocket | null;
    history: string[];
    historyOffset: number;
    authToken: string | null;
    systemMessage: Notice;
    player: Player;
    spool: Spool | null;
    presentations: State<Presentations>;

    constructor() {
        this.ws = null;
        this.history = [];
        this.historyOffset = 0;
        this.authToken = null;
        this.systemMessage = new Notice();
        this.player = new Player("", "", false);
        this.spool = null;
        this.presentations = van.state(new Presentations({}));
    }
}

export class Presentations {
    constructor(private presentations: { [key: string]: State<PresentationModel> }) {
    }

    withAdded(id: string, model: State<PresentationModel>) {
        let old = this.presentations;
        old[id] = model;
        return new Presentations(old);
    }

    withRemoved(id: string) {
        let old = this.presentations;
        delete old[id];
        return new Presentations(old);
    }

    rightDockPresentations() {
        // return a calculation of the set of presentations that are target 'right-dock', as derived state
        let rds = [];
        for (const id in this.presentations) {
            let p = this.presentations[id];
            console.log("Presentation: ", p, " @ ", id);
            if (p.val.target == "right-dock") {
                rds.push(id);
            }
        }
        return rds;
    }

    getPresentation(id: string) {
        return this.presentations[id];
    }
}

export enum EventKind {
    SystemMessage,
    NarrativeMessage,
}

export interface NarrativeEvent {
    kind: EventKind.NarrativeMessage;
    message: string;
    content_type: string | null;
    author: string;
}

export interface SystemEvent {
    kind: EventKind.SystemMessage;
    system_message: string;
}

export interface Presentation {
    id: string;
    content_type: string;
    content: string;
    target: string;
    attributes: Array<[string, string]>;
}

export interface Traceback {
    msg: string;
    code: string;
    traceback: Array<string>;
}
