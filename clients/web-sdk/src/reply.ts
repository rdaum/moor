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
import { unionToDaemonToHostReplyUnion } from "@moor/schema/generated/moor-rpc/daemon-to-host-reply-union";
import { HostSuccess } from "@moor/schema/generated/moor-rpc/host-success";
import { ReplyResult } from "@moor/schema/generated/moor-rpc/reply-result";
import { ReplyResultUnion, unionToReplyResultUnion } from "@moor/schema/generated/moor-rpc/reply-result-union";
import * as flatbuffers from "flatbuffers";

import { MoorApiError } from "./api-client";
import { extractFailureError } from "./errors";

function parseReplyResult(bytes: Uint8Array): ReplyResult {
    return ReplyResult.getRootAsReplyResult(new flatbuffers.ByteBuffer(bytes));
}

function replyTypeName(value: unknown): string {
    return (value as any)?.constructor?.name ?? typeof value;
}

export function parseClientReplyUnion(bytes: Uint8Array, context: string): unknown {
    const replyResult = parseReplyResult(bytes);
    const resultType = replyResult.resultType();

    if (resultType === ReplyResultUnion.NONE) {
        throw new MoorApiError("decode", "Empty or invalid FlatBuffer response", { context });
    }

    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, context);
    }

    if (resultType !== ReplyResultUnion.ClientSuccess) {
        throw new MoorApiError("protocol", `Unexpected result type: ${ReplyResultUnion[resultType]}`, { context });
    }

    const clientSuccess = unionToReplyResultUnion(
        resultType,
        (obj: any) => replyResult.result(obj),
    ) as ClientSuccess | null;

    if (!clientSuccess) {
        throw new MoorApiError("decode", "Failed to parse ClientSuccess", { context });
    }

    const daemonReply = clientSuccess.reply();
    if (!daemonReply) {
        throw new MoorApiError("decode", "Missing daemon reply", { context });
    }

    const replyType = daemonReply.replyType();
    const replyUnion = unionToDaemonToClientReplyUnion(
        replyType,
        (obj: any) => daemonReply.reply(obj),
    );

    if (!replyUnion) {
        throw new MoorApiError("decode", "Failed to parse reply union", { context });
    }

    return replyUnion;
}

export function parseClientReplyAs<T>(
    bytes: Uint8Array,
    context: string,
    ctor: new(...args: any[]) => T,
): T {
    const replyUnion = parseClientReplyUnion(bytes, context);
    if (!(replyUnion instanceof ctor)) {
        throw new MoorApiError("protocol", `${context}: unexpected reply type ${replyTypeName(replyUnion)}`, {
            context,
        });
    }
    return replyUnion as T;
}

export function parseHostReplyUnion(bytes: Uint8Array, context: string): unknown {
    const replyResult = parseReplyResult(bytes);
    const resultType = replyResult.resultType();

    if (resultType === ReplyResultUnion.NONE) {
        throw new MoorApiError("decode", "Empty or invalid FlatBuffer response", { context });
    }

    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, context);
    }

    if (resultType !== ReplyResultUnion.HostSuccess) {
        throw new MoorApiError("protocol", `Unexpected result type: ${ReplyResultUnion[resultType]}`, { context });
    }

    const hostSuccess = unionToReplyResultUnion(
        resultType,
        (obj: any) => replyResult.result(obj),
    ) as HostSuccess | null;

    if (!hostSuccess) {
        throw new MoorApiError("decode", "Missing host success payload", { context });
    }

    const daemonReply = hostSuccess.reply();
    if (!daemonReply) {
        throw new MoorApiError("decode", "Missing host reply payload", { context });
    }

    const replyType = daemonReply.replyType();
    const replyUnion = unionToDaemonToHostReplyUnion(
        replyType,
        (obj: any) => daemonReply.reply(obj),
    );

    if (!replyUnion) {
        throw new MoorApiError("decode", "Failed to parse host reply union", { context });
    }

    return replyUnion;
}
