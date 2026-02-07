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

import { NarrativeEvent } from "@moor/schema/generated/moor-common/narrative-event";
import { VerbCallSuccess } from "@moor/schema/generated/moor-rpc/verb-call-success";

import { parseNarrativeEvent } from "./narrative";

export type WelcomeContentType = "text/plain" | "text/djot" | "text/html" | "text/traceback" | "text/x-uri";

export interface WelcomeMessagePayload {
    welcomeMessage: string;
    contentType: WelcomeContentType;
}

export function extractWelcomeMessage(
    verbCallSuccess: VerbCallSuccess,
    decodeVarToJs: (value: unknown) => unknown,
): WelcomeMessagePayload {
    let welcomeMessage = "";
    let contentType: WelcomeContentType = "text/plain";

    for (let i = 0; i < verbCallSuccess.outputLength(); i++) {
        const narrativeEvent = verbCallSuccess.output(i, new NarrativeEvent());
        if (!narrativeEvent) {
            continue;
        }

        const parsed = parseNarrativeEvent(
            narrativeEvent,
            decodeVarToJs,
            () => null,
        );
        if (!parsed || parsed.eventType !== "NotifyEvent") {
            continue;
        }

        const content = parsed.event.value;
        if (typeof content === "string") {
            welcomeMessage = content;
        } else if (Array.isArray(content)) {
            welcomeMessage = content.join("\n");
        }

        const ct = parsed.event.contentType;
        if (ct === "text/plain" || ct === "text/djot" || ct === "text/html" || ct === "text/x-uri") {
            contentType = ct;
        }

        break;
    }

    return { welcomeMessage, contentType };
}
