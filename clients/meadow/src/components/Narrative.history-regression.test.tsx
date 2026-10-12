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

import { act, render } from "@testing-library/react";
import { createRef, useSyncExternalStore } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { installMockWebHostFetch } from "../../../web-sdk/src/testing/mock-http-host";
import type { Transcript } from "../lib/transcript";
import { Narrative, NarrativeMessage, NarrativeRef } from "./Narrative";

vi.mock("./InputArea", () => ({
    InputArea: () => null,
}));

vi.mock("./OutputWindow", () => ({
    OutputWindow: ({ transcript }: { transcript: Transcript }) => {
        useSyncExternalStore(transcript.subscribe, transcript.getVersion);
        const messages = transcript.allMessages();
        return (
            <div data-testid="output-window" data-messages={JSON.stringify(messages)}>
                {messages.map((message) => String(message.content)).join("|")}
            </div>
        );
    },
}));

interface MockHistoryItem {
    id: string;
    content: string;
    timestamp: number;
}

function installMatchMediaMock(matches = false) {
    Object.defineProperty(window, "matchMedia", {
        configurable: true,
        writable: true,
        value: (query: string) => ({
            matches,
            media: query,
            onchange: null,
            addListener: () => {},
            removeListener: () => {},
            addEventListener: () => {},
            removeEventListener: () => {},
            dispatchEvent: () => false,
        }),
    });
}

async function fetchMockHistory(): Promise<NarrativeMessage[]> {
    const response = await fetch("/v1/history?since_seconds=86400");
    if (!response.ok) {
        throw new Error(`mock history fetch failed: ${response.status}`);
    }
    const data = await response.json() as { events: MockHistoryItem[] };
    return data.events.map((event) => ({
        id: event.id,
        content: event.content,
        type: "narrative",
        timestamp: event.timestamp,
        isHistorical: true,
        contentType: "text/plain",
    }));
}

