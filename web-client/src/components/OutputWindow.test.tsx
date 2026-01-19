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

import { virtual } from "@guidepup/virtual-screen-reader";
import { act, render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { OutputWindow } from "./OutputWindow";

// Helper to create a message
function createMessage(id: string, content: string, opts: {
    type?: "narrative" | "input_echo" | "system" | "error";
    presentationHint?: string;
    groupId?: string;
    contentType?: "text/plain" | "text/djot" | "text/html";
    eventMetadata?: {
        verb?: string;
        dobjName?: string;
    };
} = {}) {
    return {
        id,
        content,
        type: opts.type || "narrative",
        timestamp: Date.now(),
        isHistorical: false,
        contentType: opts.contentType || "text/plain",
        presentationHint: opts.presentationHint,
        groupId: opts.groupId,
        eventMetadata: opts.eventMetadata,
    };
}

// Helper to collect announcements from virtual screen reader
async function collectAnnouncements(maxIterations = 50): Promise<string[]> {
    const announcements: string[] = [];
    let lastPhrase = "";
    let iterations = 0;

    while (iterations < maxIterations) {
        const phrase = await virtual.lastSpokenPhrase();
        if (phrase && phrase !== lastPhrase) {
            announcements.push(phrase);
            lastPhrase = phrase;
        }

        const beforeNext = await virtual.lastSpokenPhrase();
        await virtual.next();
        const afterNext = await virtual.lastSpokenPhrase();

        // If we're at the end (no change after next), break
        if (beforeNext === afterNext && iterations > 5) {
            break;
        }

        iterations++;
    }

    return announcements;
}

// Helper to get only aria-live announcements from spoken phrase log
function getLiveAnnouncements(log: string[]): string[] {
    return log.filter(p => p.startsWith("polite:"));
}

describe("OutputWindow screen reader announcements", () => {
    it("announces simple text messages", async () => {
        const messages = [
            createMessage("1", "You head north."),
        ];

        const { container } = render(<OutputWindow messages={messages} />);
        const outputWindow = container.querySelector("#output_window");
        expect(outputWindow).not.toBeNull();

        await virtual.start({ container: outputWindow as Element });
        const announcements = await collectAnnouncements();
        await virtual.stop();

        expect(announcements.some(a => a.includes("north"))).toBe(true);
    });

    it("announces room descriptions with djot content", async () => {
        const messages = [
            createMessage("1", "You head north."),
            createMessage("2", "# The Anteroom\n\nAn empty room awaiting a description.", {
                presentationHint: "inset",
                groupId: "room-123",
                contentType: "text/djot",
                eventMetadata: {
                    verb: "look",
                    dobjName: "The Anteroom",
                },
            }),
            createMessage("3", "You arrive from the south."),
        ];

        const { container } = render(<OutputWindow messages={messages} />);
        const outputWindow = container.querySelector("#output_window");

        await virtual.start({ container: outputWindow as Element });
        const announcements = await collectAnnouncements();
        await virtual.stop();

        const allText = announcements.join(" ").toLowerCase();
        expect(allText).toContain("north");
        expect(allText).toContain("anteroom");
        expect(allText).toContain("south");
    });

    it("announces all messages in a room transition sequence", async () => {
        const messages = [
            createMessage("1", "You head north."),
            createMessage("2", "# Anteroom\n\nAn empty room.", {
                presentationHint: "inset",
                groupId: "room-1",
                contentType: "text/djot",
                eventMetadata: { verb: "look", dobjName: "Anteroom" },
            }),
            createMessage("3", "You arrive from the south."),
            createMessage("4", "You head south."),
            createMessage("5", "# The First Room\n\nThe starting room.", {
                presentationHint: "inset",
                groupId: "room-2",
                contentType: "text/djot",
                eventMetadata: { verb: "look", dobjName: "The First Room" },
            }),
            createMessage("6", "You arrive from the north."),
        ];

        const { container } = render(<OutputWindow messages={messages} />);
        const outputWindow = container.querySelector("#output_window");

        await virtual.start({ container: outputWindow as Element });
        const announcements = await collectAnnouncements(100);
        await virtual.stop();

        const allText = announcements.join(" ").toLowerCase();
        expect(allText).toContain("head north");
        expect(allText).toContain("head south");
        expect(allText).toContain("arrive from the south");
        expect(allText).toContain("arrive from the north");
        expect(allText).toContain("anteroom");
        expect(allText).toContain("first room");
    });

    it("announces dynamically added messages via aria-live", async () => {
        const messages = [
            createMessage("1", "Initial message."),
        ];

        const { container, rerender } = render(<OutputWindow messages={[...messages]} />);
        const outputWindow = container.querySelector("#output_window");

        await virtual.start({ container: outputWindow as Element });
        await new Promise(r => setTimeout(r, 10));

        // Add messages one at a time with delays
        messages.push(createMessage("2", "Second message."));
        await act(async () => {
            rerender(<OutputWindow messages={[...messages]} />);
        });
        await new Promise(r => setTimeout(r, 50));

        messages.push(createMessage("3", "Third message."));
        await act(async () => {
            rerender(<OutputWindow messages={[...messages]} />);
        });
        await new Promise(r => setTimeout(r, 50));

        messages.push(createMessage("4", "Fourth message."));
        await act(async () => {
            rerender(<OutputWindow messages={[...messages]} />);
        });
        await new Promise(r => setTimeout(r, 50));

        const liveAnnouncements = getLiveAnnouncements(await virtual.spokenPhraseLog());
        await virtual.stop();

        expect(liveAnnouncements.length).toBeGreaterThanOrEqual(3);
        expect(liveAnnouncements.some(a => a.includes("Second"))).toBe(true);
        expect(liveAnnouncements.some(a => a.includes("Third"))).toBe(true);
        expect(liveAnnouncements.some(a => a.includes("Fourth"))).toBe(true);
    });

    it("announces room transition messages added dynamically", async () => {
        const messages = [
            createMessage("initial", "You are in the starting room."),
        ];

        const { container, rerender } = render(<OutputWindow messages={[...messages]} />);
        const outputWindow = container.querySelector("#output_window");

        await virtual.start({ container: outputWindow as Element });
        await new Promise(r => setTimeout(r, 10));

        // Movement message
        messages.push(createMessage("move", "You head north."));
        await act(async () => {
            rerender(<OutputWindow messages={[...messages]} />);
        });
        await new Promise(r => setTimeout(r, 30));

        // Room description
        messages.push(createMessage("room", "# Anteroom\n\nAn empty room.", {
            presentationHint: "inset",
            groupId: "room-1",
            contentType: "text/djot",
            eventMetadata: { verb: "look", dobjName: "Anteroom" },
        }));
        await act(async () => {
            rerender(<OutputWindow messages={[...messages]} />);
        });
        await new Promise(r => setTimeout(r, 30));

        // Arrival message
        messages.push(createMessage("arrive", "You arrive from the south."));
        await act(async () => {
            rerender(<OutputWindow messages={[...messages]} />);
        });
        await new Promise(r => setTimeout(r, 30));

        const liveAnnouncements = getLiveAnnouncements(await virtual.spokenPhraseLog());
        await virtual.stop();

        const announcedText = liveAnnouncements.join(" ").toLowerCase();
        expect(announcedText).toContain("north");
        expect(announcedText).toContain("anteroom");
        expect(announcedText).toContain("south");
    });

    it("announces batched messages added in single render", async () => {
        const messages = [
            createMessage("1", "Initial message."),
        ];

        const { container, rerender } = render(<OutputWindow messages={[...messages]} />);
        const outputWindow = container.querySelector("#output_window");

        await virtual.start({ container: outputWindow as Element });
        await new Promise(r => setTimeout(r, 10));

        // Add multiple messages in a single rerender
        messages.push(createMessage("2", "Batch message alpha."));
        messages.push(createMessage("3", "Batch message beta."));
        messages.push(createMessage("4", "Batch message gamma."));

        await act(async () => {
            rerender(<OutputWindow messages={[...messages]} />);
        });
        await new Promise(r => setTimeout(r, 100));

        const liveAnnouncements = getLiveAnnouncements(await virtual.spokenPhraseLog());
        await virtual.stop();

        const announcedText = liveAnnouncements.join(" ").toLowerCase();
        expect(announcedText).toContain("alpha");
        expect(announcedText).toContain("beta");
        expect(announcedText).toContain("gamma");
    });
});
