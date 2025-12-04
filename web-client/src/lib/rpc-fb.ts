// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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
//

// FlatBuffer-based RPC client
// Proof of concept for direct FlatBuffer communication

import * as flatbuffers from "flatbuffers";
import { CompileErrorUnion, unionToCompileErrorUnion } from "../generated/moor-common/compile-error-union.js";
import { CompileError } from "../generated/moor-common/compile-error.js";
import { ErrorCode } from "../generated/moor-common/error-code.js";
import { EventUnion, unionToEventUnion } from "../generated/moor-common/event-union.js";
import { NarrativeEvent } from "../generated/moor-common/narrative-event.js";
import { NotifyEvent } from "../generated/moor-common/notify-event.js";
import { ParseError } from "../generated/moor-common/parse-error.js";
import { PresentEvent } from "../generated/moor-common/present-event.js";
import { TracebackEvent } from "../generated/moor-common/traceback-event.js";
import { UnpresentEvent } from "../generated/moor-common/unpresent-event.js";
import { AbortLimitReason } from "../generated/moor-rpc/abort-limit-reason.js";
import { ClientEventUnion, unionToClientEventUnion } from "../generated/moor-rpc/client-event-union.js";
import { ClientEvent } from "../generated/moor-rpc/client-event.js";
import { ClientSuccess } from "../generated/moor-rpc/client-success.js";
import { CommandErrorUnion } from "../generated/moor-rpc/command-error-union.js";
import { CommandExecutionError } from "../generated/moor-rpc/command-execution-error.js";
import { CompilationError } from "../generated/moor-rpc/compilation-error.js";
import { CurrentPresentations } from "../generated/moor-rpc/current-presentations.js";
import { unionToDaemonToClientReplyUnion } from "../generated/moor-rpc/daemon-to-client-reply-union.js";
import {
    DaemonToHostReplyUnion,
    unionToDaemonToHostReplyUnion,
} from "../generated/moor-rpc/daemon-to-host-reply-union.js";
import { EvalResult } from "../generated/moor-rpc/eval-result.js";
import { Failure } from "../generated/moor-rpc/failure.js";
import { HistoryResponseReply } from "../generated/moor-rpc/history-response-reply.js";
import { HostSuccess } from "../generated/moor-rpc/host-success.js";
import { ListObjectsReply } from "../generated/moor-rpc/list-objects-reply.js";
import { NarrativeEventMessage } from "../generated/moor-rpc/narrative-event-message.js";
import { PropertiesReply } from "../generated/moor-rpc/properties-reply.js";
import { PropertyUpdated } from "../generated/moor-rpc/property-updated.js";
import { PropertyValue } from "../generated/moor-rpc/property-value.js";
import { ReplyResultUnion, unionToReplyResultUnion } from "../generated/moor-rpc/reply-result-union.js";
import { ReplyResult } from "../generated/moor-rpc/reply-result.js";
import { RequestInputEvent } from "../generated/moor-rpc/request-input-event.js";
import { SchedulerErrorUnion, unionToSchedulerErrorUnion } from "../generated/moor-rpc/scheduler-error-union.js";
import { SchedulerError } from "../generated/moor-rpc/scheduler-error.js";
import { ServerFeatures } from "../generated/moor-rpc/server-features.js";
import { SysPropValue } from "../generated/moor-rpc/sys-prop-value.js";
import { SystemMessageEvent } from "../generated/moor-rpc/system-message-event.js";
import { SystemVerbResponseReply } from "../generated/moor-rpc/system-verb-response-reply.js";
import { unionToSystemVerbResponseUnion } from "../generated/moor-rpc/system-verb-response-union.js";
import { SystemVerbSuccess } from "../generated/moor-rpc/system-verb-success.js";
import { TaskAbortedException } from "../generated/moor-rpc/task-aborted-exception.js";
import { TaskAbortedLimit } from "../generated/moor-rpc/task-aborted-limit.js";
import { TaskErrorEvent } from "../generated/moor-rpc/task-error-event.js";
import { TaskSuccessEvent } from "../generated/moor-rpc/task-success-event.js";
import { VerbCompilationError } from "../generated/moor-rpc/verb-compilation-error.js";
import { VerbProgramErrorUnion } from "../generated/moor-rpc/verb-program-error-union.js";
import {
    unionToVerbProgramErrorUnion,
    VerbProgramErrorUnion as VerbProgramErrorUnionType,
} from "../generated/moor-rpc/verb-program-error-union.js";
import { VerbProgramFailed } from "../generated/moor-rpc/verb-program-failed.js";
import { VerbProgramFailure } from "../generated/moor-rpc/verb-program-failure.js";
import {
    unionToVerbProgramResponseUnion,
    VerbProgramResponseUnion,
} from "../generated/moor-rpc/verb-program-response-union.js";
import { VerbValue } from "../generated/moor-rpc/verb-value.js";
import { VerbsReply } from "../generated/moor-rpc/verbs-reply.js";
import { VarList } from "../generated/moor-var/var-list.js";
import { VarUnion } from "../generated/moor-var/var-union.js";
import { Var } from "../generated/moor-var/var.js";
import { decryptEventBlob } from "./age-decrypt.js";
import { buildAuthHeaders } from "./authHeaders";
import { parseInputMetadata } from "./input-metadata.js";
import { MoorVar } from "./MoorVar.js";

export interface ServerFeatureSet {
    persistentTasks: boolean;
    richNotify: boolean;
    lexicalScopes: boolean;
    typeDispatch: boolean;
    flyweightType: boolean;
    listComprehensions: boolean;
    boolType: boolean;
    useBooleanReturns: boolean;
    symbolType: boolean;
    useSymbolsInBuiltins: boolean;
    customErrors: boolean;
    useUuobjids: boolean;
    enableEventlog: boolean;
    anonymousObjects: boolean;
}

/**
 * Helper function to extract error details from a Failure response
 */
