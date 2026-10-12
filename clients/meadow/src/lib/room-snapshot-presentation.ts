// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

import { decodeAnnotations } from "@moor/web-sdk";
import { PresentationData } from "../types/presentation";
import { jsObjectRefToCurie } from "./var";

export function roomSnapshotToPresentation(payload: unknown): PresentationData | null {
    if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
    const snapshot = payload as Record<string, unknown>;
    if (typeof snapshot.title !== "string" || typeof snapshot.content !== "string") return null;
    const contentType = snapshot.content_type;
    if (contentType !== "text/html" && contentType !== "text/djot" && contentType !== "text/plain") return null;
    const room = jsObjectRefToCurie(snapshot.room);
    if (!room) return null;

    return {
        id: "room-look",
        target: "top",
        content_type: contentType,
        content: snapshot.content,
        annotations: decodeAnnotations(snapshot.annotations),
        attributes: [["title", snapshot.title], ["kind", "room_look"], ["room", room]],
    };
}
