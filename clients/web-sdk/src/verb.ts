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

import { CompileError } from "@moor/schema/generated/moor-common/compile-error";
import { SchedulerErrorUnion } from "@moor/schema/generated/moor-rpc/scheduler-error-union";
import { VerbCallError } from "@moor/schema/generated/moor-rpc/verb-call-error";
import { VerbCallResponse } from "@moor/schema/generated/moor-rpc/verb-call-response";
import { unionToVerbCallResponseUnion } from "@moor/schema/generated/moor-rpc/verb-call-response-union";
import { VerbCallSuccess } from "@moor/schema/generated/moor-rpc/verb-call-success";
import { VerbCompilationError } from "@moor/schema/generated/moor-rpc/verb-compilation-error";
import {
    unionToVerbProgramErrorUnion,
    VerbProgramErrorUnion,
} from "@moor/schema/generated/moor-rpc/verb-program-error-union";
import { VerbProgramFailure } from "@moor/schema/generated/moor-rpc/verb-program-failure";
import { VerbProgramResponseReply } from "@moor/schema/generated/moor-rpc/verb-program-response-reply";
import {
    unionToVerbProgramResponseUnion,
    VerbProgramResponseUnion,
} from "@moor/schema/generated/moor-rpc/verb-program-response-union";
import { VerbProgramSuccess } from "@moor/schema/generated/moor-rpc/verb-program-success";
import * as flatbuffers from "flatbuffers";

function replyTypeName(value: unknown): string {
    return (value as any)?.constructor?.name ?? typeof value;
}

export function parseVerbCallUnionFromBytes(
    bytes: Uint8Array,
    context: string,
): VerbCallSuccess | VerbCallError {
    const verbCallResponse = VerbCallResponse.getRootAsVerbCallResponse(
        new flatbuffers.ByteBuffer(bytes),
    );
    return parseVerbCallUnionFromResponse(verbCallResponse, context);
}

export function parseVerbCallUnionFromReply(
    replyUnion: unknown,
    context: string,
): VerbCallSuccess | VerbCallError {
    if (!(replyUnion instanceof VerbCallResponse)) {
        throw new Error(`${context}: unexpected reply type ${replyTypeName(replyUnion)}`);
    }
    return parseVerbCallUnionFromResponse(replyUnion, context);
}

export function parseVerbCallSuccessFromBytes(
    bytes: Uint8Array,
    context: string,
): VerbCallSuccess {
    return ensureVerbCallSuccess(parseVerbCallUnionFromBytes(bytes, context), context);
}

export function parseVerbCallSuccessFromReply(
    replyUnion: unknown,
    context: string,
): VerbCallSuccess {
    return ensureVerbCallSuccess(parseVerbCallUnionFromReply(replyUnion, context), context);
}

function parseVerbCallUnionFromResponse(
    verbCallResponse: VerbCallResponse,
    context: string,
): VerbCallSuccess | VerbCallError {
    const responseType = verbCallResponse.responseType();
    const responseUnion = unionToVerbCallResponseUnion(
        responseType,
        (obj: any) => verbCallResponse.response(obj),
    );

    if (!responseUnion) {
        throw new Error(`${context}: failed to parse verb call response`);
    }
    if (!(responseUnion instanceof VerbCallSuccess) && !(responseUnion instanceof VerbCallError)) {
        throw new Error(`${context}: unexpected verb call response union`);
    }
    return responseUnion;
}

function ensureVerbCallSuccess(
    responseUnion: VerbCallSuccess | VerbCallError,
    context: string,
): VerbCallSuccess {
    if (responseUnion instanceof VerbCallError) {
        const schedulerError = responseUnion.error();
        if (schedulerError) {
            const errorType = schedulerError.errorType();
            throw new Error(`${context}: ${SchedulerErrorUnion[errorType] ?? "unknown error"}`);
        }
        throw new Error(`${context}: unknown error`);
    }
    return responseUnion;
}

export function parseVerbProgramUnionFromReply(
    replyUnion: unknown,
    context: string,
): VerbProgramSuccess | VerbProgramFailure {
    if (!(replyUnion instanceof VerbProgramResponseReply)) {
        throw new Error(`${context}: unexpected reply type ${replyTypeName(replyUnion)}`);
    }

    const response = replyUnion.response();
    if (!response) {
        throw new Error(`${context}: missing verb program response`);
    }

    const responseType = response.responseType();
    const responseUnion = unionToVerbProgramResponseUnion(
        responseType,
        (obj: any) => response.response(obj),
    );
    if (!responseUnion) {
        throw new Error(`${context}: failed to parse verb program response union`);
    }
    if (!(responseUnion instanceof VerbProgramSuccess) && !(responseUnion instanceof VerbProgramFailure)) {
        throw new Error(`${context}: unexpected verb program response type ${responseType}`);
    }

    return responseUnion;
}

export type VerbProgramCompileOutcome =
    | { success: true }
    | {
        success: false;
        error: {
            type: VerbProgramErrorUnion | null;
            message: string;
            compileError?: CompileError;
        };
    };

export function parseVerbProgramCompileOutcome(
    replyUnion: unknown,
    context: string,
): VerbProgramCompileOutcome {
    const responseUnion = parseVerbProgramUnionFromReply(replyUnion, context);
    if (responseUnion instanceof VerbProgramSuccess) {
        return { success: true };
    }

    const programError = responseUnion.error();
    if (!programError) {
        return { success: false, error: { type: null, message: "Compilation failed" } };
    }

    const errorType = programError.errorType();
    switch (errorType) {
        case VerbProgramErrorUnion.VerbCompilationError: {
            const compError = unionToVerbProgramErrorUnion(
                errorType,
                (obj: any) => programError.error(obj),
            ) as VerbCompilationError | null;
            const compileError = compError?.error() ?? undefined;
            return {
                success: false,
                error: {
                    type: errorType,
                    message: compileError ? "Compilation error" : "Compilation failed",
                    compileError,
                },
            };
        }
        case VerbProgramErrorUnion.NoVerbToProgram:
            return { success: false, error: { type: errorType, message: "No verb to program" } };
        case VerbProgramErrorUnion.VerbPermissionDenied:
            return { success: false, error: { type: errorType, message: "Permission denied" } };
        case VerbProgramErrorUnion.VerbDatabaseError:
            return { success: false, error: { type: errorType, message: "Database error" } };
        default:
            return { success: false, error: { type: errorType, message: "Unknown compilation error" } };
    }
}
