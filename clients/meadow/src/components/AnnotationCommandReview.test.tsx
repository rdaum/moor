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

import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { AnnotationCommandReview } from "./AnnotationCommandReview";

describe("annotation command review", () => {
    it("returns focus to the exit link when cancelled", async () => {
        const submit = vi.fn(() => true);
        function Harness() {
            const [open, setOpen] = useState(false);
            return (
                <>
                    <button onClick={() => setOpen(true)}>examples</button>
                    {open && (
                        <AnnotationCommandReview
                            annotation={{ kind: "command", command: "go examples" }}
                            position={{ x: 0, y: 0 }}
                            onSubmit={submit}
                            onClose={() => setOpen(false)}
                        />
                    )}
                </>
            );
        }
        render(<Harness />);
        const exit = screen.getByRole("button", { name: "examples" });
        act(() => exit.focus());
        fireEvent.click(exit);
        fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
        await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
        expect(document.activeElement).toBe(exit);
        expect(submit).not.toHaveBeenCalled();
    });

    it("retains the captured exit identity when submitting a command", async () => {
        const submit = vi.fn(() => true);
        const annotation = {
            kind: "command" as const,
            command: "go east",
            exit: { source: "oid:12", destination: "oid:13", passage: "original-door" },
        };
        render(
            <AnnotationCommandReview
                annotation={annotation}
                position={{ x: 0, y: 0 }}
                onSubmit={submit}
                onClose={vi.fn()}
            />,
        );
        fireEvent.click(screen.getByRole("button", { name: "Go" }));
        await waitFor(() => expect(submit).toHaveBeenCalledExactlyOnceWith("go east", annotation));
    });

    it("sends exactly the supplied command only on confirmation", async () => {
        const submit = vi.fn(() => true);
        const close = vi.fn();
        render(
            <AnnotationCommandReview
                position={{ x: 120, y: 180 }}
                annotation={{ kind: "command", command: "go e" }}
                onSubmit={submit}
                onClose={close}
            />,
        );
        expect(submit).not.toHaveBeenCalled();
        expect(screen.getByRole("dialog").getAttribute("aria-modal")).toBe("false");
        fireEvent.click(screen.getByRole("button", { name: "Run command" }));
        await waitFor(() => expect(submit).toHaveBeenCalledExactlyOnceWith("go e"));
        expect(close).toHaveBeenCalledOnce();
    });
    it("cancels without submitting and reports a server prompt refusal", async () => {
        const submit = vi.fn(() => {
            throw new Error("Finish the current prompt.");
        });
        const close = vi.fn();
        render(
            <AnnotationCommandReview
                position={{ x: 120, y: 180 }}
                annotation={{ kind: "command", command: "take #47" }}
                onSubmit={submit}
                onClose={close}
            />,
        );
        fireEvent.click(screen.getByRole("button", { name: "Run command" }));
        await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Finish the current prompt."));
        expect(close).not.toHaveBeenCalled();
        fireEvent.click(screen.getByRole("button", { name: "Close command" }));
        expect(submit).toHaveBeenCalledTimes(1);
        expect(close).toHaveBeenCalledOnce();
    });
    it("collects both slots without submitting, then uses the exact template", async () => {
        const submit = vi.fn(() => true);
        render(
            <AnnotationCommandReview
                position={{ x: 120, y: 180 }}
                annotation={{
                    kind: "command",
                    template: "write {dobj} on {iobj}",
                    arguments: {
                        dobj: { label: "Words", expectedKind: "text" },
                        iobj: { label: "Surface", expectedKind: "text" },
                    },
                }}
                onSubmit={submit}
                onClose={vi.fn()}
            />,
        );
        expect(screen.getByRole("dialog").getAttribute("aria-modal")).toBe("false");
        fireEvent.change(screen.getByLabelText("Surface"), { target: { value: "wall" } });
        fireEvent.change(screen.getByLabelText("Words"), { target: { value: "hello" } });
        expect(submit).not.toHaveBeenCalled();
        fireEvent.click(screen.getByRole("button", { name: "Run command" }));
        await waitFor(() => expect(submit).toHaveBeenCalledExactlyOnceWith("write hello on wall"));
    });
    it("keeps free-text arguments free of object suggestions", async () => {
        const submit = vi.fn(() => true);
        render(
            <AnnotationCommandReview
                position={{ x: 120, y: 180 }}
                annotation={{
                    kind: "command",
                    template: "say {dobj}",
                    arguments: { dobj: { label: "Message", expectedKind: "text" } },
                }}
                onSubmit={submit}
                onClose={vi.fn()}
            />,
        );
        const input = screen.getByRole("textbox", { name: "Message" });
        fireEvent.focus(input);
        fireEvent.change(input, { target: { value: "hello everyone" } });
        expect(screen.queryByRole("combobox")).toBeNull();
        expect(screen.queryByRole("listbox")).toBeNull();
        expect(screen.queryByText("Choose from suggestions or the transcript.")).toBeNull();
        fireEvent.click(screen.getByRole("button", { name: "Run command" }));
        await waitFor(() => expect(submit).toHaveBeenCalledExactlyOnceWith("say hello everyone"));
    });
    it("uses authored action identity without deriving it from parser text", async () => {
        const submit = vi.fn(() => true);
        render(
            <AnnotationCommandReview
                position={{ x: 120, y: 180 }}
                annotation={{
                    kind: "command",
                    command: "hand #47 at #45",
                    action: { icon: "give", label: "Give", title: "Give to Mr. Welcome" },
                }}
                onSubmit={submit}
                onClose={vi.fn()}
            />,
        );
        expect(screen.getByRole("dialog", { name: "Give to Mr. Welcome" })).toBeTruthy();
        expect(screen.getByRole("heading").querySelector("svg")).toBeTruthy();
        expect(submit).not.toHaveBeenCalled();
        fireEvent.click(screen.getByRole("button", { name: "Give" }));
        await waitFor(() => expect(submit).toHaveBeenCalledExactlyOnceWith("hand #47 at #45"));
    });
});