function extractFailureError(replyResult: ReplyResult, context: string): never {
    const resultType = replyResult.resultType();
    const failure = unionToReplyResultUnion(
        resultType,
        (obj) => replyResult.result(obj),
    ) as Failure | null;

    if (failure) {
        const rpcError = failure.error();
        if (rpcError) {
            const rawMessage = rpcError.message() || "Unknown error";
            const schedulerError = rpcError.schedulerError();

            console.log("Error details:", {
                hasSchedulerError: !!schedulerError,
                rawMessage: rawMessage,
                errorCode: rpcError.errorCode(),
            });

            // If there's a SchedulerError, try to extract detailed error info
            if (schedulerError) {
                const errorType = schedulerError.errorType();
                const errorUnion = unionToSchedulerErrorUnion(
                    errorType,
                    (obj: any) => schedulerError.error(obj),
                );

                console.log("SchedulerError type:", SchedulerErrorUnion[errorType]);

                // Handle CompilationError with detailed parse error information
                if (errorType === SchedulerErrorUnion.CompilationError && errorUnion) {
                    const compilationError = errorUnion as CompilationError;
                    const compileError = compilationError.error();
                    if (compileError) {
                        const errorType = compileError.errorType();
                        const errorDetail = unionToCompileErrorUnion(
                            errorType,
                            (obj: any) => compileError.error(obj),
                        );

                        // Special handling for ParseError to show location and message
                        if (errorType === CompileErrorUnion.ParseError && errorDetail) {
                            const parseError = errorDetail as ParseError;
                            const message = parseError.message();
                            const context = parseError.context();
                            const position = parseError.errorPosition();

                            if (message && position) {
                                const line = position.line();
                                const col = position.col();
                                throw new Error(
                                    `${context} failed: Parse error at line ${line}, col ${col}: ${message}`,
                                );
                            } else if (message) {
                                throw new Error(`${context} failed: ${message}`);
                            }
                        }

                        // For other compile errors, try to extract a meaningful message
                        throw new Error(`${context} failed: Compilation error (${CompileErrorUnion[errorType]})`);
                    }
                }

                // Handle TaskAbortedException which contains the MOO error
                if (errorType === SchedulerErrorUnion.TaskAbortedException && errorUnion) {
                    const taskException = errorUnion as TaskAbortedException;
                    const exception = taskException.exception();

                    if (exception) {
                        const error = exception.error();
                        if (error) {
                            const errType = error.errType();
                            const customMsg = error.msg();

                            // Map error codes to friendly messages
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

            // Fallback: try to parse the raw message string
            // Look for backtrace with clean error message
            const backtraceMatch = rawMessage.match(/backtrace:\s*\[String\("([^"]+)"\)/);
            if (backtraceMatch && backtraceMatch[1]) {
                const backtraceLine = backtraceMatch[1];
                // Extract just the error part: "E_PERM (Permission denied)"
                const errorMatch = backtraceLine.match(/:\s*([EW]_\w+\s*\([^)]+\))/);
                if (errorMatch && errorMatch[1]) {
                    throw new Error(`${context} failed: ${errorMatch[1]}`);
                }
                // Fallback to full backtrace line
                throw new Error(`${context} failed: ${backtraceLine}`);
            }

            // Try to extract just error code
            const errorTypeMatch = rawMessage.match(/error:\s*([EW]_\w+)/);
            if (errorTypeMatch && errorTypeMatch[1]) {
                const errorType = errorTypeMatch[1];
                throw new Error(`${context} failed: ${errorType}`);
            }

            // Last resort: show the raw message
            throw new Error(`${context} failed: ${rawMessage}`);
        }
    }
    throw new Error(`${context} failed with unknown error`);
}

/**
 * Evaluates a MOO expression on the server using FlatBuffer protocol
 *
 * @param authToken - Authentication token for the request
 * @param expr - MOO expression to evaluate
 * @returns Promise resolving to the evaluated result
 * @throws Error if the evaluation fails
 */
export async function performEvalFlatBuffer(authToken: string, expr: string): Promise<any> {
    try {
        const headers = buildAuthHeaders(authToken);
        const response = await fetch("/fb/eval", {
            method: "POST",
            body: expr,
            headers,
        });

        if (!response.ok) {
            throw new Error(`Eval failed: ${response.status} ${response.statusText}`);
        }

        // Get the response as an ArrayBuffer
        const arrayBuffer = await response.arrayBuffer();
        const bytes = new Uint8Array(arrayBuffer);

        // Parse the FlatBuffer response
        const replyResult = ReplyResult.getRootAsReplyResult(
            new flatbuffers.ByteBuffer(bytes),
        );

        // Navigate the union structure
        const resultType = replyResult.resultType();

        // Handle Failure case
        if (resultType === ReplyResultUnion.Failure) {
            extractFailureError(replyResult, "Eval");
        }

        if (resultType !== ReplyResultUnion.ClientSuccess) {
            throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
        }

        const clientSuccess = unionToReplyResultUnion(
            resultType,
            (obj) => replyResult.result(obj),
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

        // Check if it's an EvalResult
        if (!(replyUnion instanceof EvalResult)) {
            throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
        }

        const evalResult = replyUnion as EvalResult;
        const varResult = evalResult.result();

        if (!varResult) {
            throw new Error("Missing result var");
        }

        // Convert the Var to a JavaScript value using our wrapper
        return new MoorVar(varResult).toJS();
    } catch (err) {
        console.error("Exception during FlatBuffer eval:", err);
        throw err;
    }
}

/**
 * Evaluates a MOO expression and returns the result as a MoorVar for formatted display.
 * This is useful when you want MOO literal representation instead of JavaScript values.
 *
 * @param authToken - Authentication token for the request
 * @param expr - MOO expression to evaluate
 * @returns Promise resolving to the evaluated result as MoorVar
 * @throws Error if the evaluation fails
 */
export async function performEvalMoorVar(authToken: string, expr: string): Promise<MoorVar> {
    try {
        const headers = buildAuthHeaders(authToken);
        const response = await fetch("/fb/eval", {
            method: "POST",
            body: expr,
            headers,
        });

        if (!response.ok) {
            throw new Error(`Eval failed: ${response.status} ${response.statusText}`);
        }

        // Get the response as an ArrayBuffer
        const arrayBuffer = await response.arrayBuffer();
        const bytes = new Uint8Array(arrayBuffer);

        // Parse the FlatBuffer response
        const replyResult = ReplyResult.getRootAsReplyResult(
            new flatbuffers.ByteBuffer(bytes),
        );

        // Navigate the union structure
        const resultType = replyResult.resultType();

        // Handle Failure case
        if (resultType === ReplyResultUnion.Failure) {
            extractFailureError(replyResult, "Eval");
        }

        if (resultType !== ReplyResultUnion.ClientSuccess) {
            throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
        }

        const clientSuccess = unionToReplyResultUnion(
            resultType,
            (obj) => replyResult.result(obj),
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

        // Check if it's an EvalResult
        if (!(replyUnion instanceof EvalResult)) {
            throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
        }

        const evalResult = replyUnion as EvalResult;
        const varResult = evalResult.result();

        if (!varResult) {
            throw new Error("Missing result var");
        }

        // Return the MoorVar wrapper so the caller can decide how to format it
        return new MoorVar(varResult);
    } catch (err) {
        console.error("Exception during FlatBuffer eval:", err);
        throw err;
    }
}

/**
 * Retrieves server feature flags from the daemon.
 */
export async function fetchServerFeatures(): Promise<ServerFeatureSet> {
    const response = await fetch("/fb/features");
    if (!response.ok) {
        throw new Error(`Feature query failed: ${response.status} ${response.statusText}`);
    }

    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);
    const replyResult = ReplyResult.getRootAsReplyResult(new flatbuffers.ByteBuffer(bytes));

    const resultType = replyResult.resultType();

    // Handle Failure case
    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, "Server features request");
    }

    if (resultType !== ReplyResultUnion.HostSuccess) {
        throw new Error("Unexpected feature reply type");
    }

    const hostSuccess = unionToReplyResultUnion(
        resultType,
        (obj) => replyResult.result(obj),
    ) as HostSuccess | null;
    if (!hostSuccess) {
        throw new Error("Missing host success payload");
    }

    const daemonReply = hostSuccess.reply();
    if (!daemonReply) {
        throw new Error("Missing host reply for features");
    }

    const replyType = daemonReply.replyType();
    if (replyType !== DaemonToHostReplyUnion.ServerFeatures) {
        throw new Error("Unexpected server feature reply union");
    }

    const features = unionToDaemonToHostReplyUnion(
        replyType,
        (obj) => daemonReply.reply(obj),
    ) as ServerFeatures | null;

    if (!features) {
        throw new Error("Missing server feature payload");
    }

    return {
        persistentTasks: features.persistentTasks(),
        richNotify: features.richNotify(),
        lexicalScopes: features.lexicalScopes(),
        typeDispatch: features.typeDispatch(),
        flyweightType: features.flyweightType(),
        listComprehensions: features.listComprehensions(),
        boolType: features.boolType(),
        useBooleanReturns: features.useBooleanReturns(),
        symbolType: features.symbolType(),
        useSymbolsInBuiltins: features.useSymbolsInBuiltins(),
        customErrors: features.customErrors(),
        useUuobjids: features.useUuobjids(),
        enableEventlog: features.enableEventlog(),
        anonymousObjects: features.anonymousObjects(),
    };
}

/**
 * Retrieves a system property value from the server using FlatBuffer protocol
 *
 * @param objectPath - Array of path components (e.g., ['login', 'welcome_message'])
 * @param propertyName - Name of the property to retrieve
 * @returns Promise resolving to the property value
 * @throws Error if the retrieval fails
 */
export async function getSystemPropertyFlatBuffer(
    objectPath: string[],
    propertyName: string,
): Promise<any> {
    try {
        // Build the path
        const path = [...objectPath, propertyName].join("/");
        const response = await fetch(`/fb/system_property/${path}`, {
            method: "GET",
        });

        if (response.status === 404) {
            return null;
        }

        if (!response.ok) {
            throw new Error(`System property fetch failed: ${response.status} ${response.statusText}`);
        }

        // Get the response as an ArrayBuffer
        const arrayBuffer = await response.arrayBuffer();
        const bytes = new Uint8Array(arrayBuffer);

        // Parse the FlatBuffer response
        const replyResult = ReplyResult.getRootAsReplyResult(
            new flatbuffers.ByteBuffer(bytes),
        );

        // Navigate the union structure
        const resultType = replyResult.resultType();

        if (resultType === ReplyResultUnion.NONE) {
            throw new Error("Empty or invalid FlatBuffer response");
        }

        if (resultType === ReplyResultUnion.Failure) {
            extractFailureError(replyResult, "System property fetch");
        }

        if (resultType !== ReplyResultUnion.ClientSuccess) {
            throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
        }

        const clientSuccess = unionToReplyResultUnion(
            resultType,
            (obj) => replyResult.result(obj),
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

        // Check if it's a SysPropValue
        if (!(replyUnion instanceof SysPropValue)) {
            throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
        }

        const sysPropValue = replyUnion as SysPropValue;
        const varResult = sysPropValue.value();

        if (!varResult) {
            return null; // Property exists but has no value
        }

        // Convert the Var to a JavaScript value using our wrapper
        return new MoorVar(varResult).toJS();
    } catch (err) {
        console.error("Exception during FlatBuffer system property fetch:", err);
        throw err;
    }
}

/**
 * Fetch and decrypt player event history using FlatBuffer protocol
 *
 * @param authToken - Authentication token
 * @param ageIdentity - Age identity string (AGE-SECRET-KEY-1...) for decryption, or null
 * @param limit - Maximum number of events to fetch
 * @param sinceSeconds - Fetch events from N seconds ago
 * @param untilEvent - Fetch events until this event UUID
 * @returns Promise resolving to array of decrypted historical events
 */
export async function fetchHistoryFlatBuffer(
    authToken: string,
    ageIdentity: string | null,
    limit?: number,
    sinceSeconds?: number,
    untilEvent?: string,
): Promise<any[]> {
    try {
        // Build query parameters
        const params = new URLSearchParams();
        if (limit !== undefined) {
            params.set("limit", limit.toString());
        }
        if (sinceSeconds !== undefined) {
            params.set("since_seconds", sinceSeconds.toString());
        }
        if (untilEvent !== undefined) {
            params.set("until_event", untilEvent);
        }

        const url = `/fb/api/history?${params}`;

        const headers = buildAuthHeaders(authToken);
        const response = await fetch(url, {
            method: "GET",
            headers,
        });

        if (!response.ok) {
            throw new Error(`History fetch failed: ${response.status} ${response.statusText}`);
        }

        // Get the response as an ArrayBuffer
        const arrayBuffer = await response.arrayBuffer();
        const bytes = new Uint8Array(arrayBuffer);

        // Parse the FlatBuffer response (ReplyResult containing HistoryResponseReply)
        const replyResult = ReplyResult.getRootAsReplyResult(
            new flatbuffers.ByteBuffer(bytes),
        );

        const resultType = replyResult.resultType();

        // Handle Failure case
        if (resultType === ReplyResultUnion.Failure) {
            extractFailureError(replyResult, "History fetch");
        }

        if (resultType !== ReplyResultUnion.ClientSuccess) {
            throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
        }

        const clientSuccess = unionToReplyResultUnion(
            resultType,
            (obj) => replyResult.result(obj),
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

        if (!(replyUnion instanceof HistoryResponseReply)) {
            throw new Error(`Unexpected reply type: ${replyUnion?.constructor.name}`);
        }

        const historyResponse = replyUnion.response();
        if (!historyResponse) {
            throw new Error("Missing history response");
        }

        // Process encrypted events
        const events: any[] = [];
        const eventsLength = historyResponse.eventsLength();

        for (let i = 0; i < eventsLength; i++) {
            const historicalEvent = historyResponse.events(i);
            if (!historicalEvent) continue;

            const encryptedBlob = historicalEvent.encryptedBlobArray();
            if (!encryptedBlob) continue;

            // Decrypt the blob if we have an identity
            if (!ageIdentity) {
                console.warn("No age identity provided, skipping encrypted event");
                continue;
            }

            try {
                // Decrypt the blob using the age identity
                const decryptedBytes = await decryptEventBlob(encryptedBlob, ageIdentity);

                // Parse the decrypted NarrativeEvent FlatBuffer
                const narrativeEvent = NarrativeEvent.getRootAsNarrativeEvent(
                    new flatbuffers.ByteBuffer(decryptedBytes),
                );

                // Extract event data
                const eventId = narrativeEvent.eventId()?.dataArray();
                const eventIdStr = eventId
                    ? Array.from(eventId).map((b: number) => b.toString(16).padStart(2, "0")).join("")
                    : "";

                const timestamp = Number(narrativeEvent.timestamp());
                const isHistorical = historicalEvent.isHistorical();

                const eventData = narrativeEvent.event();
                if (!eventData) continue;

                // Convert event to format expected by useHistory
                events.push({
                    event_id: eventIdStr,
                    timestamp: timestamp / 1000000, // Convert from nanoseconds to milliseconds
                    is_historical: isHistorical,
                    event: eventData,
                    narrative_event: narrativeEvent,
                });
            } catch (err) {
                console.error("Failed to decrypt/parse event:", err);
                continue;
            }
        }

        return events;
    } catch (err) {
        console.error("Exception during FlatBuffer history fetch:", err);
        throw err;
    }
}

/**
 * Handle task errors by converting SchedulerError to user-friendly messages
 * Sends output to narrative like exception tracebacks do
 */
function handleTaskError(
    schedulerError: SchedulerError,
    onNarrativeMessage?: (
        content: string | string[],
        timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
        noNewline?: boolean,
        presentationHint?: string,
        groupId?: string,
        ttsText?: string,
        thumbnail?: { contentType: string; data: string },
    ) => void,
): void {
    const errorType = schedulerError.errorType();
    let message: string | null = null;
    let description: string[] | null = null;

    switch (errorType) {
        case SchedulerErrorUnion.CommandExecutionError: {
            const cmdExecError = schedulerError.error(new CommandExecutionError()) as CommandExecutionError | null;
            if (!cmdExecError) break;

            const cmdError = cmdExecError.error();
            if (!cmdError) break;

            const cmdErrorType = cmdError.errorType();
            switch (cmdErrorType) {
                case CommandErrorUnion.CouldNotParseCommand:
                    message = "I don't understand that.";
                    break;
                case CommandErrorUnion.NoObjectMatch:
                    message = "I don't see that here.";
                    break;
                case CommandErrorUnion.NoCommandMatch:
                    message = "I don't know how to do that.";
                    break;
                case CommandErrorUnion.PermissionDenied:
                    message = "You can't do that.";
                    break;
            }
            break;
        }

        case SchedulerErrorUnion.VerbProgramFailed: {
            const verbProgramFailed = schedulerError.error(new VerbProgramFailed()) as VerbProgramFailed | null;
            if (!verbProgramFailed) break;

            const verbError = verbProgramFailed.error();
            if (!verbError) break;

            const verbErrorType = verbError.errorType();
            switch (verbErrorType) {
                case VerbProgramErrorUnion.VerbCompilationError: {
                    const compError = verbError.error(new VerbCompilationError()) as VerbCompilationError | null;
                    if (compError) {
                        const ce = compError.error();
                        message = "Verb not programmed.";
                        if (ce) {
                            // Extract compilation error details
                            description = [ce.toString()];
                        }
                    }
                    break;
                }
                case VerbProgramErrorUnion.NoVerbToProgram:
                    message = "Verb not programmed.";
                    break;
                case VerbProgramErrorUnion.VerbPermissionDenied:
                    message = "Permission denied.";
                    break;
            }
            break;
        }

        case SchedulerErrorUnion.TaskAbortedLimit: {
            const abortLimit = schedulerError.error(new TaskAbortedLimit()) as TaskAbortedLimit | null;
            if (!abortLimit) break;

            const limit = abortLimit.limit();
            if (!limit) break;

            const limitReason = limit.reason();
            switch (limitReason) {
                case AbortLimitReason.Ticks:
                    message = "Task ran out of ticks";
                    break;
                case AbortLimitReason.Time:
                    message = "Task ran out of seconds";
                    break;
            }
            break;
        }

        case SchedulerErrorUnion.TaskAbortedError:
            message = "Task aborted";
            break;

        case SchedulerErrorUnion.TaskAbortedException:
            // No need to emit anything here, the standard exception handler should show.
            return;

        case SchedulerErrorUnion.TaskAbortedCancelled:
            message = "Task cancelled";
            break;

        default:
            console.warn(`[WS] Unhandled task error type: ${SchedulerErrorUnion[errorType]}`, schedulerError);
            return;
    }

    if (message && onNarrativeMessage) {
        const fullMessage = description ? `${message}\n${description.join("\n")}` : message;
        // Send to narrative output styled like exception tracebacks (red)
        onNarrativeMessage(
            fullMessage,
            new Date().toISOString(),
            "text/traceback",
            false,
            false,
            undefined,
            undefined,
            undefined,
            undefined,
        );
    }
}

/**
 * Handle a ClientEvent FlatBuffer from websocket
 *
 * Parses the binary ClientEvent and dispatches to appropriate handlers
 */
export interface EventMetadata {
    verb?: string;
    actor?: any;
    actorName?: string;
    content?: string;
    thisObj?: any;
    thisName?: string;
    dobj?: any;
    iobj?: any;
    timestamp?: number;
}

export function handleClientEventFlatBuffer(
    bytes: Uint8Array,
    onSystemMessage?: (message: string, duration?: number) => void,
    onNarrativeMessage?: (
        content: string | string[],
        timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
        noNewline?: boolean,
        presentationHint?: string,
        groupId?: string,
        ttsText?: string,
        thumbnail?: { contentType: string; data: string },
        eventMetadata?: EventMetadata,
    ) => void,
    onPresentMessage?: (presentData: any) => void,
    onUnpresentMessage?: (id: string) => void,
    onPlayerFlagsChange?: (flags: number) => void,
    lastEventTimestampRef?: React.MutableRefObject<bigint | null>,
    onInputMetadata?: (metadata: import("../types/input").InputMetadata | null) => void,
): void {
    try {
        // Parse the ClientEvent
        const clientEvent = ClientEvent.getRootAsClientEvent(
            new flatbuffers.ByteBuffer(bytes),
        );

        const eventType = clientEvent.eventType();

        // Handle based on event type
        switch (eventType) {
            case ClientEventUnion.NarrativeEventMessage: {
                const narrative = clientEvent.event(new NarrativeEventMessage()) as NarrativeEventMessage | null;
                if (!narrative) {
                    console.error("[WS] Failed to parse NarrativeEventMessage");
                    return;
                }

                const event = narrative.event();
                if (!event) {
                    console.error("[WS] Missing narrative event");
                    return;
                }

                const eventData = event.event();
                if (!eventData) {
                    console.error("[WS] Missing event data");
                    return;
                }

                // Extract timestamp (convert from nanoseconds to milliseconds)
                const timestampNanos = event.timestamp();
                const timestamp = new Date(Number(timestampNanos) / 1000000).toISOString();

                // Check for out-of-order messages using timestamp
                if (lastEventTimestampRef) {
                    if (lastEventTimestampRef.current !== null && timestampNanos < lastEventTimestampRef.current) {
                        console.warn(
                            `[WS] OUT OF ORDER MESSAGE DETECTED! Current: ${timestampNanos}, Previous: ${lastEventTimestampRef.current}, Diff: ${
                                lastEventTimestampRef.current - timestampNanos
                            }ns`,
                        );
                    }
                    lastEventTimestampRef.current = timestampNanos;
                }

                // Handle different event types within NarrativeEvent
                const innerEventType = eventData.eventType();

                switch (innerEventType) {
                    case EventUnion.NotifyEvent: {
                        const notify = eventData.event(new NotifyEvent()) as NotifyEvent | null;
                        if (!notify) {
                            console.error("[WS] Failed to parse NotifyEvent");
                            return;
                        }

                        const value = notify.value();
                        if (!value) {
                            console.error("[WS] Missing notify value");
                            return;
                        }

                        // Convert the Var to JavaScript value
                        const content = new MoorVar(value).toJS();

                        // Get content type
                        const contentTypeSym = notify.contentType();
                        let contentType = contentTypeSym ? contentTypeSym.value() : "text/plain";

                        // Normalize alternative content type formats
                        if (contentType === "text_djot" || contentType === "text/djot") {
                            contentType = "text/djot";
                        } else if (contentType === "text_html" || contentType === "text/html") {
                            contentType = "text/html";
                        } else {
                            contentType = "text/plain";
                        }

                        const noNewline = notify.noNewline();

                        // Extract metadata fields directly from top-level entries
                        let presentationHint: string | undefined;
                        let groupId: string | undefined;
                        let ttsText: string | undefined;
                        let thumbnail: { contentType: string; data: string } | undefined;
                        const eventMeta: EventMetadata = {};
                        const metadataLength = notify.metadataLength();
                        for (let i = 0; i < metadataLength; i++) {
                            const metadata = notify.metadata(i);
                            if (metadata) {
                                const key = metadata.key();
                                const keyValue = key ? key.value() : null;
                                const metaValue = metadata.value();
                                const value = metaValue ? new MoorVar(metaValue).toJS() : null;

                                if (keyValue === "presentation_hint" && typeof value === "string") {
                                    presentationHint = value;
                                } else if (keyValue === "group_id" && typeof value === "string") {
                                    groupId = value;
                                } else if (keyValue === "tts_text" && typeof value === "string") {
                                    ttsText = value;
                                } else if (keyValue === "thumbnail" && Array.isArray(value) && value.length === 2) {
                                    // thumbnail is [content_type, binary_data]
                                    const [thumbContentType, binaryData] = value;
                                    if (
                                        typeof thumbContentType === "string"
                                        && binaryData instanceof Uint8Array
                                    ) {
                                        // Convert binary data to base64 data URL
                                        const base64 = btoa(String.fromCharCode(...binaryData));
                                        thumbnail = {
                                            contentType: thumbContentType,
                                            data: `data:${thumbContentType};base64,${base64}`,
                                        };
                                    }
                                } else if (keyValue === "verb" && typeof value === "string") {
                                    eventMeta.verb = value;
                                } else if (keyValue === "actor") {
                                    eventMeta.actor = value;
                                } else if (keyValue === "actor_name" && typeof value === "string") {
                                    eventMeta.actorName = value;
                                } else if (keyValue === "content" && typeof value === "string") {
                                    eventMeta.content = value;
                                } else if (keyValue === "this_obj") {
                                    eventMeta.thisObj = value;
                                } else if (keyValue === "this_name" && typeof value === "string") {
                                    eventMeta.thisName = value;
                                } else if (keyValue === "dobj") {
                                    eventMeta.dobj = value;
                                } else if (keyValue === "iobj") {
                                    eventMeta.iobj = value;
                                } else if (keyValue === "timestamp" && typeof value === "number") {
                                    eventMeta.timestamp = value;
                                }
                            }
                        }

                        // Log the full parsed NotifyEvent with all metadata
                        const rawMetadata: Array<{ key: string | null; value: any }> = [];
                        for (let i = 0; i < metadataLength; i++) {
                            const md = notify.metadata(i);
                            if (md) {
                                const k = md.key();
                                const v = md.value();
                                rawMetadata.push({
                                    key: k ? k.value() : null,
                                    value: v ? new MoorVar(v).toJS() : null,
                                });
                            }
                        }
                        console.log("[Narrative Event]", {
                            eventType: "NotifyEvent",
                            content,
                            contentType,
                            noNewline,
                            timestamp,
                            rawMetadata,
                            parsedMetadata: {
                                presentationHint,
                                groupId,
                                ttsText,
                                thumbnail: thumbnail ? "(present)" : undefined,
                            },
                        });

                        if (onNarrativeMessage) {
                            onNarrativeMessage(
                                content,
                                timestamp,
                                contentType || undefined,
                                false,
                                noNewline,
                                presentationHint,
                                groupId,
                                ttsText,
                                thumbnail,
                                Object.keys(eventMeta).length > 0 ? eventMeta : undefined,
                            );
                        }
                        break;
                    }

                    case EventUnion.PresentEvent: {
                        const present = eventData.event(new PresentEvent()) as PresentEvent | null;
                        if (!present) {
                            console.error("[WS] Failed to parse PresentEvent");
                            return;
                        }

                        const presentation = present.presentation();
                        if (!presentation && onPresentMessage) {
                            console.error("[WS] Missing presentation");
                            return;
                        }

                        if (onPresentMessage && presentation) {
                            // Convert presentation to plain JS object
                            let contentType = presentation.contentType() || "text/plain";

                            // Normalize alternative content type formats
                            if (contentType === "text_djot" || contentType === "text/djot") {
                                contentType = "text/djot";
                            } else if (contentType === "text_html" || contentType === "text/html") {
                                contentType = "text/html";
                            } else {
                                contentType = "text/plain";
                            }

                            // Parse attributes
                            const attributes: Array<[string, string]> = [];
                            const attributesLength = presentation.attributesLength();
                            for (let i = 0; i < attributesLength; i++) {
                                const attr = presentation.attributes(i);
                                if (attr) {
                                    const key = attr.key();
                                    const value = attr.value();
                                    if (key && value) {
                                        attributes.push([key, value]);
                                    }
                                }
                            }

                            const presentData = {
                                id: presentation.id(),
                                content: presentation.content(),
                                content_type: contentType,
                                target: presentation.target(),
                                attributes,
                            };
                            console.log("[Narrative Event]", {
                                eventType: "PresentEvent",
                                ...presentData,
                            });
                            onPresentMessage(presentData);
                        }
                        break;
                    }

                    case EventUnion.UnpresentEvent: {
                        const unpresent = eventData.event(new UnpresentEvent()) as UnpresentEvent | null;
                        if (!unpresent) {
                            console.error("[WS] Failed to parse UnpresentEvent");
                            return;
                        }

                        const presentationId = unpresent.presentationId();
                        console.log("[Narrative Event]", {
                            eventType: "UnpresentEvent",
                            presentationId,
                        });
                        if (presentationId && onUnpresentMessage) {
                            onUnpresentMessage(presentationId);
                        }
                        break;
                    }

                    case EventUnion.TracebackEvent: {
                        const traceback = eventData.event(new TracebackEvent()) as TracebackEvent | null;
                        if (!traceback) {
                            console.error("[WS] Failed to parse TracebackEvent");
                            return;
                        }

                        const exception = traceback.exception();
                        if (!exception) {
                            console.error("[WS] Missing exception");
                            return;
                        }

                        // Build traceback text from backtrace frames
                        const tracebackLines: string[] = [];
                        for (let i = 0; i < exception.backtraceLength(); i++) {
                            const backtraceVar = exception.backtrace(i);
                            if (backtraceVar) {
                                // Extract string from the Var
                                const line = new MoorVar(backtraceVar).asString();
                                if (line) {
                                    tracebackLines.push(line);
                                }
                            }
                        }

                        const tracebackText = tracebackLines.join("\n");

                        console.log("[Narrative Event]", {
                            eventType: "TracebackEvent",
                            tracebackLines,
                        });

                        if (onNarrativeMessage) {
                            onNarrativeMessage(
                                tracebackText,
                                timestamp,
                                "text/traceback",
                                false,
                                false,
                                undefined,
                                undefined,
                                undefined,
                                undefined,
                            );
                        }
                        break;
                    }

                    default:
                        console.warn(`[WS] Unknown inner event type: ${innerEventType}`);
                }
                break;
            }

            case ClientEventUnion.SystemMessageEvent: {
                const sysMsg = clientEvent.event(new SystemMessageEvent()) as SystemMessageEvent | null;
                if (!sysMsg) {
                    console.error("[WS] Failed to parse SystemMessageEvent");
                    return;
                }

                const message = sysMsg.message();
                if (message && onSystemMessage) {
                    onSystemMessage(message, 5);
                }
                break;
            }

            case ClientEventUnion.RequestInputEvent: {
                const requestInput = clientEvent.event(new RequestInputEvent()) as RequestInputEvent | null;
                if (!requestInput) {
                    console.error("[WS] Failed to parse RequestInputEvent");
                    return;
                }

                // Extract metadata if present
                const metadataPairs = [];
                const metadataLength = requestInput.metadataLength();
                for (let i = 0; i < metadataLength; i++) {
                    const pair = requestInput.metadata(i);
                    if (pair) {
                        metadataPairs.push(pair);
                    }
                }

                // Parse metadata into InputMetadata structure
                const metadata = parseInputMetadata(metadataPairs.length > 0 ? metadataPairs : null);

                // Notify the input metadata handler
                if (onInputMetadata) {
                    onInputMetadata(metadata);
                }
                break;
            }

            case ClientEventUnion.TaskErrorEvent: {
                const taskError = clientEvent.event(new TaskErrorEvent()) as TaskErrorEvent | null;
                if (!taskError) {
                    console.error("[WS] Failed to parse TaskErrorEvent");
                    return;
                }

                const error = taskError.error();
                if (!error) {
                    console.error("[WS] Missing scheduler error");
                    return;
                }

                // Handle the error using our error handler - send to narrative like exceptions
                handleTaskError(error, onNarrativeMessage);
                break;
            }

            case ClientEventUnion.TaskSuccessEvent: {
                const taskSuccess = clientEvent.event(new TaskSuccessEvent()) as TaskSuccessEvent | null;
                if (!taskSuccess) {
                    console.error("[WS] Failed to parse TaskSuccessEvent");
                    return;
                }

                // Task completed successfully - these now come via HTTP response for verb invocations
                break;
            }

            case ClientEventUnion.PlayerSwitchedEvent:
            case ClientEventUnion.SetConnectionOptionEvent:
            case ClientEventUnion.DisconnectEvent:
                // These events don't need client-side handling
                break;

            default:
                console.warn(`[WS] Unknown event type: ${eventType}`);
        }
    } catch (err) {
        console.error("[WS] Failed to parse ClientEvent FlatBuffer:", err);
    }
}

/**
 * Get list of verbs on an object using FlatBuffer protocol
 *
 * @param authToken - Authentication token
 * @param objectCurie - Object CURIE (e.g., "oid:123")
 * @param inherited - Whether to include inherited verbs
 * @returns Promise resolving to VerbsReply FlatBuffer
 */
export async function getVerbsFlatBuffer(
    authToken: string,
    objectCurie: string,
    inherited: boolean = true,
): Promise<VerbsReply> {
    const params = new URLSearchParams();
    if (inherited) {
        params.set("inherited", "true");
    }

    const headers = buildAuthHeaders(authToken);
    const response = await fetch(`/fb/verbs/${objectCurie}?${params}`, {
        method: "GET",
        headers,
    });

    if (!response.ok) {
        throw new Error(`Get verbs failed: ${response.status} ${response.statusText}`);
    }

    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);

    const replyResult = ReplyResult.getRootAsReplyResult(
        new flatbuffers.ByteBuffer(bytes),
    );

    const resultType = replyResult.resultType();

    // Handle Failure case
    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, "Get verbs");
    }

    if (resultType !== ReplyResultUnion.ClientSuccess) {
        throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
    }

    const clientSuccess = unionToReplyResultUnion(
        resultType,
        (obj) => replyResult.result(obj),
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

    // Return the VerbsReply FlatBuffer wrapper
    if (!(replyUnion instanceof VerbsReply)) {
        throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
    }

    return replyUnion as VerbsReply;
}

/**
 * Get verb code using FlatBuffer protocol
 *
 * @param authToken - Authentication token
 * @param objectCurie - Object CURIE
 * @param verbName - Verb name
 * @returns Promise resolving to VerbValue FlatBuffer
 */
export async function getVerbCodeFlatBuffer(
    authToken: string,
    objectCurie: string,
    verbName: string,
): Promise<VerbValue> {
    const headers = buildAuthHeaders(authToken);
    const response = await fetch(`/fb/verbs/${objectCurie}/${encodeURIComponent(verbName)}`, {
        method: "GET",
        headers,
    });

    if (!response.ok) {
        throw new Error(`Get verb code failed: ${response.status} ${response.statusText}`);
    }

    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);

    const replyResult = ReplyResult.getRootAsReplyResult(
        new flatbuffers.ByteBuffer(bytes),
    );

    const resultType = replyResult.resultType();

    // Handle Failure case
    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, "Get verb code");
    }

    if (resultType !== ReplyResultUnion.ClientSuccess) {
        throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
    }

    const clientSuccess = unionToReplyResultUnion(
        resultType,
        (obj) => replyResult.result(obj),
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

    // Return the VerbValue FlatBuffer wrapper
    if (!(replyUnion instanceof VerbValue)) {
        throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
    }

    return replyUnion as VerbValue;
}

/**
 * Invoke a verb using FlatBuffer protocol
 *
 * @param authToken - Authentication token
 * @param objectCurie - Object CURIE
 * @param verbName - Verb name
 * @param args - Optional FlatBuffer-encoded Var (typically a VarList) as Uint8Array
 * @returns Promise resolving to EvalResult FlatBuffer (or TaskSubmitted)
 */
export async function invokeVerbFlatBuffer(
    authToken: string,
    objectCurie: string,
    verbName: string,
    args?: Uint8Array,
): Promise<EvalResult | any> {
    // If no args provided, build an empty list FlatBuffer
    let argsBytes: Uint8Array;
    if (!args) {
        const builder = new flatbuffers.Builder(256);
        const emptyListOffset = VarList.createVarList(builder, VarList.createElementsVector(builder, []));
        const listVarOffset = Var.createVar(builder, VarUnion.VarList, emptyListOffset);
        builder.finish(listVarOffset);
        argsBytes = builder.asUint8Array();
    } else {
        argsBytes = args;
    }

    // Create a properly sized ArrayBuffer from the Uint8Array
    // Can't use argsBytes.buffer directly as it might be larger than the actual data
    const bodyBuffer = argsBytes.slice(0, argsBytes.length).buffer as ArrayBuffer;

    const headers = buildAuthHeaders(authToken);
    headers["Content-Type"] = "application/x-flatbuffer";
    const response = await fetch(`/fb/verbs/${objectCurie}/${encodeURIComponent(verbName)}/invoke`, {
        method: "POST",
        headers,
        body: bodyBuffer,
    });

    if (!response.ok) {
        throw new Error(`Invoke verb failed: ${response.status} ${response.statusText}`);
    }

    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);

    // Server returns ClientEvent (TaskSuccessEvent or TaskErrorEvent)
    const clientEvent = ClientEvent.getRootAsClientEvent(
        new flatbuffers.ByteBuffer(bytes),
    );

    const eventType = clientEvent.eventType();
    const event = unionToClientEventUnion(
        eventType,
        (obj: any) => clientEvent.event(obj),
    );

    if (!event) {
        throw new Error("Failed to parse client event");
    }

    // Return the event (TaskSuccessEvent or TaskErrorEvent)
    return event;
}

