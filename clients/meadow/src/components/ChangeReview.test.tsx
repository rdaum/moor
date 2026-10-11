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

import { Var } from "@moor/schema/generated/moor-var/var";
import { MoorVar } from "@moor/web-sdk";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ByteBuffer } from "flatbuffers";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { invokeVerbFlatBuffer } from "../lib/rpc-fb";
import { ChangeReview } from "./ChangeReview";

vi.mock("../lib/rpc-fb", () => ({ invokeVerbFlatBuffer: vi.fn() }));
vi.mock("../lib/monaco", () => ({}));
vi.mock("../lib/monaco-moo", () => ({ registerMooLanguage: vi.fn() }));
vi.mock("./ThemeProvider", () => ({ useTheme: () => ({ theme: "dark" }) }));
vi.mock("@monaco-editor/react", () => ({
    DiffEditor: ({ original, modified }: { original: string; modified: string }) => (
        <div data-testid="diff">{original} → {modified}</div>
    ),
    default: (
        { value, defaultValue, onChange, options }: {
            value?: string;
            defaultValue?: string;
            onChange?: (value: string) => void;
            options?: { readOnly?: boolean };
        },
    ) => (
        <textarea
            aria-label={options?.readOnly ? "Source" : "Proposed program"}
            readOnly={options?.readOnly}
            value={value}
            defaultValue={defaultValue}
            onChange={event => onChange?.(event.target.value)}
        />
    ),
}));

const target = { kind: "change", provider: "oid:2000", review: 1, generation: 1, row: "verb/program" } as const;
const baseRow = {
    id: target.row,
    label: "$test:verb",
    classification: "conflict",
    base: "base",
    live: "local",
    incoming: "incoming",
    live_text: "return 1;",
    incoming_text: "return 2;",
    eligible: true,
    blockers: [],
    choices: ["incoming", "local", "edited", "defer"],
    default: "unresolved",
    choice: {},
};
const defaultCapabilities = {
    schema: 1,
    operations: ["packages", "status", "review", "details", "inspection", "diagnostics", "resolve", "apply", "refresh"],
    fields: ["program"],
    choices: ["incoming", "local", "edited", "defer"],
    transports: ["upload"],
    default_package: "cowbell",
    max_source_bytes: 4194304,
    max_packages: 32,
    git_authorization: "wizard",
};
let capabilities = defaultCapabilities;
let generation: number;
let saved: Record<string, unknown>;
let rejectChoice: boolean;
let applied: boolean;
let reviewStatus: string;

beforeEach(() => {
    capabilities = defaultCapabilities;
    generation = 1;
    saved = {};
    rejectChoice = false;
    applied = false;
    reviewStatus = "ready";
    vi.mocked(invokeVerbFlatBuffer).mockReset().mockImplementation(async (_token, _provider, method, bytes) => {
        const args = new MoorVar(Var.getRootAsVar(new ByteBuffer(bytes!))).toJS() as unknown[];
        if (method === "capabilities") return { result: capabilities, output: [] };
        if (method === "diagnostics") {
            return { result: { schema: 1, review_id: 1, generation, diagnostics: [], total: 0, next: 0 }, output: [] };
        }
        const common = { schema: 1, review_id: 1, generation };
        if (method === "status") {
            return {
                result: { ...common, package: "cowbell", status: applied ? "complete" : reviewStatus, error: {} },
                output: [],
            };
        }
        if (args[1] !== generation) throw new Error("Review changed; reload its status.");
        if (method === "inspection") {
            return { result: { ...common, rows: [], counts: {}, revision: "view-1", next: 0 }, output: [] };
        }
        if (method === "review") {
            return {
                result: {
                    ...common,
                    operation: "update",
                    rows: [{ ...baseRow, choice: saved }],
                    cursor: [],
                    counts: { conflict: 1, unchanged: 100 },
                    decision_counts: { selected: saved.choice ? 1 : 0, unresolved: saved.choice ? 0 : 1, blocked: 0 },
                },
                output: [],
            };
        }
        if (method === "details") {
            if (rejectChoice) throw new Error("Live state changed. Refresh before choosing.");
            return { result: { ...common, row: { ...baseRow, choice: saved } }, output: [] };
        }
        if (method === "resolve") {
            if (rejectChoice) throw new Error("Live state changed. Refresh before choosing.");
            saved = { choice: args[3], program: args[4] };
            generation++;
            return { result: { ...common, generation, validation: [] }, output: [] };
        }
        if (method === "apply") {
            applied = true;
            return { result: { ...common, status: "applying" }, output: [] };
        }
        if (method === "refresh") {
            reviewStatus = "ready";
            saved = {};
            rejectChoice = false;
            generation++;
            return { result: { ...common, generation, status: "ready" }, output: [] };
        }
        throw new Error(`Unexpected method ${method}`);
    });
});
afterEach(cleanup);

