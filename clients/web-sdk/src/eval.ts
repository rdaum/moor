// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Lesser General Public License as published by the Free Software Foundation,
// version 3 or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Lesser General Public License for more
// details.
//
// You should have received a copy of the GNU Lesser General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

import { ClientSuccess } from "@moor/schema/generated/moor-rpc/client-success";
import { unionToDaemonToClientReplyUnion } from "@moor/schema/generated/moor-rpc/daemon-to-client-reply-union";
import { EvalResult } from "@moor/schema/generated/moor-rpc/eval-result";
import { ReplyResult } from "@moor/schema/generated/moor-rpc/reply-result";
import { ReplyResultUnion, unionToReplyResultUnion } from "@moor/schema/generated/moor-rpc/reply-result-union";
import { Var } from "@moor/schema/generated/moor-var/var";
import * as flatbuffers from "flatbuffers";

import { extractFailureError } from "./errors";

/**
 * Parse a web-host /v1/eval FlatBuffer reply and return its Var result payload.
 */
export function parseEvalResultVar(bytes: Uint8Array): Var {
    const replyResult = ReplyResult.getRootAsReplyResult(
        new flatbuffers.ByteBuffer(bytes),
    );

    const resultType = replyResult.resultType();

    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, "Eval");
    }

    if (resultType !== ReplyResultUnion.ClientSuccess) {
        throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
    }

    const clientSuccess = unionToReplyResultUnion(
        resultType,
        (obj: any) => replyResult.result(obj),
    ) as ClientSuccess | null;

    if (!clientSuccess) {
        throw new Error("Failed to parse ClientSuccess");
    }

    const daemonReply = clientSuccess.reply();
    if (!daemonReply) {
        throw new Error("Missing daemon reply");
    }

    const replyType = daemonReply.replyType();
    const replyUnion = unionToDaemonToClientReplyUnion(
        replyType,
        (obj: any) => daemonReply.reply(obj),
    );

    if (!replyUnion) {
        throw new Error("Failed to parse reply union");
    }

    if (!(replyUnion instanceof EvalResult)) {
        throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
    }

    const varResult = (replyUnion as EvalResult).result();
    if (!varResult) {
        throw new Error("Missing result var");
    }

    return varResult;
}
