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
//

import { CompileError } from "@moor/schema/generated/moor-common/compile-error";
import { NarrativeEvent } from "@moor/schema/generated/moor-common/narrative-event";
import { CurrentPresentations } from "@moor/schema/generated/moor-rpc/current-presentations";
import { ListObjectsReply } from "@moor/schema/generated/moor-rpc/list-objects-reply";
import { PropertiesReply } from "@moor/schema/generated/moor-rpc/properties-reply";
import { PropertyUpdated } from "@moor/schema/generated/moor-rpc/property-updated";
import { PropertyValue } from "@moor/schema/generated/moor-rpc/property-value";
import { VerbValue } from "@moor/schema/generated/moor-rpc/verb-value";
import { VerbsReply } from "@moor/schema/generated/moor-rpc/verbs-reply";
import type { NarrativeContentType, ParsedNarrativeEvent } from "@moor/web-sdk";
import {
    extractWelcomeMessage,
    MoorApiError,
    parseClientReplyAs,
    parseClientReplyUnion,
    parseNarrativeEvent,
    parseVerbCallSuccessFromBytes,
    parseVerbCallSuccessFromReply,
    parseVerbProgramCompileOutcome,
} from "@moor/web-sdk";

import { MoorVar } from "./MoorVar";
import { authHeaders, moorApi } from "./rpc-fb-shared";

export interface VerbInvocationResult {
    result: unknown;
    output: Array<
        {
            eventId: string;
            timestamp: Date;
            author: unknown;
        } & ParsedNarrativeEvent
    >;
}

export async function getVerbsFlatBuffer(
    authToken: string,
    objectCurie: string,
    inherited: boolean = true,
): Promise<VerbsReply> {
    const params = new URLSearchParams();
    if (inherited) {
        params.set("inherited", "true");
    }

    const headers = authHeaders(authToken);
    const bytes = await moorApi.getFlatBuffer(`/v1/verbs/${objectCurie}?${params}`, {
        method: "GET",
        headers,
    });
    return parseClientReplyAs(bytes, "Get verbs", VerbsReply);
}

export async function getVerbCodeFlatBuffer(
    authToken: string,
    objectCurie: string,
    verbName: string,
): Promise<VerbValue> {
    const headers = authHeaders(authToken);
    const bytes = await moorApi.getFlatBuffer(`/v1/verbs/${objectCurie}/${encodeURIComponent(verbName)}`, {
        method: "GET",
        headers,
    });
    return parseClientReplyAs(bytes, "Get verb code", VerbValue);
}

export async function invokeVerbFlatBuffer(
    authToken: string,
    objectCurie: string,
    verbName: string,
    args?: Uint8Array,
): Promise<VerbInvocationResult> {
    const argsBytes = args ?? MoorVar.buildEmptyList();
    const bodyBuffer = argsBytes.slice(0, argsBytes.length).buffer as ArrayBuffer;

    const headers = authHeaders(authToken, { "Content-Type": "application/x-flatbuffers" });
    const bytes = await moorApi.getFlatBuffer(`/v1/verbs/${objectCurie}/${encodeURIComponent(verbName)}/invoke`, {
        method: "POST",
        headers,
        body: bodyBuffer,
    });
    const { response: verbCallResponse, success: verbCallSuccess } = parseVerbCallSuccessFromBytes(
        bytes,
        "Verb invocation failed",
    );

    const resultVar = verbCallSuccess.result();
    const result = resultVar ? new MoorVar(resultVar).toJS() : null;

    const output: VerbInvocationResult["output"] = [];
    const outputCount = verbCallResponse.outputLength();
    for (let i = 0; i < outputCount; i++) {
        const narrativeEvent = verbCallResponse.output(i, new NarrativeEvent());
        if (!narrativeEvent) {
            continue;
        }

        const eventIdBytes = narrativeEvent.eventId()?.dataArray();
        const eventId = eventIdBytes
            ? Array.from(eventIdBytes as ArrayLike<number>).map((b) => b.toString(16).padStart(2, "0")).join("")
            : "";
        const timestamp = new Date(Number(narrativeEvent.timestamp()) / 1000000);
        const authorVar = narrativeEvent.author();
        const eventObj = parseNarrativeEvent(
            narrativeEvent,
            (value) => new MoorVar(value).toJS(),
            (value) => new MoorVar(value).asString(),
        );
        if (!eventObj) {
            continue;
        }

        output.push({
            eventId,
            timestamp,
            author: authorVar ? new MoorVar(authorVar).toJS() : null,
            ...eventObj,
        });
    }

    return { result, output };
}

