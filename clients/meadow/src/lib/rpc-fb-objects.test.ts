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

import { CompileContext } from "@moor/schema/generated/moor-common/compile-context";
import { CompileError } from "@moor/schema/generated/moor-common/compile-error";
import { CompileErrorUnion } from "@moor/schema/generated/moor-common/compile-error-union";
import { ParseError } from "@moor/schema/generated/moor-common/parse-error";
import { ClientSuccess } from "@moor/schema/generated/moor-rpc/client-success";
import { DaemonToClientReply } from "@moor/schema/generated/moor-rpc/daemon-to-client-reply";
import { DaemonToClientReplyUnion } from "@moor/schema/generated/moor-rpc/daemon-to-client-reply-union";
import { ReplyResult } from "@moor/schema/generated/moor-rpc/reply-result";
import { ReplyResultUnion } from "@moor/schema/generated/moor-rpc/reply-result-union";
import { VerbCompilationError } from "@moor/schema/generated/moor-rpc/verb-compilation-error";
import { VerbProgramError } from "@moor/schema/generated/moor-rpc/verb-program-error";
import { VerbProgramErrorUnion } from "@moor/schema/generated/moor-rpc/verb-program-error-union";
import { VerbProgramFailure } from "@moor/schema/generated/moor-rpc/verb-program-failure";
import { VerbProgramResponse } from "@moor/schema/generated/moor-rpc/verb-program-response";
import { VerbProgramResponseReply } from "@moor/schema/generated/moor-rpc/verb-program-response-reply";
import { VerbProgramResponseUnion } from "@moor/schema/generated/moor-rpc/verb-program-response-union";
import { Builder } from "flatbuffers";
import { afterEach, expect, it, vi } from "vitest";
import { compileVerbFlatBuffer } from "./rpc-fb-objects";
import { moorApi } from "./rpc-fb-shared";

afterEach(() => vi.restoreAllMocks());

it("keeps compiler diagnostics on HTTP 422 and rejects untyped HTTP failures", async () => {
    const b = new Builder();
    const message = b.createString("expected expression");
    const context = b.createString("parser");
    const position = CompileContext.createCompileContext(b, 1n, 9n);
    const parseError = ParseError.createParseError(b, position, context, 0n, 0n, false, message, 0n, 0n, false, 0, 0);
    const compileError = CompileError.createCompileError(b, CompileErrorUnion.ParseError, parseError);
    const error = VerbProgramError.createVerbProgramError(
        b,
        VerbProgramErrorUnion.VerbCompilationError,
        VerbCompilationError.createVerbCompilationError(b, compileError),
    );
    const outcome = VerbProgramResponse.createVerbProgramResponse(
        b,
        VerbProgramResponseUnion.VerbProgramFailure,
        VerbProgramFailure.createVerbProgramFailure(b, error),
    );
    const reply = DaemonToClientReply.createDaemonToClientReply(
        b,
        DaemonToClientReplyUnion.VerbProgramResponseReply,
        VerbProgramResponseReply.createVerbProgramResponseReply(b, outcome),
    );
    b.finish(
        ReplyResult.createReplyResult(b, ReplyResultUnion.ClientSuccess, ClientSuccess.createClientSuccess(b, reply)),
    );
    const request = vi.spyOn(moorApi, "request").mockResolvedValueOnce(
        new Response(new Uint8Array(b.asUint8Array()), {
            status: 422,
            headers: { "content-type": "application/x-flatbuffers" },
        }),
    );
    const result = await compileVerbFlatBuffer("token", "oid:1", "example", "return (;");
    expect(result.success).toBe(false);
    if (!result.success) {
        expect(result.error).toBeInstanceOf(CompileError);
        expect((result.error as CompileError).error(new ParseError())?.message()).toBe("expected expression");
    }
    request.mockResolvedValueOnce(new Response("proxy error", { status: 502 }));
    await expect(compileVerbFlatBuffer("token", "oid:1", "example", "return 1;")).rejects.toMatchObject({
        status: 502,
    });
});