/**
 * Get list of properties on an object using FlatBuffer protocol
 *
 * @param authToken - Authentication token
 * @param objectCurie - Object CURIE (e.g., "oid:123")
 * @param inherited - Whether to include inherited properties
 * @returns Promise resolving to PropertiesReply FlatBuffer
 */
export async function getPropertiesFlatBuffer(
    authToken: string,
    objectCurie: string,
    inherited: boolean = true,
): Promise<PropertiesReply> {
    const params = new URLSearchParams();
    if (inherited) {
        params.set("inherited", "true");
    }

    const headers = buildAuthHeaders(authToken);
    const response = await fetch(`/fb/properties/${objectCurie}?${params}`, {
        method: "GET",
        headers,
    });

    if (!response.ok) {
        throw new Error(`Get properties failed: ${response.status} ${response.statusText}`);
    }

    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);

    const replyResult = ReplyResult.getRootAsReplyResult(
        new flatbuffers.ByteBuffer(bytes),
    );

    const resultType = replyResult.resultType();

    // Handle Failure case
    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, "Get properties");
    }

    if (resultType !== ReplyResultUnion.ClientSuccess) {
        throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
    }

    const clientSuccess = unionToReplyResultUnion(
        resultType,
        (obj) => replyResult.result(obj),
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

    // Return the PropertiesReply FlatBuffer wrapper
    if (!(replyUnion instanceof PropertiesReply)) {
        throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
    }

    return replyUnion as PropertiesReply;
}

