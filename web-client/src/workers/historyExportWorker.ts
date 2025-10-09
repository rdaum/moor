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

// Web Worker for exporting event history
// Handles decryption and JSON conversion off the main thread

import * as flatbuffers from "flatbuffers";
import { EventUnion } from "../generated/moor-common/event-union.js";
import { NarrativeEvent } from "../generated/moor-common/narrative-event.js";
import { NotifyEvent } from "../generated/moor-common/notify-event.js";
import { PresentEvent } from "../generated/moor-common/present-event.js";
import { TracebackEvent } from "../generated/moor-common/traceback-event.js";
import { UnpresentEvent } from "../generated/moor-common/unpresent-event.js";
import { ClientSuccess } from "../generated/moor-rpc/client-success.js";
import { unionToDaemonToClientReplyUnion } from "../generated/moor-rpc/daemon-to-client-reply-union.js";
import { HistoryResponseReply } from "../generated/moor-rpc/history-response-reply.js";
import { ReplyResultUnion, unionToReplyResultUnion } from "../generated/moor-rpc/reply-result-union.js";
import { ReplyResult } from "../generated/moor-rpc/reply-result.js";
import { decryptEventBlob } from "../lib/age-decrypt.js";
import { MoorVar } from "../lib/MoorVar.js";

// Message types
export interface StartExportMessage {
    type: "start";
    authToken: string;
    ageIdentity: string;
    systemTitle: string;
    playerOid: string;
}

export interface ProgressMessage {
    type: "progress";
    processed: number;
    total?: number;
}

export interface ErrorMessage {
    type: "error";
    error: string;
}

export interface CompleteMessage {
    type: "complete";
    jsonBlob: Blob;
}

export type WorkerResponse = ProgressMessage | ErrorMessage | CompleteMessage;

// Convert a decrypted NarrativeEvent to a JSON-serializable object
function narrativeEventToJSON(narrativeEvent: NarrativeEvent): any {
    const eventData = narrativeEvent.event();
    if (!eventData) return null;

    const eventId = narrativeEvent.eventId()?.dataArray();
    const eventIdStr = eventId
        ? Array.from(eventId).map((b: number) => b.toString(16).padStart(2, "0")).join("")
        : "";

    const timestamp = Number(narrativeEvent.timestamp());
    const timestampMs = timestamp / 1000000; // Convert from nanoseconds to milliseconds
    const timestampISO = new Date(timestampMs).toISOString();

    const eventType = eventData.eventType();

    const result: any = {
        event_id: eventIdStr,
        timestamp: timestampISO,
        timestamp_ms: timestampMs,
    };

    // Extract author (player OID) if present
    const author = narrativeEvent.author();
    if (author) {
        const authorValue = new MoorVar(author).toJS();
        if (authorValue && typeof authorValue === "object" && "Obj" in authorValue) {
            result.author_oid = authorValue.Obj;
        }
    }

    switch (eventType) {
        case EventUnion.NotifyEvent: {
            const notify = eventData.event(new NotifyEvent());
            if (!notify) break;

            const value = notify.value();
            if (!value) break;

            result.type = "notify";
            result.content = new MoorVar(value).toJS();

            const contentTypeSym = notify.contentType();
            if (contentTypeSym && contentTypeSym.value()) {
                result.content_type = contentTypeSym.value();
            }

            result.no_newline = notify.noNewline();
            break;
        }

        case EventUnion.TracebackEvent: {
            const traceback = eventData.event(new TracebackEvent());
            if (!traceback) break;

            const exception = traceback.exception();
            if (!exception) break;

            result.type = "traceback";
            result.backtrace = [];

            for (let i = 0; i < exception.backtraceLength(); i++) {
                const backtraceVar = exception.backtrace(i);
                if (backtraceVar) {
                    const line = new MoorVar(backtraceVar).asString();
                    if (line) {
                        result.backtrace.push(line);
                    }
                }
            }
            break;
        }

        case EventUnion.PresentEvent: {
            const present = eventData.event(new PresentEvent());
            if (!present) break;

            const presentation = present.presentation();
            if (!presentation) break;

            result.type = "present";
            result.presentation = {
                id: presentation.id(),
                content: presentation.content(),
                content_type: presentation.contentType() || "text/plain",
                target: presentation.target(),
            };
            break;
        }

        case EventUnion.UnpresentEvent: {
            const unpresent = eventData.event(new UnpresentEvent());
            if (!unpresent) break;

            result.type = "unpresent";
            result.presentation_id = unpresent.presentationId();
            break;
        }

        default:
            result.type = "unknown";
            result.event_type_code = eventType;
    }

    return result;
}

