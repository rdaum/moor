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

import { decodeAnnotations } from "@moor/web-sdk";
import { describe, expect, it } from "vitest";
import { renderDjot, renderHtmlContent } from "./djot-renderer";

const object = { kind: "object", ref: "oid:47" };
const table = decodeAnnotations({ a1: object });

describe("semantic annotation boundary", () => {
    it("drops unknown/malformed entries independently and limits retained occurrences", () => {
        expect(
            decodeAnnotations({
                a1: object,
                a2: { kind: "object", ref: "oid:4garbage" },
                a3: { kind: "execute", command: "go" },
            }),
        ).toEqual({ a1: object });
        expect(
            Object.keys(
                decodeAnnotations(Object.fromEntries(Array.from({ length: 600 }, (_, i) => [`a${i}`, object]))),
            ),
        ).toHaveLength(512);
    });
    it("rejects command newlines and mismatched argument templates", () => {
        expect(
            decodeAnnotations({
                a1: { kind: "command", command: "go east\nshutdown" },
                a2: {
                    kind: "command",
                    template: "put {dobj} in {iobj}",
                    arguments: { dobj: { label: "Item", expectedKind: "object" } },
                },
            }),
        ).toEqual({});
    });
    it("preserves real Djot spans through sanitization and escaped labels", () => {
        const html = renderDjot("[A \\[brass\\] key <&> 🙂]{annotation=a1}", { annotations: table, enableEmoji: true });
        const node = document.createElement("div");
        node.innerHTML = html;
        expect(node.querySelector("[data-moor-annotation=\"a1\"]")?.textContent).toBe("A [brass] key <&> 🙂");
        expect(node.querySelector("[role=\"button\"]")).not.toBeNull();
    });
    it("labels the separate source icon with the actual member", () => {
        const node = document.createElement("div");
        node.innerHTML = renderDjot("[↗]{annotation=a1}", {
            annotations: decodeAnnotations({ a1: { kind: "verb", receiver: "oid:65", name: "get", definer: "oid:8" } }),
        });
        expect(node.querySelector(".semantic-source-link")?.getAttribute("aria-label")).toBe("Browse verb: oid:65:get");
    });
    it("requires the owning event table, for both Djot and HTML", () => {
        for (
            const html of [
                renderDjot("[key]{annotation=a1}"),
                renderHtmlContent("<span data-moor-annotation=\"a1\">key</span>"),
            ]
        ) {
            expect(html).toContain("key");
            expect(html).not.toContain("data-moor-annotation");
            expect(html).not.toContain("role=\"button\"");
        }
        expect(renderHtmlContent("<span data-moor-annotation=\"a1\">key</span>", false, table)).toContain(
            "data-moor-annotation=\"a1\"",
        );
    });
});

// Semantic decoration does not change labels or invent meaning for unknown actions.
it("keeps prose objects undecorated and only renders authored command icons", () => {
    const html = renderHtmlContent(
        "<span data-moor-annotation=\"person\">Pat</span><span data-moor-annotation=\"action\">hand it over</span><span data-moor-annotation=\"custom\">frobnicate</span>",
        false,
        decodeAnnotations({
            person: { kind: "object", ref: "oid:45", objectKind: "person" },
            action: { kind: "command", command: "hand #47 at #45", action: { icon: "give", label: "Give" } },
            custom: {
                kind: "command",
                command: "frobnicate",
                action: { icon: "unrecognised-action", label: "Frobnicate" },
            },
        }),
    );
    const element = document.createElement("div");
    element.innerHTML = html;
    expect(element.querySelector("[data-moor-annotation=\"person\"] svg")).toBeNull();
    expect(element.querySelector("[data-moor-annotation=\"action\"] svg")).toBeTruthy();
    expect(element.querySelector("[data-moor-annotation=\"custom\"] svg")).toBeNull();
    expect(element.textContent).toBe("Pathand it overfrobnicate");
    expect(element.querySelector("svg")?.getAttribute("aria-hidden")).toBe("true");
});

it("identifies people in structured room lists", () => {
    const html = renderHtmlContent(
        "<ul class=\"reference-columns\"><li><span data-moor-annotation=\"person\">Pat</span></li></ul>",
        false,
        decodeAnnotations({ person: { kind: "object", ref: "oid:45", objectKind: "person" } }),
    );
    const element = document.createElement("div");
    element.innerHTML = html;
    expect(element.querySelector(".semantic-person svg")).toBeTruthy();
    expect(element.textContent).toBe("Pat");
});

it("retains the documentation icon on help-topic links", () => {
    const html = renderDjot("[Movement]{annotation=topic}", {
        annotations: decodeAnnotations({ topic: { kind: "help", provider: "oid:9", topic: "movement" } }),
    });
    const element = document.createElement("div");
    element.innerHTML = html;
    expect(element.querySelector("[aria-label=\"Read help: Movement\"] svg")).toBeTruthy();
});

it("uses a command glyph in command lists and lets an authored icon override it", () => {
    const annotations = decodeAnnotations({
        plain: { kind: "command", command: "say hello" },
        authored: { kind: "command", command: "offer", action: { label: "Offer", icon: "give" } },
    });
    const element = document.createElement("div");
    element.innerHTML = renderDjot("{.reference-columns}\n* [say]{annotation=plain}\n* [offer]{annotation=authored}", {
        annotations,
    });
    const plain = element.querySelector("[data-moor-annotation=\"plain\"] svg path");
    const authored = element.querySelector("[data-moor-annotation=\"authored\"] svg path");
    expect(plain).toBeTruthy();
    expect(authored).toBeTruthy();
    expect(plain?.getAttribute("d")).not.toEqual(authored?.getAttribute("d"));
    expect(renderDjot("[say]{annotation=plain}", { annotations })).not.toContain("<svg");
});
