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
import { act, renderHook, waitFor } from "@testing-library/react";
import { ByteBuffer } from "flatbuffers";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CustomPronouns } from "../lib/pronouns";
import { invokeVerbFlatBuffer } from "../lib/rpc-fb";
import { usePronouns } from "./usePronouns";

vi.mock("../lib/rpc-fb", () => ({ invokeVerbFlatBuffer: vi.fn() }));

const custom: CustomPronouns = {
    subject: "ze",
    object: "zir",
    possessive_adjective: "zir",
    possessive_pronoun: "zirs",
    reflexive: "zirself",
    agreement: "is",
};

describe("pronoun client calls", () => {
    beforeEach(() => {
        vi.mocked(invokeVerbFlatBuffer).mockReset();
        vi.mocked(invokeVerbFlatBuffer).mockImplementation(async (_token, _object, verb) => ({
            result: verb === "list_presets"
                ? ["they/them", "she/her"]
                : verb === "pronouns_display"
                ? "ze/zir"
                : verb === "pronouns_settings"
                ? custom
                : verb === "examples"
                ? ["Ze is ready."]
                : null,
        } as Awaited<ReturnType<typeof invokeVerbFlatBuffer>>));
    });

    it("loads the custom forms and sends literal fields to preview and the shared setter", async () => {
        const { result } = renderHook(() => usePronouns("token", "oid:42"));
        await waitFor(() => expect(result.current.customPronouns).toEqual(custom));
        expect(result.current.currentPronouns).toBe("ze/zir");
        await act(async () => {
            expect(await result.current.previewPronouns(custom)).toEqual(["Ze is ready."]);
        });
        expect(vi.mocked(invokeVerbFlatBuffer).mock.calls.some(call => call[2] === "set_pronouns")).toBe(false);
        await act(async () => {
            await result.current.updatePronouns(custom);
        });
        for (const verb of ["examples", "set_pronouns"]) {
            const call = vi.mocked(invokeVerbFlatBuffer).mock.calls.find(call => call[2] === verb)!;
            const argument = new MoorVar(Var.getRootAsVar(new ByteBuffer(call[3] as Uint8Array))).toJS();
            expect(argument).toEqual([custom]);
        }
    });

    it("keeps presets available for a core without custom entry", async () => {
        vi.mocked(invokeVerbFlatBuffer).mockImplementation(async (_token, _object, verb) => {
            if (verb === "pronouns_settings") throw new Error("No such verb");
            return { result: verb === "list_presets" ? ["they/them"] : "they/them" } as Awaited<
                ReturnType<typeof invokeVerbFlatBuffer>
            >;
        });
        const { result } = renderHook(() => usePronouns("token", "oid:42"));
        await waitFor(() => expect(result.current.pronounsAvailable).toBe(true));
        expect(result.current.customPronouns).toBeNull();
        expect(result.current.availablePresets).toEqual(["they/them"]);
    });
});
