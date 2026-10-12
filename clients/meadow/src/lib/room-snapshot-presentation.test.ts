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

// Copyright (C) 2026 The mooR Authors
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { roomSnapshotToPresentation } from "./room-snapshot-presentation";

describe("captured room presentations", () => {
    const annotation = {
        kind: "command",
        command: "go e",
        exit: { source: "oid:10", destination: "oid:20", passage: "identity" },
    };
    const snapshot = {
        room: 10,
        title: "Hall",
        content: "<p>A <strong>brass</strong> bell.</p><p data-moor-annotation=\"a1\">East door</p>",
        content_type: "text/html",
        annotations: { a1: annotation },
    };
    it("uses the core body unchanged with its own annotation table and separate title", () => {
        const presentation = roomSnapshotToPresentation(snapshot);
        expect(presentation?.content).toBe(snapshot.content);
        expect(presentation?.annotations?.a1).toEqual(annotation);
        expect(presentation?.attributes).toEqual([["title", "Hall"], ["kind", "room_look"], ["room", "oid:10"]]);
    });
    it.each(["text/html", "text/djot", "text/plain"])("accepts rendered %s without interpreting the content", type => {
        const presentation = roomSnapshotToPresentation({ ...snapshot, content_type: type, content: "*literal*" });
        expect(presentation?.content).toBe("*literal*");
        expect(presentation?.content_type).toBe(type);
    });
    it("rejects raw descriptions and unsupported bodies instead of inventing a representation", () => {
        expect(roomSnapshotToPresentation({ room: 10, title: "Hall", description: "raw" })).toBeNull();
        expect(roomSnapshotToPresentation({ ...snapshot, content_type: "text/unknown" })).toBeNull();
        expect(roomSnapshotToPresentation({ ...snapshot, content: {} })).toBeNull();
    });
    it("retains separate identities for identical labels and validates descriptors", () => {
        const presentation = roomSnapshotToPresentation({
            ...snapshot,
            annotations: {
                a1: { kind: "object", ref: "oid:11" },
                a2: { kind: "object", ref: "oid:12" },
                bad: { kind: "command", command: "go east\n@shutdown" },
            },
        });
        expect(presentation?.annotations).toEqual({
            a1: { kind: "object", ref: "oid:11" },
            a2: { kind: "object", ref: "oid:12" },
        });
    });
});