// Fetch all history in batches
async function fetchAllHistoryEncrypted(authToken: string, ageIdentity: string): Promise<Uint8Array[]> {
    const allEncryptedBlobs: Uint8Array[] = [];
    let hasMore = true;
    let oldestEventId: string | undefined = undefined;
    const batchSize = 1000; // Fetch in large batches

    while (hasMore) {
        const params = new URLSearchParams();
        params.set("limit", batchSize.toString());

        // On first request, get all history by using a very large time range
        // Use 10 years (315,360,000 seconds) to ensure we get everything
        if (!oldestEventId) {
            params.set("since_seconds", "315360000"); // ~10 years
        } else {
            // On subsequent requests, use until_event for pagination
            params.set("until_event", oldestEventId);
        }

        const url = `/fb/api/history?${params}`;

        console.log(`[Worker] Fetching batch: ${url}`);
        const response = await fetch(url, {
            method: "GET",
            headers: {
                "X-Moor-Auth-Token": authToken,
            },
        });

        if (!response.ok) {
            throw new Error(`History fetch failed: ${response.status} ${response.statusText}`);
        }

        const arrayBuffer = await response.arrayBuffer();
        const bytes = new Uint8Array(arrayBuffer);

        // Parse the FlatBuffer response to extract encrypted blobs
        // This follows the same structure as fetchHistoryFlatBuffer in rpc-fb.ts
        // Import these at the top level instead of dynamically
        // (imports are at top of file now)
        const replyResult = ReplyResult.getRootAsReplyResult(
            new flatbuffers.ByteBuffer(bytes),
        );

        const resultType = replyResult.resultType();
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

        const eventsLength = historyResponse.eventsLength();

        console.log(`[Worker] Received ${eventsLength} events in this batch`);

        if (eventsLength === 0) {
            hasMore = false;
            break;
        }

        // Extract encrypted blobs and track oldest event ID for pagination
        for (let i = 0; i < eventsLength; i++) {
            const historicalEvent = historyResponse.events(i);
            if (!historicalEvent) continue;

            const encryptedBlob = historicalEvent.encryptedBlobArray();
            if (encryptedBlob) {
                allEncryptedBlobs.push(encryptedBlob);
            }

            // Track the event ID for the first event (oldest in this batch)
            if (i === 0 && encryptedBlob) {
                try {
                    // We need to decrypt briefly just to get the event ID for pagination
                    // This is unavoidable since event IDs are inside the encrypted blob
                    const decryptedBytes = await decryptEventBlob(encryptedBlob, ageIdentity);
                    const narrativeEvent = NarrativeEvent.getRootAsNarrativeEvent(
                        new flatbuffers.ByteBuffer(decryptedBytes),
                    );
                    const eventId = narrativeEvent.eventId()?.dataArray();
                    if (eventId) {
                        oldestEventId = Array.from(eventId).map((b: number) => b.toString(16).padStart(2, "0")).join(
                            "",
                        );
                    }
                } catch (err) {
                    console.error("Failed to extract event ID for pagination:", err);
                }
            }
        }

        // If we got fewer events than requested, we've reached the end
        if (eventsLength < batchSize) {
            hasMore = false;
        }
    }

    return allEncryptedBlobs;
}

// Worker state
declare const self: Worker & { ageIdentityCache: string };

// Handle messages from main thread
self.addEventListener("message", async (event: MessageEvent<StartExportMessage>) => {
    const { type, authToken, ageIdentity, systemTitle, playerOid } = event.data;

    if (type !== "start") {
        self.postMessage({ type: "error", error: "Invalid message type" } as ErrorMessage);
        return;
    }

    try {
        // Cache the age identity for the worker's lifetime
        self.ageIdentityCache = ageIdentity;

        // Step 1: Fetch all encrypted history
        self.postMessage({ type: "progress", processed: 0 } as ProgressMessage);

        const exportStartTime = Date.now();
        const encryptedBlobs = await fetchAllHistoryEncrypted(authToken, ageIdentity);
        const total = encryptedBlobs.length;

        // Step 2: Decrypt and convert to JSON
        const events: any[] = [];

        for (let i = 0; i < encryptedBlobs.length; i++) {
            const encryptedBlob = encryptedBlobs[i];

            try {
                const decryptedBytes = await decryptEventBlob(encryptedBlob, ageIdentity);
                const narrativeEvent = NarrativeEvent.getRootAsNarrativeEvent(
                    new flatbuffers.ByteBuffer(decryptedBytes),
                );

                const eventJSON = narrativeEventToJSON(narrativeEvent);
                if (eventJSON) {
                    events.push(eventJSON);
                }
            } catch (err) {
                console.error("Failed to decrypt/convert event:", err);
                // Continue with next event rather than failing the entire export
            }

            // Report progress every 100 events
            if ((i + 1) % 100 === 0 || i === total - 1) {
                self.postMessage({ type: "progress", processed: i + 1, total } as ProgressMessage);
            }
        }

        // Step 3: Create JSON blob with comprehensive metadata
        const exportEndTime = Date.now();
        const oldestEvent = events.length > 0 ? events[events.length - 1] : null;
        const newestEvent = events.length > 0 ? events[0] : null;

        const jsonString = JSON.stringify(
            {
                export_version: "1.0",
                export_date: new Date().toISOString(),
                system_title: systemTitle,
                player_oid: playerOid,
                event_count: events.length,
                time_range: {
                    oldest_event: oldestEvent ? oldestEvent.timestamp : null,
                    newest_event: newestEvent ? newestEvent.timestamp : null,
                    export_duration_ms: exportEndTime - exportStartTime,
                },
                events,
            },
            null,
            2,
        );

        const jsonBlob = new Blob([jsonString], { type: "application/json" });

        // Step 4: Send completion message
        self.postMessage({ type: "complete", jsonBlob } as CompleteMessage);
    } catch (error) {
        self.postMessage({
            type: "error",
            error: error instanceof Error ? error.message : String(error),
        } as ErrorMessage);
    }
});