/**
 * Get property value using FlatBuffer protocol
 *
 * @param authToken - Authentication token
 * @param objectCurie - Object CURIE
 * @param propertyName - Property name
 * @returns Promise resolving to PropertyValue FlatBuffer
 */
export async function getPropertyFlatBuffer(
    authToken: string,
    objectCurie: string,
    propertyName: string,
): Promise<PropertyValue> {
    const headers = buildAuthHeaders(authToken);
    const response = await fetch(`/fb/properties/${objectCurie}/${encodeURIComponent(propertyName)}`, {
        method: "GET",
        headers,
    });

    if (!response.ok) {
        throw new Error(`Get property failed: ${response.status} ${response.statusText}`);
    }

    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);

    const replyResult = ReplyResult.getRootAsReplyResult(
        new flatbuffers.ByteBuffer(bytes),
    );

    const resultType = replyResult.resultType();

    // Handle Failure case
    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, "Get property");
    }

    if (resultType !== ReplyResultUnion.ClientSuccess) {
        throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
    }

    const clientSuccess = unionToReplyResultUnion(
        resultType,
        (obj) => replyResult.result(obj),
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

    // Return the PropertyValue FlatBuffer wrapper
    if (!(replyUnion instanceof PropertyValue)) {
        throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
    }

    return replyUnion as PropertyValue;
}

