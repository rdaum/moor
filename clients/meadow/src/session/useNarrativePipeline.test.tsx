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

import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { NarrativeRef } from "../components/Narrative";
import { createEditorLaunchBridge } from "./editorLaunchBridge";
import { useNarrativePipeline } from "./useNarrativePipeline";

const presentations = vi.hoisted(() => ({ add: vi.fn(), remove: vi.fn() }));

vi.mock("../context/PresentationContext", () => ({
    usePresentationContext: () => ({
        addPresentation: presentations.add,
        removePresentation: presentations.remove,
    }),
}));

vi.mock("../lib/auth-session", () => ({
    readReconnectCredentials: vi.fn(() => null),
}));

describe("useNarrativePipeline buffering", () => {
    it("keeps its ref callback stable and flushes the current buffer once", () => {
        const bridge = createEditorLaunchBridge();
        const { result } = renderHook(() => useNarrativePipeline(bridge));
        const callback = result.current.narrativeCallbackRef;

        act(() => {
            result.current.handlers.handleNarrativeMessage("buffered message", undefined, "text/plain");
        });

        expect(result.current.narrativeCallbackRef).toBe(callback);

        const addNarrativeContent = vi.fn<NarrativeRef["addNarrativeContent"]>();
        const narrative = { addNarrativeContent } as unknown as NarrativeRef;
        act(() => callback(narrative));

        expect(addNarrativeContent).toHaveBeenCalledTimes(1);
        expect(addNarrativeContent).toHaveBeenCalledWith(
            "buffered message",
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
            undefined,
        );

        act(() => {
            callback(null);
            callback(narrative);
        });
        expect(addNarrativeContent).toHaveBeenCalledTimes(1);
    });
    it("buffers annotation tables with their owning event", () => {
        const { result } = renderHook(() => useNarrativePipeline(createEditorLaunchBridge()));
        const annotations = { a1: { kind: "object" as const, ref: "oid:47" } };
        act(() =>
            result.current.handlers.handleNarrativeMessage(
                "[Compass]{annotation=a1}",
                undefined,
                "text/djot",
                undefined,
                undefined,
                undefined,
                undefined,
                undefined,
                undefined,
                undefined,
                { annotations },
            )
        );
        const addNarrativeContent = vi.fn<NarrativeRef["addNarrativeContent"]>();
        act(() => result.current.narrativeCallbackRef({ addNarrativeContent } as unknown as NarrativeRef));
        expect(addNarrativeContent.mock.calls[0][7]).toBeUndefined();
        expect(addNarrativeContent.mock.calls[0][8]).toEqual({ annotations });
    });
});

it("buffers messages during recovery without requiring a render between receipt and retry", () => {
    const bridge = createEditorLaunchBridge();
    const { result } = renderHook(() => useNarrativePipeline(bridge, "owner"));
    const addNarrativeContent = vi.fn<NarrativeRef["addNarrativeContent"]>();
    const narrative = { addNarrativeContent } as unknown as NarrativeRef;
    act(() => {
        result.current.narrativeCallbackRef(narrative);
        result.current.narrativeCallbackRef(null);
        result.current.handlers.handleNarrativeMessage("during recovery", undefined, "text/plain");
        result.current.narrativeCallbackRef(narrative);
        result.current.narrativeCallbackRef(null);
        result.current.narrativeCallbackRef(narrative);
    });
    expect(addNarrativeContent).toHaveBeenCalledOnce();
    expect(addNarrativeContent.mock.calls[0][0]).toBe("during recovery");
});
it.each([null, "other-owner"])("drops buffered output when the history owner becomes %s during recovery", owner => {
    const bridge = createEditorLaunchBridge();
    const { result, rerender } = renderHook(
        ({ owner }: { owner: string | null }) => useNarrativePipeline(bridge, owner),
        { initialProps: { owner: "first-owner" as string | null } },
    );
    act(() => result.current.handlers.handleNarrativeMessage("private old output"));
    rerender({ owner });
    act(() => result.current.handlers.handleNarrativeMessage("current output"));
    const addNarrativeContent = vi.fn<NarrativeRef["addNarrativeContent"]>();
    act(() => result.current.narrativeCallbackRef({ addNarrativeContent } as unknown as NarrativeRef));
    expect(addNarrativeContent.mock.calls.map(call => call[0])).toEqual(["current output"]);
});

it("does not replay successfully flushed messages when a later append fails during retry", () => {
    const bridge = createEditorLaunchBridge();
    const { result } = renderHook(() => useNarrativePipeline(bridge, "owner"));
    act(() => {
        result.current.handlers.handleNarrativeMessage("accepted");
        result.current.handlers.handleNarrativeMessage("try again");
    });
    const addNarrativeContent = vi.fn<NarrativeRef["addNarrativeContent"]>()
        .mockImplementationOnce(() => {})
        .mockImplementationOnce(() => {
            throw new Error("append failed");
        });
    const narrative = { addNarrativeContent } as unknown as NarrativeRef;
    expect(() => result.current.narrativeCallbackRef(narrative)).toThrow("append failed");
    act(() => result.current.narrativeCallbackRef(narrative));
    expect(addNarrativeContent.mock.calls.map(call => call[0])).toEqual(["accepted", "try again", "try again"]);
});

it("historical room narrative cannot reopen or replace the current room panel", () => {
    presentations.add.mockClear();
    presentations.remove.mockClear();
    const { result } = renderHook(() => useNarrativePipeline(createEditorLaunchBridge()));
    const addNarrativeContent = vi.fn<NarrativeRef["addNarrativeContent"]>();
    act(() => {
        result.current.narrativeCallbackRef({ addNarrativeContent } as unknown as NarrativeRef);
        result.current.handlers.handleDataMessage({
            namespace: "state",
            eventKind: "room_snapshot",
            timestamp: "2026-10-12T00:00:00Z",
            payload: {
                room: 20,
                title: "Current room",
                content: "<p>Now</p>",
                content_type: "text/html",
                annotations: {},
            },
        });
    });
    const current = presentations.add.mock.calls[0][0];
    act(() =>
        result.current.handlers.handleNarrativeMessage(
            "Old room",
            undefined,
            "text/plain",
            true,
            undefined,
            "inset",
            "look_10",
            undefined,
            undefined,
            undefined,
            { lookKind: "room", lookRoom: "oid:10" },
        )
    );
    expect(addNarrativeContent.mock.calls.at(-1)?.[0]).toBe("Old room");
    expect(presentations.add).toHaveBeenCalledOnce();
    expect(presentations.add.mock.calls[0][0]).toBe(current);
    expect(presentations.remove).not.toHaveBeenCalled();
});
