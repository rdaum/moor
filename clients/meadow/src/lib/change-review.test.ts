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
import { buildStructuredArgs, decodeAnnotations, MoorVar } from "@moor/web-sdk";
import { ByteBuffer } from "flatbuffers";
import { describe, expect, it, vi } from "vitest";
import { baselineText, ChangeReviewClient, decodeChangeRow } from "./change-review";
import { invokeVerbFlatBuffer } from "./rpc-fb";

vi.mock("./rpc-fb", () => ({ invokeVerbFlatBuffer: vi.fn() }));
const target = { kind: "change", provider: "oid:2000", review: 1, generation: 7 } as const;
const row = {
    id: "verb/program",
    label: "$test:verb",
    classification: "conflict",
    eligible: true,
    blockers: [],
    choices: ["incoming", "local", "edited", "defer"],
    default: "unresolved",
    choice: {},
    base: "old-hash",
    live: "local-hash",
    incoming: "upstream-hash",
    live_text: "return 1;",
    incoming_text: "return 2;",
};

describe("change review protocol", () => {
    it("preserves typed generations, cursors and literal multiline programs on the wire", () => {
        const args = [1, 7, [1, 7, 51], "return `$x ! ANY';\nreturn \"`$foo`\";"];
        const decoded = new MoorVar(Var.getRootAsVar(new ByteBuffer(buildStructuredArgs(args))));
        expect(decoded.toJS()).toEqual(args);
        expect(() => buildStructuredArgs([Number.MAX_SAFE_INTEGER + 1])).toThrow();
    });
    it("validates review links without interpreting commands or object-browser references", () => {
        expect(decodeAnnotations({ a1: target })).toEqual({ a1: target });
        for (const patch of [{ review: 0 }, { generation: 1.5 }, { provider: "#2000" }, { row: "x\ny" }]) {
            expect(decodeAnnotations({ a1: { ...target, ...patch } })).toEqual({});
        }
    });
    it("only reconstructs an accepted source when a program hash matches", () => {
        const detail = decodeChangeRow(row, true);
        expect(baselineText(detail)).toBeUndefined();
        expect(baselineText({ ...detail, base: detail.live })).toBe("return 1;");
        expect(baselineText({ ...detail, base: detail.incoming })).toBe("return 2;");
        expect(baselineText({ ...detail, base: undefined })).toBeUndefined();
    });
    it("rejects a response from another review generation before exposing its source", async () => {
        vi.mocked(invokeVerbFlatBuffer).mockResolvedValueOnce({
            result: { schema: 1, review_id: 1, generation: 8, row },
            output: [],
        });
        await expect(new ChangeReviewClient("token", target).details(7, row.id)).rejects.toThrow("Review changed");
    });
    it("sends edited choices through the guarded service, without evaluating the program", async () => {
        vi.mocked(invokeVerbFlatBuffer).mockResolvedValueOnce({
            result: { schema: 1, review_id: 1, generation: 8, validation: [] },
            output: [],
        });
        const program = "return 9;\n// $wiz_features:verb()";
        await new ChangeReviewClient("token", target).resolve(7, row.id, "edited", program);
        const call = vi.mocked(invokeVerbFlatBuffer).mock.lastCall!;
        expect(call.slice(0, 3)).toEqual(["token", "oid:2000", "resolve"]);
        expect(new MoorVar(Var.getRootAsVar(new ByteBuffer(call[3]!))).toJS()).toEqual([
            1,
            7,
            row.id,
            "edited",
            program,
        ]);
    });
});

it("rejects diagnostics from another generation", async () => {
    vi.mocked(invokeVerbFlatBuffer).mockResolvedValueOnce({
        result: { schema: 1, review_id: 1, generation: 8, diagnostics: [], next: 0, total: 0 },
        output: [],
    });
    await expect(new ChangeReviewClient("token", target).diagnostics(7)).rejects.toThrow("Review changed");
});

it("decodes saved compiler ranges and completed outcomes without reconstructing source", async () => {
    const detail = decodeChangeRow({
        ...row,
        validation: [{ message: "Invalid expression", line: 2, column: 4, end_line: 2, end_column: 5 }],
    }, true);
    expect(detail.validation).toEqual([{
        message: "Invalid expression",
        startLineNumber: 2,
        startColumn: 4,
        endLineNumber: 2,
        endColumn: 5,
    }]);
    vi.mocked(invokeVerbFlatBuffer).mockResolvedValueOnce({
        result: {
            schema: 1,
            review_id: 1,
            generation: 7,
            package: "cowbell",
            status: "complete",
            provenance: { commit: "sha1:abc" },
            completed_at: 10,
            decision_count: 1,
            decisions: [{ id: "row", label: "#1:run", choice: "edited", accepted: "hash" }],
        },
        output: [],
    });
    expect(await new ChangeReviewClient("token", target).status()).toMatchObject({
        provenance: { commit: "sha1:abc" },
        receipt: { decisionCount: 1, decisions: [{ label: "#1:run", choice: "edited" }] },
    });
});