describe("Narrative history merge regressions", () => {
    afterEach(() => {
        // no-op cleanup handled per-test by restore()
    });

    it("keeps chronological order when historical backfill overlaps live messages", async () => {
        installMatchMediaMock();
        const mockHttp = installMockWebHostFetch([
            {
                method: "GET",
                path: "/v1/history",
                handler: () =>
                    new Response(
                        JSON.stringify({
                            events: [
                                { id: "hist_old", content: "HIST old (t=80)", timestamp: 80 },
                                { id: "hist_new", content: "HIST new (t=120)", timestamp: 120 },
                            ],
                        }),
                        {
                            status: 200,
                            headers: { "Content-Type": "application/json" },
                        },
                    ),
            },
        ]);

        try {
            const narrativeRef = createRef<NarrativeRef>();
            const { container } = render(
                <Narrative
                    ref={narrativeRef}
                    visible={true}
                    connectionStatus="connected"
                    onSendMessage={() => true}
                />,
            );

            act(() => {
                narrativeRef.current?.addNarrativeContent(
                    "LIVE (t=100)",
                    "text/plain",
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    100,
                );
            });

            const historicalMessages = await fetchMockHistory();
            act(() => {
                narrativeRef.current?.addHistoricalMessages(historicalMessages);
            });

            const requestUrls = mockHttp.getRequests().map((r) => r.url.pathname);
            expect(requestUrls).toContain("/v1/history");

            const rendered = container.querySelector("[data-testid=\"output-window\"]")?.textContent || "";
            const narrativeLines = rendered.split("|").map((line) => line.trim()).filter(Boolean);

            // Desired chronological order should be old history -> live -> newer history.
            // Current implementation prepends all historical messages, which places LIVE last.
            expect(narrativeLines).toEqual([
                "HIST old (t=80)",
                "LIVE (t=100)",
                "HIST new (t=120)",
            ]);
        } finally {
            mockHttp.restore();
        }
    });

    it("does not append queued live duplicate after history merge of same event id", async () => {
        installMatchMediaMock(true);
        vi.useFakeTimers();
        const narrativeRef = createRef<NarrativeRef>();
        const { container } = render(
            <Narrative
                ref={narrativeRef}
                visible={true}
                connectionStatus="connected"
                onSendMessage={() => true}
            />,
        );

        try {
            // First message commits immediately.
            act(() => {
                narrativeRef.current?.addNarrativeContent(
                    "LIVE e1",
                    "text/plain",
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    {
                        eventId: "e1",
                    },
                );
            });

            // Second message with unique event id should be queued (rapid arrival).
            act(() => {
                narrativeRef.current?.addNarrativeContent(
                    "LIVE e2",
                    "text/plain",
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    {
                        eventId: "e2",
                    },
                );
            });

            // History load inserts same e2 before queued live message flushes.
            act(() => {
                narrativeRef.current?.addHistoricalMessages([
                    {
                        id: "hist_e2",
                        eventId: "e2",
                        content: "HIST e2",
                        type: "narrative",
                        timestamp: 10,
                        isHistorical: true,
                        contentType: "text/plain",
                    },
                ]);
            });

            // Flush queued DOM commits.
            act(() => {
                vi.advanceTimersByTime(250);
            });

            const rendered = container.querySelector("[data-testid=\"output-window\"]")?.textContent || "";
            const lines = rendered.split("|").map(line => line.trim()).filter(Boolean);

            // e2 should appear only once (from history), not duplicated by delayed live queue flush.
            expect(lines).toEqual(["HIST e2", "LIVE e1"]);
        } finally {
            vi.useRealTimers();
        }
    });

    it("dedupes history/live copies by shared delivery id even when event ids differ", async () => {
        installMatchMediaMock();
        const narrativeRef = createRef<NarrativeRef>();
        const { container } = render(
            <Narrative
                ref={narrativeRef}
                visible={true}
                connectionStatus="connected"
                onSendMessage={() => true}
            />,
        );

        act(() => {
            narrativeRef.current?.addNarrativeContent(
                "SAME LINE",
                "text/plain",
                undefined,
                undefined,
                undefined,
                undefined,
                undefined,
                undefined,
                {
                    eventId: "live_eid",
                    deliveryId: "delivery_1",
                },
                undefined,
                undefined,
                100,
            );
        });

        act(() => {
            narrativeRef.current?.addHistoricalMessages([
                {
                    id: "hist_delivery_1",
                    eventId: "hist_eid",
                    content: "SAME LINE",
                    type: "narrative",
                    timestamp: 90,
                    isHistorical: true,
                    contentType: "text/plain",
                    eventMetadata: {
                        delivery_id: "delivery_1",
                    },
                },
            ]);
        });

        const rendered = container.querySelector("[data-testid=\"output-window\"]")?.textContent || "";
        const lines = rendered.split("|").map(line => line.trim()).filter(Boolean);
        expect(lines).toEqual(["SAME LINE"]);
    });
    it("rewrites complete content envelopes together and preserves rejected rewrite targets", async () => {
        const document = (text: string) => ({
            version: 1,
            blocks: [{ kind: "paragraph", children: [{ kind: "text", text }] }],
        });
        installMatchMediaMock();
        const ref = createRef<NarrativeRef>();
        const { container } = render(
            <Narrative ref={ref} visible connectionStatus="connected" onSendMessage={() => true} />,
        );
        act(() =>
            ref.current!.addNarrativeContent(
                "[old]{annotation=a1}",
                "text/djot",
                false,
                undefined,
                undefined,
                undefined,
                undefined,
                undefined,
                {
                    eventId: "original",
                    actor: { oid: 2 },
                    cowbellDocument: document("old"),
                    annotations: { a1: { kind: "object", ref: "oid:47" } },
                },
                { id: "replace", owner: "oid:2", ttl: 30 },
            )
        );
        await act(async () => {
            await new Promise(resolve => setTimeout(resolve, 100));
        });
        for (
            const metadata of [
                { actor: { oid: 3 }, cowbellDocument: document("wrong owner"), annotations: {} },
                { actor: { oid: 2 }, annotations: {} },
                { actor: { oid: 2 }, cowbellDocument: document("missing annotations") },
            ]
        ) {
            act(() =>
                ref.current!.addNarrativeContent(
                    "rejected",
                    "text/plain",
                    false,
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    undefined,
                    metadata,
                    undefined,
                    "replace",
                )
            );
            const unchanged = JSON.parse(container.querySelector("[data-messages]")!.getAttribute("data-messages")!);
            expect(unchanged[0].content).toBe("[old]{annotation=a1}");
            expect(unchanged[0].eventMetadata.cowbellDocument).toEqual(document("old"));
            expect(unchanged[0].rewritable.owner).toBe("oid:2");
        }
        act(() =>
            ref.current!.addNarrativeContent(
                "[new]{annotation=a1}",
                "text/djot",
                false,
                undefined,
                undefined,
                undefined,
                undefined,
                undefined,
                {
                    eventId: "replacement",
                    actor: { oid: 2 },
                    cowbellDocument: document("new"),
                    annotations: { a1: { kind: "object", ref: "oid:65" } },
                },
                undefined,
                "replace",
            )
        );
        const messages = JSON.parse(container.querySelector("[data-messages]")!.getAttribute("data-messages")!);
        expect(messages).toHaveLength(1);
        expect(messages[0].content).toBe("[new]{annotation=a1}");
        expect(messages[0].eventMetadata.annotations.a1.ref).toBe("oid:65");
        expect(messages[0].eventMetadata.cowbellDocument).toEqual(document("new"));
        act(() =>
            ref.current!.addNarrativeContent(
                "duplicate",
                "text/plain",
                false,
                undefined,
                undefined,
                undefined,
                undefined,
                undefined,
                { eventId: "original" },
            )
        );
        expect(JSON.parse(container.querySelector("[data-messages]")!.getAttribute("data-messages")!)).toHaveLength(1);
    });
});