/**
 * Get current presentations using FlatBuffer protocol
 *
 * @param authToken - Authentication token
 * @returns Promise resolving to CurrentPresentations FlatBuffer
 */
export async function getCurrentPresentationsFlatBuffer(
    authToken: string,
): Promise<CurrentPresentations> {
    const headers = buildAuthHeaders(authToken);
    const response = await fetch(`/fb/api/presentations`, {
        method: "GET",
        headers,
    });

    if (!response.ok) {
        throw new Error(`Get presentations failed: ${response.status} ${response.statusText}`);
    }

    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);

    const replyResult = ReplyResult.getRootAsReplyResult(
        new flatbuffers.ByteBuffer(bytes),
    );

    const resultType = replyResult.resultType();

    // Handle Failure case
    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, "Get presentations");
    }

    if (resultType !== ReplyResultUnion.ClientSuccess) {
        throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
    }

    const clientSuccess = unionToReplyResultUnion(
        resultType,
        (obj) => replyResult.result(obj),
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

    // Return the CurrentPresentations FlatBuffer wrapper
    if (!(replyUnion instanceof CurrentPresentations)) {
        throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
    }

    return replyUnion as CurrentPresentations;
}

/**
 * Compiles/programs a verb using FlatBuffer protocol
 *
 * @param authToken - Authentication token
 * @param objectCurie - Object CURIE (e.g., "oid:123")
 * @param verbName - Name of the verb to compile
 * @param code - Source code to compile
 * @returns Promise resolving to empty object on success, or object with CompileError on failure
 */