export async function getPropertiesFlatBuffer(
    authToken: string,
    objectCurie: string,
    inherited: boolean = true,
): Promise<PropertiesReply> {
    const params = new URLSearchParams();
    if (inherited) {
        params.set("inherited", "true");
    }

    const headers = authHeaders(authToken);
    const bytes = await moorApi.getFlatBuffer(`/v1/properties/${objectCurie}?${params}`, {
        method: "GET",
        headers,
    });
    return parseClientReplyAs(bytes, "Get properties", PropertiesReply);
}

export async function getPropertyFlatBuffer(
    authToken: string,
    objectCurie: string,
    propertyName: string,
): Promise<PropertyValue> {
    const headers = authHeaders(authToken);
    const bytes = await moorApi.getFlatBuffer(`/v1/properties/${objectCurie}/${encodeURIComponent(propertyName)}`, {
        method: "GET",
        headers,
    });
    return parseClientReplyAs(bytes, "Get property", PropertyValue);
}

export async function getCurrentPresentationsFlatBuffer(
    authToken: string,
): Promise<CurrentPresentations> {
    const headers = authHeaders(authToken);
    const bytes = await moorApi.getFlatBuffer(`/v1/presentations`, {
        method: "GET",
        headers,
    });
    return parseClientReplyAs(bytes, "Get presentations", CurrentPresentations);
}

export async function compileVerbFlatBuffer(
    authToken: string,
    objectCurie: string,
    verbName: string,
    code: string,
): Promise<{ success: true } | { success: false; error: CompileError | string }> {
    const headers = authHeaders(authToken);
    const response = await moorApi.request(`/v1/verbs/${objectCurie}/${encodeURIComponent(verbName)}`, {
        method: "POST",
        headers,
        body: code,
    });
    // Programming failures retain the typed diagnostic body on non-2xx responses.
    const contentType = response.headers.get("content-type")?.split(";", 1)[0].trim();
    if (
        !response.ok && !(contentType === "application/x-flatbuffers" && [403, 404, 422, 500].includes(response.status))
    ) {
        throw new MoorApiError("transport", `Compile verb failed: ${response.status} ${response.statusText}`, {
            status: response.status,
        });
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    const replyUnion = parseClientReplyUnion(bytes, "Compile verb");

    const compileResult = parseVerbProgramCompileOutcome(replyUnion, "Compile verb");
    if (compileResult.success) {
        return { success: true };
    }
    return { success: false, error: compileResult.error.compileError ?? compileResult.error.message };
}

export async function invokeWelcomeMessageFlatBuffer(): Promise<{
    welcomeMessage: string;
    contentType: NarrativeContentType;
}> {
    try {
        const bytes = await moorApi.getFlatBuffer(`/v1/invoke_welcome_message`, {
            method: "GET",
        });
        if (bytes.length === 0) {
            return { welcomeMessage: "", contentType: "text/plain" };
        }

        const replyUnion = parseClientReplyUnion(bytes, "Invoke welcome message");
        const { response: verbCallResponse } = parseVerbCallSuccessFromReply(
            replyUnion,
            "Invoke welcome message failed",
        );

        return extractWelcomeMessage(
            verbCallResponse,
            (value) => new MoorVar(value).toJS(),
        );
    } catch (err) {
        console.error("Exception during welcome message invocation:", err);
        throw err;
    }
}

export async function listObjectsFlatBuffer(
    authToken: string,
): Promise<ListObjectsReply> {
    const headers = authHeaders(authToken);
    const bytes = await moorApi.getFlatBuffer(`/v1/objects`, {
        method: "GET",
        headers,
    });
    return parseClientReplyAs(bytes, "List objects", ListObjectsReply);
}

export async function updatePropertyFlatBuffer(
    authToken: string,
    objectCurie: string,
    propertyName: string,
    value: string,
): Promise<void> {
    const headers = authHeaders(authToken, { "Content-Type": "text/plain" });
    const bytes = await moorApi.getFlatBuffer(`/v1/properties/${objectCurie}/${encodeURIComponent(propertyName)}`, {
        method: "POST",
        headers,
        body: value,
    });
    parseClientReplyAs(bytes, "Update property", PropertyUpdated);
}
