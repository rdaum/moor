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

import { ClientEvent } from "@moor/schema/generated/moor-rpc/client-event";
import { ClientEventUnion, unionToClientEventUnion } from "@moor/schema/generated/moor-rpc/client-event-union";
import * as flatbuffers from "flatbuffers";

export interface ParsedClientEvent {
    eventType: ClientEventUnion;
    eventUnion: unknown;
}

export function parseClientEvent(bytes: Uint8Array, context: string = "WebSocket event"): ParsedClientEvent {
    const clientEvent = ClientEvent.getRootAsClientEvent(new flatbuffers.ByteBuffer(bytes));
    const eventType = clientEvent.eventType();
    if (eventType === ClientEventUnion.NONE) {
        throw new Error(`${context}: empty client event`);
    }

    const eventUnion = unionToClientEventUnion(
        eventType,
        (obj: any) => clientEvent.event(obj),
    );
    if (!eventUnion) {
        throw new Error(`${context}: failed to parse client event union`);
    }

    return { eventType, eventUnion };
}