export async function compileVerbFlatBuffer(
    authToken: string,
    objectCurie: string,
    verbName: string,
    code: string,
): Promise<{ success: true } | { success: false; error: CompileError | string }> {
    const headers = buildAuthHeaders(authToken);
    const response = await fetch(`/fb/verbs/${objectCurie}/${verbName}`, {
        method: "POST",
        headers,
        body: code,
    });

    if (!response.ok) {
        throw new Error(`Compile verb failed: ${response.status} ${response.statusText}`);
    }

    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);

    const replyResult = ReplyResult.getRootAsReplyResult(
        new flatbuffers.ByteBuffer(bytes),
    );

    const resultType = replyResult.resultType();

    // Handle Failure case
    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, "Compile verb");
    }

    if (resultType !== ReplyResultUnion.ClientSuccess) {
        throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
    }

    const clientSuccess = unionToReplyResultUnion(
        resultType,
        (obj) => replyResult.result(obj),
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

    // replyUnion is VerbProgramResponseReply, get the VerbProgramResponse from it
    const verbProgramResponseReply = replyUnion as any;
    const verbProgramResponse = verbProgramResponseReply.response();
    if (!verbProgramResponse) {
        throw new Error("Missing VerbProgramResponse");
    }

    const responseType = verbProgramResponse.responseType();

    if (responseType === VerbProgramResponseUnion.VerbProgramSuccess) {
        return { success: true };
    } else if (responseType === VerbProgramResponseUnion.VerbProgramFailure) {
        const failureResponse = unionToVerbProgramResponseUnion(
            responseType,
            (obj: any) => verbProgramResponse.response(obj),
        ) as VerbProgramFailure | null;

        if (failureResponse) {
            const programError = failureResponse.error();
            if (programError) {
                const errorType = programError.errorType();

                // Handle different error types
                switch (errorType) {
                    case VerbProgramErrorUnionType.VerbCompilationError: {
                        const compError = unionToVerbProgramErrorUnion(
                            errorType,
                            (obj: any) => programError.error(obj),
                        ) as VerbCompilationError | null;

                        if (compError && compError.error()) {
                            // Return the full structured CompileError FlatBuffer object
                            return { success: false, error: compError.error()! };
                        }
                        return { success: false, error: "Compilation error" };
                    }
                    case VerbProgramErrorUnionType.NoVerbToProgram:
                        return { success: false, error: "No verb to program" };
                    case VerbProgramErrorUnionType.VerbPermissionDenied:
                        return { success: false, error: "Permission denied" };
                    case VerbProgramErrorUnionType.VerbDatabaseError:
                        return { success: false, error: "Database error" };
                    default:
                        return { success: false, error: "Unknown compilation error" };
                }
            }
        }
        return { success: false, error: "Compilation failed" };
    } else {
        throw new Error(`Unexpected VerbProgramResponse type: ${responseType}`);
    }
}

