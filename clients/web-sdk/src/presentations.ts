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

import { Presentation as PresentationFB } from "@moor/schema/generated/moor-common/presentation";
import { PresentationSnapshot } from "@moor/schema/generated/moor-rpc/presentation-snapshot";
import * as flatbuffers from "flatbuffers";

export type PresentationContentType = "text/plain" | "text/djot" | "text/html";

export interface ParsedPresentation {
    id: string;
    target: string;
    content: string;
    contentType: PresentationContentType;
    attributes: Array<[string, string]>;
}

export interface PresentationData {
    readonly id: string;
    readonly content_type: PresentationContentType;
    readonly content: string;
    readonly target: string;
    readonly attributes: ReadonlyArray<readonly [string, string]>;
}

export interface PresentationParseFallback {
    id?: string;
    target?: string;
    content?: string;
    contentType?: string | null;
}

export interface ParsedPresentationSnapshot {
    id: string;
    encryptedBlob: Uint8Array;
}

export function normalizePresentationContentType(value: string | null | undefined): PresentationContentType {
    if (value === "text_djot" || value === "text/djot") {
        return "text/djot";
    }
    if (value === "text_html" || value === "text/html") {
        return "text/html";
    }
    return "text/plain";
}

export function parsePresentationSnapshot(
    snapshot: PresentationSnapshot | null,
): ParsedPresentationSnapshot | null {
    if (!snapshot) {
        return null;
    }
    const encryptedBlob = snapshot.encryptedBlobArray();
    if (!encryptedBlob || encryptedBlob.length === 0) {
        return null;
    }
    return {
        id: snapshot.id() || "",
        encryptedBlob,
    };
}

export function parsePresentationValue(
    presentation: PresentationFB | null,
    fallback: PresentationParseFallback = {},
): ParsedPresentation | null {
    if (!presentation) {
        return null;
    }

    const attributes: Array<[string, string]> = [];
    const attrsLength = presentation.attributesLength();
    for (let i = 0; i < attrsLength; i++) {
        const attr = presentation.attributes(i);
        const key = attr?.key();
        const value = attr?.value();
        if (!key || !value) {
            continue;
        }
        attributes.push([key, value]);
    }

    return {
        id: presentation.id() || fallback.id || "",
        target: presentation.target() || fallback.target || "window",
        content: presentation.content() || fallback.content || "",
        contentType: normalizePresentationContentType(
            presentation.contentType() || fallback.contentType,
        ),
        attributes,
    };
}

export function parsePresentationBytes(
    presentationBytes: Uint8Array,
    fallback: PresentationParseFallback = {},
): ParsedPresentation | null {
    if (presentationBytes.length === 0) {
        return null;
    }

    const decodedPresentation = PresentationFB.getRootAsPresentation(
        new flatbuffers.ByteBuffer(presentationBytes),
    );
    return parsePresentationValue(decodedPresentation, fallback);
}

export function toPresentationData(presentation: ParsedPresentation): PresentationData {
    return {
        id: presentation.id,
        target: presentation.target,
        content: presentation.content,
        content_type: presentation.contentType,
        attributes: presentation.attributes,
    };
}
