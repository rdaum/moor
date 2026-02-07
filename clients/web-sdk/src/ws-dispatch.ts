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

import { ClientEventUnion } from "@moor/schema/generated/moor-rpc/client-event-union";
import { NarrativeEventMessage } from "@moor/schema/generated/moor-rpc/narrative-event-message";
import { RequestInputEvent } from "@moor/schema/generated/moor-rpc/request-input-event";
import { SystemMessageEvent } from "@moor/schema/generated/moor-rpc/system-message-event";
import { TaskErrorEvent } from "@moor/schema/generated/moor-rpc/task-error-event";
import { TaskSuccessEvent } from "@moor/schema/generated/moor-rpc/task-success-event";

import { parseClientEvent } from "./client-event";

export interface WsDispatchHandlers {
    onNarrativeEventMessage?: (narrative: NarrativeEventMessage) => void;
    onSystemMessageEvent?: (sysMsg: SystemMessageEvent) => void;
    onRequestInputEvent?: (requestInput: RequestInputEvent) => void;
    onTaskErrorEvent?: (taskError: TaskErrorEvent) => void;
    onTaskSuccessEvent?: (taskSuccess: TaskSuccessEvent) => void;
    onIgnoredEvent?: (eventType: ClientEventUnion) => void;
    onUnknownEvent?: (eventType: ClientEventUnion) => void;
    onMalformedEvent?: (eventType: ClientEventUnion, expected: string) => void;
}

export function dispatchClientEvent(bytes: Uint8Array, handlers: WsDispatchHandlers): void {
    const { eventType, eventUnion } = parseClientEvent(bytes);

    switch (eventType) {
        case ClientEventUnion.NarrativeEventMessage: {
            if (!(eventUnion instanceof NarrativeEventMessage)) {
                handlers.onMalformedEvent?.(eventType, "NarrativeEventMessage");
                return;
            }
            handlers.onNarrativeEventMessage?.(eventUnion);
            return;
        }
        case ClientEventUnion.SystemMessageEvent: {
            if (!(eventUnion instanceof SystemMessageEvent)) {
                handlers.onMalformedEvent?.(eventType, "SystemMessageEvent");
                return;
            }
            handlers.onSystemMessageEvent?.(eventUnion);
            return;
        }
        case ClientEventUnion.RequestInputEvent: {
            if (!(eventUnion instanceof RequestInputEvent)) {
                handlers.onMalformedEvent?.(eventType, "RequestInputEvent");
                return;
            }
            handlers.onRequestInputEvent?.(eventUnion);
            return;
        }
        case ClientEventUnion.TaskErrorEvent: {
            if (!(eventUnion instanceof TaskErrorEvent)) {
                handlers.onMalformedEvent?.(eventType, "TaskErrorEvent");
                return;
            }
            handlers.onTaskErrorEvent?.(eventUnion);
            return;
        }
        case ClientEventUnion.TaskSuccessEvent: {
            if (!(eventUnion instanceof TaskSuccessEvent)) {
                handlers.onMalformedEvent?.(eventType, "TaskSuccessEvent");
                return;
            }
            handlers.onTaskSuccessEvent?.(eventUnion);
            return;
        }
        case ClientEventUnion.PlayerSwitchedEvent:
        case ClientEventUnion.SetConnectionOptionEvent:
        case ClientEventUnion.DisconnectEvent:
            handlers.onIgnoredEvent?.(eventType);
            return;
        default:
            handlers.onUnknownEvent?.(eventType);
            return;
    }
}