/**
 * Convert a FlatBuffer NarrativeEvent to a JavaScript object
 */
function narrativeEventToJS(narrativeEvent: any): any {
    if (!narrativeEvent) return null;

    // Get the Event object from the NarrativeEvent
    const eventObj = narrativeEvent.event();
    if (!eventObj) return null;

    // Get the event type from the Event object
    const eventType = eventObj.eventType();

    // Use the correct union pattern to get the event union
    const eventUnion = unionToEventUnion(
        eventType,
        (obj: any) => eventObj.event(obj),
    );

    if (!eventUnion) return null;

    // Handle different event types
    switch (eventType) {
        case 1: { // NotifyEvent
            const notifyEvent = eventUnion as any;
            const value = notifyEvent.value();
            const contentTypeSym = notifyEvent.contentType();

            // Convert the Var to JavaScript value using the same pattern as WebSocket handler
            const content = value ? new MoorVar(value).toJS() : "";

            // Get content type
            let contentType = contentTypeSym ? contentTypeSym.value() : "text/plain";

            // Normalize alternative content type formats
            if (contentType === "text_djot" || contentType === "text/djot") {
                contentType = "text/djot";
            } else if (contentType === "text_html" || contentType === "text/html") {
                contentType = "text/html";
            } else {
                contentType = "text/plain";
            }

            return {
                eventType: "NotifyEvent",
                event: {
                    value: content,
                    contentType: contentType,
                },
            };
        }
        case 2: { // PresentEvent
            const presentEvent = eventUnion as any;
            return {
                eventType: "PresentEvent",
                event: {
                    object: presentEvent.object(),
                },
            };
        }
        case 3: { // UnpresentEvent
            const unpresentEvent = eventUnion as any;
            return {
                eventType: "UnpresentEvent",
                event: {
                    object: unpresentEvent.object(),
                },
            };
        }
        case 4: { // TracebackEvent
            const tracebackEvent = eventUnion as any;
            return {
                eventType: "TracebackEvent",
                event: {
                    traceback: tracebackEvent.traceback(),
                },
            };
        }
        default:
            return null;
    }
}

