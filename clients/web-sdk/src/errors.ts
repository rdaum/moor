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

import { CompileErrorUnion, unionToCompileErrorUnion } from "@moor/schema/generated/moor-common/compile-error-union";
import { ErrorCode } from "@moor/schema/generated/moor-common/error-code";
import { ParseError } from "@moor/schema/generated/moor-common/parse-error";
import { CompilationError } from "@moor/schema/generated/moor-rpc/compilation-error";
import { Failure } from "@moor/schema/generated/moor-rpc/failure";
import { ReplyResult } from "@moor/schema/generated/moor-rpc/reply-result";
import { unionToReplyResultUnion } from "@moor/schema/generated/moor-rpc/reply-result-union";
import { SchedulerErrorUnion, unionToSchedulerErrorUnion } from "@moor/schema/generated/moor-rpc/scheduler-error-union";
import { TaskAbortedException } from "@moor/schema/generated/moor-rpc/task-aborted-exception";

export function extractFailureError(replyResult: ReplyResult, context: string): never {
    const resultType = replyResult.resultType();
    const failure = unionToReplyResultUnion(
        resultType,
        (obj: any) => replyResult.result(obj),
    ) as Failure | null;

    if (!failure) {
        throw new Error(`${context} failed with unknown error`);
    }

    const rpcError = failure.error();
    if (!rpcError) {
        throw new Error(`${context} failed with unknown error`);
    }

    const rawMessage = rpcError.message() || "Unknown error";
    const schedulerError = rpcError.schedulerError();

    if (schedulerError) {
        const schedulerErrorType = schedulerError.errorType();
        const errorUnion = unionToSchedulerErrorUnion(
            schedulerErrorType,
            (obj: any) => schedulerError.error(obj),
        );

        if (schedulerErrorType === SchedulerErrorUnion.CompilationError && errorUnion) {
            const compilationError = errorUnion as CompilationError;
            const compileError = compilationError.error();
            if (compileError) {
                const compileErrorType = compileError.errorType();
                const errorDetail = unionToCompileErrorUnion(
                    compileErrorType,
                    (obj: any) => compileError.error(obj),
                );

                if (compileErrorType === CompileErrorUnion.ParseError && errorDetail) {
                    const parseError = errorDetail as ParseError;
                    const message = parseError.message();
                    const parseContext = parseError.context();
                    const position = parseError.errorPosition();

                    if (message && position) {
                        throw new Error(
                            `${parseContext} failed: Parse error at line ${position.line()}, col ${position.col()}: ${message}`,
                        );
                    }
                    if (message) {
                        throw new Error(`${parseContext} failed: ${message}`);
                    }
                }

                throw new Error(`${context} failed: Compilation error (${CompileErrorUnion[compileErrorType]})`);
            }
        }

        if (schedulerErrorType === SchedulerErrorUnion.TaskAbortedException && errorUnion) {
            const taskException = errorUnion as TaskAbortedException;
            const exception = taskException.exception();

            if (exception) {
                const error = exception.error();
                if (error) {
                    const errType = error.errType();
                    const customMsg = error.msg();

                    const errorMessages: Record<number, string> = {
                        [ErrorCode.E_NONE]: "No error",
                        [ErrorCode.E_TYPE]: "Type mismatch",
                        [ErrorCode.E_DIV]: "Division by zero",
                        [ErrorCode.E_PERM]: "Permission denied",
                        [ErrorCode.E_PROPNF]: "Property not found",
                        [ErrorCode.E_VERBNF]: "Verb not found",
                        [ErrorCode.E_VARNF]: "Variable not found",
                        [ErrorCode.E_INVIND]: "Invalid indirection",
                        [ErrorCode.E_RECMOVE]: "Recursive move",
                        [ErrorCode.E_MAXREC]: "Too many verb calls",
                        [ErrorCode.E_RANGE]: "Range error",
                        [ErrorCode.E_ARGS]: "Incorrect number of arguments",
                        [ErrorCode.E_NACC]: "Move refused by destination",
                        [ErrorCode.E_INVARG]: "Invalid argument",
                        [ErrorCode.E_QUOTA]: "Resource limit exceeded",
                        [ErrorCode.E_FLOAT]: "Floating-point arithmetic error",
                        [ErrorCode.E_FILE]: "File error",
                        [ErrorCode.E_EXEC]: "Execution error",
                        [ErrorCode.E_INTRPT]: "Interruption",
                    };

                    const errorName = ErrorCode[errType] || `Error ${errType}`;
                    const friendlyMsg = errorMessages[errType] || "Unknown error";
                    const fullMsg = customMsg || friendlyMsg;
                    throw new Error(`${context} failed: ${errorName} (${fullMsg})`);
                }
            }
        }
    }

    const backtraceMatch = rawMessage.match(/backtrace:\s*\[String\("([^"]+)"\)/);
    if (backtraceMatch && backtraceMatch[1]) {
        const backtraceLine = backtraceMatch[1];
        const errorMatch = backtraceLine.match(/:\s*([EW]_\w+\s*\([^)]+\))/);
        if (errorMatch && errorMatch[1]) {
            throw new Error(`${context} failed: ${errorMatch[1]}`);
        }
        throw new Error(`${context} failed: ${backtraceLine}`);
    }

    const errorTypeMatch = rawMessage.match(/error:\s*([EW]_\w+)/);
    if (errorTypeMatch && errorTypeMatch[1]) {
        throw new Error(`${context} failed: ${errorTypeMatch[1]}`);
    }

    throw new Error(`${context} failed: ${rawMessage}`);
}
