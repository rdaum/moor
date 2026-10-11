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

import type { PropertyValue } from "@moor/schema/generated/moor-rpc/property-value";
import { Var } from "@moor/schema/generated/moor-var/var";
import { buildObjRefOffset, buildStructuredArgs, MoorVar, ObjectArgument } from "@moor/web-sdk";
import { Builder, ByteBuffer } from "flatbuffers";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ChangeServiceClient, discoverChangeService } from "./change-service";
import { getPropertyFlatBuffer, invokeVerbFlatBuffer } from "./rpc-fb";

vi.mock("./rpc-fb", () => ({ invokeVerbFlatBuffer: vi.fn(), getPropertyFlatBuffer: vi.fn() }));
const capabilities = {
    schema: 1,
    operations: ["packages", "configure", "upstream", "stage", "history", "discard"],
    fields: ["program"],
    choices: ["incoming", "local", "edited", "defer"],
    transports: ["upload", "http", "git"],
    default_package: "cowbell",
    max_source_bytes: 100,
    max_packages: 32,
    git_authorization: "wizard",
};
const packageValue = {
    schema: 1,
    generation: 2,
    objects: [{ oid: 12 }],
    constants: { ROOT: { oid: 1 } },
    trusted_owners: [{ oid: 2 }],
    upstream: "",
    active: 0,
};
const reply = (result: unknown) => vi.mocked(invokeVerbFlatBuffer).mockResolvedValueOnce({ result, output: [] });
const decode = (bytes: Uint8Array) => new MoorVar(Var.getRootAsVar(new ByteBuffer(bytes))).toJS();
beforeEach(() => vi.resetAllMocks());

describe("change service", () => {
    it("keeps proposed addresses typed and rejects stale package replies", async () => {
        reply({ ...capabilities, operations: [...capabilities.operations, "propose_objects"] });
        const client = await ChangeServiceClient.connect("token", "oid:44");
        const bindings = { HELPER: new ObjectArgument("oid:9600") };
        reply({ ...packageValue, proposed_bindings: { HELPER: { oid: 9600 } }, policy: { complete: [{ oid: 12 }] } });
        expect(await client.proposeObjects("test", 1, bindings, true)).toMatchObject({
            generation: 2,
            proposedBindings: { HELPER: "oid:9600" },
            complete: ["oid:12"],
        });
        expect(decode(vi.mocked(invokeVerbFlatBuffer).mock.lastCall![3]!)).toEqual(["test", 1, {
            HELPER: { oid: 9600 },
        }, 1]);
        reply({ ...packageValue, generation: 1 });
        await expect(client.proposeObjects("test", 1, bindings, false)).rejects.toThrow("Package changed");
    });
    it("keeps object arguments distinct from literal strings and maps", () => {
        expect(decode(buildStructuredArgs([new ObjectArgument("oid:12"), "oid:12", { curie: "oid:12" }]))).toEqual([
            { oid: 12 },
            "oid:12",
            { curie: "oid:12" },
        ]);
        expect(() => new ObjectArgument("#12")).toThrow();
        expect(() => new ObjectArgument("oid:12junk")).toThrow();
        expect(() => new ObjectArgument("oid:2147483648")).toThrow();
    });
    it("discovers the core binding and negotiates authenticated capabilities", async () => {
        const builder = new Builder();
        builder.finish(buildObjRefOffset(builder, "oid:44"));
        vi.mocked(getPropertyFlatBuffer).mockResolvedValueOnce(
            { value: () => Var.getRootAsVar(new ByteBuffer(builder.asUint8Array())) } as PropertyValue,
        );
        reply(capabilities);
        const client = await discoverChangeService("token");
        expect(getPropertyFlatBuffer).toHaveBeenCalledWith("token", "oid:0", "change_manager");
        expect(invokeVerbFlatBuffer).toHaveBeenCalledWith("token", "oid:44", "capabilities", expect.any(Uint8Array));
        expect(client?.provider).toBe("oid:44");
        expect(client?.supports("reload_object")).toBe(false);
    });
    it("does not invoke a service when the core omits its binding", async () => {
        vi.mocked(getPropertyFlatBuffer).mockResolvedValueOnce({ value: () => null } as unknown as PropertyValue);
        expect(await discoverChangeService("token")).toBeNull();
        expect(invokeVerbFlatBuffer).not.toHaveBeenCalled();
    });
    it("rejects unsupported schemas and operations before sending mutations", async () => {
        reply({ ...capabilities, schema: 2 });
        await expect(ChangeServiceClient.connect("token", "oid:44")).rejects.toThrow("schema");
        reply({ ...capabilities, operations: ["packages"] });
        const client = await ChangeServiceClient.connect("token", "oid:44");
        await expect(client.discard(1, 2)).rejects.toThrow("does not support discard");
        expect(invokeVerbFlatBuffer).toHaveBeenCalledTimes(2);
    });
    it("configures typed bindings and includes the expected configuration generation", async () => {
        reply(capabilities);
        const client = await ChangeServiceClient.connect("token", "oid:44");
        reply(packageValue);
        const configuration = {
            name: "test",
            generation: 1,
            objects: ["oid:12"],
            constants: { ROOT: new ObjectArgument("oid:1") },
            trustedOwners: ["oid:2"],
            upstream: "",
        };
        expect(await client.configure(configuration)).toMatchObject({ generation: 2, objects: ["oid:12"] });
        expect(decode(vi.mocked(invokeVerbFlatBuffer).mock.lastCall![3]!)).toEqual([
            "test",
            [{ oid: 12 }],
            { ROOT: { oid: 1 } },
            "",
            1,
            [{ oid: 2 }],
        ]);
    });
    it("checks UTF-8 upload size and refuses a response for a different package", async () => {
        reply(capabilities);
        const client = await ChangeServiceClient.connect("token", "oid:44");
        await expect(client.stage("test", [{ label: "x", text: "é".repeat(51) }], "update")).rejects.toThrow("limit");
        expect(invokeVerbFlatBuffer).toHaveBeenCalledTimes(1);
        reply({ schema: 1, package: "other", review_id: 1, generation: 1 });
        await expect(client.stage("test", [], "adopt")).rejects.toThrow("package");
    });
});