/**
 * Invoke the welcome message system verb using FlatBuffer protocol
 *
 * This calls #0:do_login_command and returns the narrative event output
 * @returns Promise resolving to object with welcome message and content type
 */
export async function invokeWelcomeMessageFlatBuffer(): Promise<{
    welcomeMessage: string;
    contentType: "text/plain" | "text/djot" | "text/html" | "text/traceback";
}> {
    try {
        const response = await fetch(`/fb/invoke_welcome_message`, {
            method: "GET",
        });

        if (!response.ok) {
            throw new Error(`Welcome message invocation failed: ${response.status} ${response.statusText}`);
        }

        // Get the response as an ArrayBuffer
        const arrayBuffer = await response.arrayBuffer();
        const bytes = new Uint8Array(arrayBuffer);

        // Parse the FlatBuffer response
        const replyResult = ReplyResult.getRootAsReplyResult(
            new flatbuffers.ByteBuffer(bytes),
        );

        // Navigate the union structure
        const resultType = replyResult.resultType();

        if (resultType === ReplyResultUnion.NONE) {
            throw new Error("Empty or invalid FlatBuffer response");
        }

        if (resultType === ReplyResultUnion.Failure) {
            extractFailureError(replyResult, "System property fetch");
        }

        if (resultType !== ReplyResultUnion.ClientSuccess) {
            throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
        }

        const clientSuccess = unionToReplyResultUnion(
            resultType,
            (obj) => replyResult.result(obj),
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

        // Check if it's a SystemVerbResponseReply
        if (!(replyUnion instanceof SystemVerbResponseReply)) {
            throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
        }

        const systemVerbReply = replyUnion as SystemVerbResponseReply;
        const responseType = systemVerbReply.responseType();
        const responseUnion = unionToSystemVerbResponseUnion(
            responseType,
            (obj: any) => systemVerbReply.response(obj),
        );

        if (!responseUnion) {
            throw new Error("Failed to parse system verb response union");
        }

        // Check if it's a SystemVerbSuccess
        if (!(responseUnion instanceof SystemVerbSuccess)) {
            throw new Error(`Unexpected system verb response type: ${responseUnion.constructor.name}`);
        }

        const systemVerbSuccess = responseUnion as SystemVerbSuccess;

        // Get the output (narrative events) directly from the SystemVerbSuccess
        const outputCount = systemVerbSuccess.outputLength();

        // Extract welcome message from narrative events
        let welcomeMessage = "";
        let contentType: "text/plain" | "text/djot" | "text/html" | "text/traceback" = "text/plain";

        for (let i = 0; i < outputCount; i++) {
            const narrativeEvent = systemVerbSuccess.output(i, new NarrativeEvent());
            if (narrativeEvent) {
                // Convert the narrative event to JavaScript object
                const eventObj = narrativeEventToJS(narrativeEvent);

                if (eventObj && eventObj.eventType === "NotifyEvent" && eventObj.event) {
                    const notifyEvent = eventObj.event;
                    if (notifyEvent.value) {
                        // Extract the message content
                        const content = notifyEvent.value;
                        if (typeof content === "string") {
                            welcomeMessage = content;
                        } else if (Array.isArray(content)) {
                            welcomeMessage = content.join("\n");
                        }

                        // Extract content type
                        if (notifyEvent.contentType) {
                            const ct = notifyEvent.contentType;
                            if (
                                ct === "text/html" || ct === "text/djot" || ct === "text/plain"
                                || ct === "text/traceback"
                            ) {
                                contentType = ct;
                            }
                        }
                        break; // Use the first notify event
                    }
                }
            }
        }

        return { welcomeMessage, contentType };
    } catch (err) {
        console.error("Exception during welcome message invocation:", err);
        throw err;
    }
}

/**
 * Get list of all objects using FlatBuffer protocol
 *
 * @param authToken - Authentication token
 * @returns Promise resolving to ListObjectsReply FlatBuffer
 */
export async function listObjectsFlatBuffer(
    authToken: string,
): Promise<ListObjectsReply> {
    const headers = buildAuthHeaders(authToken);
    const response = await fetch(`/fb/objects`, {
        method: "GET",
        headers,
    });

    if (!response.ok) {
        throw new Error(`List objects failed: ${response.status} ${response.statusText}`);
    }

    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);

    const replyResult = ReplyResult.getRootAsReplyResult(
        new flatbuffers.ByteBuffer(bytes),
    );

    const resultType = replyResult.resultType();

    // Handle Failure case
    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, "List objects");
    }

    if (resultType !== ReplyResultUnion.ClientSuccess) {
        throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
    }

    const clientSuccess = unionToReplyResultUnion(
        resultType,
        (obj) => replyResult.result(obj),
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

    // Return the ListObjectsReply FlatBuffer wrapper
    if (!(replyUnion instanceof ListObjectsReply)) {
        throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
    }

    return replyUnion as ListObjectsReply;
}

/**
 * Update a property value using FlatBuffer protocol
 *
 * @param authToken - Authentication token
 * @param objectCurie - Object CURIE
 * @param propertyName - Property name
 * @param value - New value (will be converted to FlatBuffer Var)
 * @returns Promise resolving when property is updated
 */
export async function updatePropertyFlatBuffer(
    authToken: string,
    objectCurie: string,
    propertyName: string,
    value: string, // MOO literal string
): Promise<void> {
    // Send MOO literal string directly (like eval endpoint)
    // Backend will parse it into a Var
    const headers = buildAuthHeaders(authToken);
    headers["Content-Type"] = "text/plain";
    const response = await fetch(`/fb/properties/${objectCurie}/${encodeURIComponent(propertyName)}`, {
        method: "POST",
        headers,
        body: value,
    });

    if (!response.ok) {
        throw new Error(`Update property failed: ${response.status} ${response.statusText}`);
    }

    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);

    const replyResult = ReplyResult.getRootAsReplyResult(
        new flatbuffers.ByteBuffer(bytes),
    );

    const resultType = replyResult.resultType();

    // Handle Failure case
    if (resultType === ReplyResultUnion.Failure) {
        extractFailureError(replyResult, "Update property");
    }

    if (resultType !== ReplyResultUnion.ClientSuccess) {
        throw new Error(`Unexpected result type: ${ReplyResultUnion[resultType]}`);
    }

    const clientSuccess = unionToReplyResultUnion(
        resultType,
        (obj) => replyResult.result(obj),
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

    // Verify it's a PropertyUpdated response
    if (!(replyUnion instanceof PropertyUpdated)) {
        throw new Error(`Unexpected reply type: ${replyUnion.constructor.name}`);
    }

    // Success - no return value needed
}
