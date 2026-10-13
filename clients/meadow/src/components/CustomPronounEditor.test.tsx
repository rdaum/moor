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

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CustomPronouns } from "../lib/pronouns";
import { CustomPronounEditor } from "./CustomPronounEditor";

const initial: CustomPronouns = {
    subject: "ze",
    object: "zir",
    possessive_adjective: "zir",
    possessive_pronoun: "zirs",
    reflexive: "zirself",
    agreement: "is",
};

describe("custom pronoun editing", () => {
    it("requires a fresh preview of the exact draft before applying", async () => {
        const apply = vi.fn(async () => {});
        const preview = vi.fn(async (input: CustomPronouns) => [`${input.subject} ${input.agreement} ready.`]);
        render(<CustomPronounEditor initial={initial} onApply={apply} preview={preview} onCancel={vi.fn()} />);
        const use = screen.getByRole("button", { name: "Use these pronouns" }) as HTMLButtonElement;
        expect(use.disabled).toBe(true);
        fireEvent.click(screen.getByRole("button", { name: "Preview sentences" }));
        await screen.findByText("ze is ready.");
        expect(apply).not.toHaveBeenCalled();
        expect(use.disabled).toBe(false);
        fireEvent.change(screen.getByLabelText("The coat is ___."), { target: { value: "zirz" } });
        expect(use.disabled).toBe(true);
        fireEvent.change(screen.getByLabelText("Which sentence fits?"), { target: { value: "are" } });
        fireEvent.click(screen.getByRole("button", { name: "Preview sentences" }));
        await screen.findByText("ze are ready.");
        fireEvent.click(use);
        await waitFor(() =>
            expect(apply).toHaveBeenCalledExactlyOnceWith({
                ...initial,
                possessive_pronoun: "zirz",
                agreement: "are",
            })
        );
        expect(preview).toHaveBeenLastCalledWith({ ...initial, possessive_pronoun: "zirz", agreement: "are" });
    });

    it("does not save on cancellation or failed preview", async () => {
        const apply = vi.fn(async () => {});
        const cancel = vi.fn();
        render(
            <CustomPronounEditor
                initial={initial}
                onApply={apply}
                preview={vi.fn(async () => {
                    throw new Error("Preview unavailable.");
                })}
                onCancel={cancel}
            />,
        );
        fireEvent.click(screen.getByRole("button", { name: "Preview sentences" }));
        expect((await screen.findByRole("alert")).textContent).toBe("Preview unavailable.");
        expect((screen.getByRole("button", { name: "Use these pronouns" }) as HTMLButtonElement).disabled).toBe(true);
        fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
        expect(cancel).toHaveBeenCalledOnce();
        expect(apply).not.toHaveBeenCalled();
    });
});