const open = () => render(<ChangeReview visible target={target} authToken="token" onClose={vi.fn()} />);

it("opens a review-specific comparison and tells the truth about missing baseline source", async () => {
    open();
    expect(await screen.findByRole("heading", { name: "$test:verb" })).toBeTruthy();
    expect(screen.getByTestId("diff").textContent).toBe("return 1; → return 2;");
    expect(screen.getByText("Load the accepted source to compare all three versions.")).toBeTruthy();
    expect(screen.queryByRole("option", { name: "Accepted baseline → Local" })).toBeNull();
    expect(screen.getByRole("button", { name: "Apply 0 choices" })).toHaveProperty("disabled", true);
});

it("advances the generation after a choice and requires a separate explicit apply", async () => {
    open();
    fireEvent.click(await screen.findByRole("button", { name: "Keep local" }));
    await waitFor(() =>
        expect(screen.getByRole("button", { name: "Apply 1 choice" })).toHaveProperty("disabled", false)
    );
    expect(applied).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Apply 1 choice" }));
    expect(applied).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Confirm apply" }));
    await screen.findByText("Updates applied.");
    expect(applied).toBe(true);
});

it("keeps an edited resolution separate from the running verb and protects unsaved drafts", async () => {
    open();
    fireEvent.click(await screen.findByRole("button", { name: "Edit resolution" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Proposed program" }), {
        target: { value: "return 3;\nreturn 4;" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.getByText("Discard the unsaved draft?")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(screen.getByRole("button", { name: "Save proposed program" }));
    await waitFor(() => expect(saved.choice).toBe("edited"));
    expect(saved.program).toBe("return 3;\nreturn 4;");
    expect(applied).toBe(false);
});

it("requires confirmation to rebuild stale evidence and clear choices", async () => {
    open();
    fireEvent.click(await screen.findByRole("button", { name: "Keep local" }));
    await waitFor(() =>
        expect(screen.getByRole("button", { name: "Apply 1 choice" })).toHaveProperty("disabled", false)
    );
    rejectChoice = true;
    fireEvent.click(await screen.findByRole("button", { name: "Use upstream" }));
    await screen.findByText("Live state changed. Refresh before choosing.");
    expect(screen.getByRole("button", { name: "Apply 1 choice" })).toHaveProperty("disabled", true);
    fireEvent.click(screen.getByRole("button", { name: "Reload review" }));
    await screen.findByText("Live state changed. Refresh before choosing.");
    fireEvent.click(screen.getByRole("button", { name: "Refresh comparison" }));
    expect(saved.choice).toBe("local");
    expect(generation).toBe(2);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(saved.choice).toBe("local");
    fireEvent.click(screen.getByRole("button", { name: "Refresh comparison" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm refresh" }));
    await screen.findByRole("heading", { name: "$test:verb" });
    expect(saved).toEqual({});
    expect(generation).toBe(3);
    expect(screen.getByRole("button", { name: "Apply 0 choices" })).toHaveProperty("disabled", true);
    expect(applied).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Use upstream" }));
    await waitFor(() =>
        expect(screen.getByRole("button", { name: "Apply 1 choice" })).toHaveProperty("disabled", false)
    );
});

it("can retry a failed review read without closing the panel", async () => {
    vi.mocked(invokeVerbFlatBuffer).mockRejectedValueOnce(new Error("Permission denied."));
    open();
    expect((await screen.findByRole("alert")).textContent).toContain("Permission denied.");
    fireEvent.click(screen.getByRole("button", { name: "Reload review" }));
    await screen.findByRole("heading", { name: "$test:verb" });
});

it("browses local-only objects, verbs and properties without offering mutation choices", async () => {
    const items = [
        { id: "inspect/object", label: "$local", field: "object", live_text: "name: Local" },
        { id: "inspect/program", label: "$local:run", field: "program", live_text: "return 42;" },
        { id: "inspect/property", label: "$local.title", field: "property", live_text: "value: Local" },
    ].map(item => ({
        ...baseRow,
        ...item,
        classification: "local_only",
        read_only: true,
        live_present: true,
        incoming_present: false,
        incoming_text: "",
        eligible: false,
        choices: [],
    }));
    items.push({
        ...items[2],
        id: "inspect/edited",
        label: "$local.edited",
        classification: "local",
        incoming_present: true,
        incoming_text: "value: Original",
    });
    const program = { ...baseRow, classification: "local" };
    const common = { schema: 1, review_id: 1, generation: 1 };
    vi.mocked(invokeVerbFlatBuffer).mockImplementation(async (_token, _provider, method, bytes) => {
        const args = new MoorVar(Var.getRootAsVar(new ByteBuffer(bytes!))).toJS() as unknown[];
        if (method === "capabilities") return { result: capabilities, output: [] };
        if (method === "diagnostics") {
            return { result: { schema: 1, review_id: 1, generation, diagnostics: [], total: 0, next: 0 }, output: [] };
        }
        if (method === "status") return { result: { ...common, package: "cowbell", status: "ready" }, output: [] };
        if (method === "review") {
            return {
                result: {
                    ...common,
                    rows: args[3] === "changed" || args[3] === "local" ? [program] : [],
                    cursor: [],
                    counts: { local: 1 },
                    decision_counts: { selected: 0, unresolved: 0, blocked: 0 },
                },
                output: [],
            };
        }
        if (method === "inspection") {
            return {
                result: {
                    ...common,
                    rows: items.filter(item => !args[3] || item.classification === args[3]),
                    counts: { local_only: 3, local: 1 },
                    revision: "view-1",
                    next: 0,
                },
                output: [],
            };
        }
        if (method === "details") {
            return { result: { ...common, row: items.find(item => item.id === args[2]) ?? program }, output: [] };
        }
        throw new Error(`Inspection must not call ${method}`);
    });
    render(<ChangeReview visible target={{ ...target, row: undefined }} authToken="token" onClose={vi.fn()} />);
    await screen.findByRole("heading", { name: "$local" });
    fireEvent.click(screen.getByRole("button", { name: /\$local:run/ }));
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Source" })).toHaveProperty("value", "return 42;"));
    expect(screen.getByRole("textbox", { name: "Source" })).toHaveProperty("readOnly", true);
    fireEvent.click(screen.getByRole("button", { name: /\$local.title/ }));
    await waitFor(() =>
        expect(screen.getByRole("textbox", { name: "Source" })).toHaveProperty("value", "value: Local")
    );
    expect(screen.queryByRole("button", { name: "Use upstream" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Edit resolution" })).toBeNull();
    expect(screen.getByRole("button", { name: "Apply 0 choices" })).toHaveProperty("disabled", true);
    fireEvent.change(screen.getByRole("combobox", { name: "Show" }), { target: { value: "local" } });
    await screen.findByRole("heading", { name: "$local.edited" });
    expect(screen.getByRole("option", { name: "Local edits (2)" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /\$local.edited/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /\$test:verb/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /\$local:run/ })).toBeNull();
});

it("keeps unsaved text across reconnect and prevents browser unload", async () => {
    const view = open();
    fireEvent.click(await screen.findByRole("button", { name: "Edit resolution" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Proposed program" }), { target: { value: "return 99;" } });
    const unload = new Event("beforeunload", { cancelable: true });
    window.dispatchEvent(unload);
    expect(unload.defaultPrevented).toBe(true);
    generation++;
    view.rerender(<ChangeReview visible target={target} authToken="token" onClose={vi.fn()} connectionRevision={1} />);
    await screen.findByText(/Your unsaved text is retained/);
    expect(screen.getByRole("textbox", { name: "Proposed program" })).toHaveProperty("value", "return 99;");
    expect(screen.getByRole("button", { name: "Save proposed program" })).toHaveProperty("disabled", true);
});

it("reads the current saved draft on reconnect when there is no unsaved text", async () => {
    const view = open();
    await screen.findByRole("heading", { name: "$test:verb" });
    generation++;
    saved = { choice: "edited", program: "return 42;", validated: true };
    view.rerender(<ChangeReview visible target={target} authToken="token" onClose={vi.fn()} connectionRevision={1} />);
    await screen.findByText("Generation 2");
    await waitFor(() =>
        expect(screen.getByRole("button", { name: "Edit resolution" })).toHaveProperty("disabled", false)
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit resolution" }));
    expect(screen.getByRole("textbox", { name: "Proposed program" })).toHaveProperty("value", "return 42;");
});

it("does not offer unadvertised operations or call optional inspection APIs", async () => {
    capabilities = { ...defaultCapabilities, operations: ["packages", "status", "review", "details"], choices: [] };
    open();
    await screen.findByRole("heading", { name: "$test:verb" });
    expect(screen.queryByRole("button", { name: "Edit resolution" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Keep local" })).toBeNull();
    expect(screen.getByRole("button", { name: "Apply 0 choices" })).toHaveProperty("disabled", true);
    const methods = vi.mocked(invokeVerbFlatBuffer).mock.calls.map(call => call[2]);
    expect(methods).not.toContain("inspection");
    expect(methods).not.toContain("diagnostics");
});

it("offers explicit recovery for an interrupted apply without needing a failed read", async () => {
    reviewStatus = "interrupted";
    open();
    await screen.findByText("Apply was interrupted.");
    fireEvent.click(screen.getByRole("button", { name: "Refresh comparison" }));
    expect(generation).toBe(1);
    fireEvent.click(screen.getByRole("button", { name: "Confirm refresh" }));
    await screen.findByRole("heading", { name: "$test:verb" });
    expect(generation).toBe(2);
    expect(applied).toBe(false);
});
