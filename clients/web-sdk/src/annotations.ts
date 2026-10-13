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

// Copyright (C) 2026 The mooR Authors
// SPDX-License-Identifier: LGPL-3.0-or-later

export interface ActionHint {
    icon?: string;
    label: string;
    title?: string;
}

export type ObjectKind = "object" | "person" | "container";

/** Semantic references are data, scoped to the event that owns their anchors. */
export type SemanticAnnotation =
    | { kind: "object"; ref: string; objectKind?: ObjectKind }
    | (
        & { kind: "command"; exit?: ExitDescriptor; action?: ActionHint }
        & (
            | { command: string; template?: never; arguments?: never }
            | { command?: never; template: string; arguments: Record<string, CommandArgument> }
        )
    )
    | { kind: "help"; provider: string; topic: string }
    | { kind: "verb"; receiver: string; name: string; definer?: string }
    | { kind: "property"; object: string; name: string }
    | { kind: "change"; provider: string; review: number; generation: number; row?: string };

export type ExitDescriptor =
    | { source: string; destination: string; passage: string }
    | { source: string; provider: string; registration: string; option: string; destination?: string };

export interface CommandArgument {
    label: string;
    expectedKind: "object" | "text";
    required?: boolean;
    suggestions?: { provider: string; source: string };
    bound?: { value: string; label: string };
}

export type AnnotationTable = Readonly<Record<string, SemanticAnnotation>>;
export const MAX_ANNOTATIONS = 512;
export const MAX_ANNOTATION_SIZE = 2048;
// Count and per-entry limits bound the table, including anchor IDs and JSON punctuation.
export const MAX_ANNOTATION_TABLE_SIZE = 600_000;
export const ANNOTATION_ID = /^[a-zA-Z][a-zA-Z0-9_-]{0,63}$/;

function record(value: unknown): value is Record<string, unknown> {
    return value !== null && typeof value === "object" && !Array.isArray(value);
}

function text(value: unknown, max = 256): value is string {
    return typeof value === "string" && value.length > 0 && value.length <= max && !/[\u0000-\u001f\u007f]/.test(value);
}

function reference(value: unknown): value is string {
    return text(value, 64) && /^(?:oid:\d+|uuid:[\da-fA-F]{6}-[\da-fA-F]{10})$/.test(value);
}

/** Reject malformed entries individually; their captured text can still be displayed. */
export function decodeAnnotation(value: unknown): SemanticAnnotation | undefined {
    if (!record(value)) return undefined;
    let size: number;
    try {
        size = JSON.stringify(value).length;
    } catch {
        return undefined;
    }
    if (size > MAX_ANNOTATION_SIZE) return undefined;
    switch (value.kind) {
        case "change":
            return reference(value.provider)
                    && Number.isSafeInteger(value.review) && (value.review as number) > 0
                    && Number.isSafeInteger(value.generation) && (value.generation as number) > 0
                    && (value.row === undefined || text(value.row))
                ? {
                    kind: "change",
                    provider: value.provider,
                    review: value.review as number,
                    generation: value.generation as number,
                    ...(value.row === undefined ? {} : { row: value.row as string }),
                }
                : undefined;
        case "object":
            return reference(value.ref)
                ? {
                    kind: "object",
                    ref: value.ref,
                    ...(value.objectKind === "person" || value.objectKind === "container"
                            || value.objectKind === "object"
                        ? { objectKind: value.objectKind }
                        : {}),
                }
                : undefined;
        case "help":
            return reference(value.provider) && text(value.topic)
                ? { kind: "help", provider: value.provider, topic: value.topic }
                : undefined;
        case "verb":
            if (
                !reference(value.receiver) || !text(value.name)
                || (value.definer !== undefined && !reference(value.definer))
            ) {
                return undefined;
            }
            return {
                kind: "verb",
                receiver: value.receiver,
                name: value.name,
                definer: value.definer as string | undefined,
            };
        case "property":
            return reference(value.object) && text(value.name)
                ? { kind: "property", object: value.object, name: value.name }
                : undefined;
        case "command": {
            let action: ActionHint | undefined;
            if (value.action !== undefined) {
                if (
                    !record(value.action) || !text(value.action.label, 80)
                    || (value.action.icon !== undefined && !text(value.action.icon, 32))
                    || (value.action.title !== undefined && !text(value.action.title))
                ) return undefined;
                action = {
                    label: value.action.label,
                    icon: value.action.icon as string | undefined,
                    title: value.action.title as string | undefined,
                };
            }
            let exit: ExitDescriptor | undefined;
            if (value.exit !== undefined) {
                if (!record(value.exit) || !reference(value.exit.source)) return undefined;
                if (value.exit.passage !== undefined) {
                    if (
                        !reference(value.exit.destination) || !text(value.exit.passage, 128)
                        || value.exit.provider !== undefined || value.exit.registration !== undefined
                        || value.exit.option !== undefined
                    ) return undefined;
                    exit = {
                        source: value.exit.source,
                        destination: value.exit.destination,
                        passage: value.exit.passage,
                    };
                } else {
                    if (
                        !reference(value.exit.provider) || !text(value.exit.registration, 128)
                        || !text(value.exit.option, 128)
                        || (value.exit.destination !== undefined && !reference(value.exit.destination))
                    ) return undefined;
                    exit = {
                        source: value.exit.source,
                        provider: value.exit.provider,
                        registration: value.exit.registration,
                        option: value.exit.option,
                        ...(value.exit.destination !== undefined ? { destination: value.exit.destination } : {}),
                    };
                }
            }
            if (text(value.command, 1024) && value.template === undefined && value.arguments === undefined) {
                return { kind: "command", command: value.command, exit, ...(action ? { action } : {}) };
            }
            if (value.command !== undefined || !text(value.template, 1024) || !record(value.arguments) || exit) {
                return undefined;
            }
            const fields = Object.entries(value.arguments);
            if (fields.length < 1 || fields.length > 2) return undefined;
            const args: Record<string, CommandArgument> = Object.create(null);
            for (const [slot, field] of fields) {
                if (
                    !/^(dobj|iobj)$/.test(slot) || !record(field) || !text(field.label)
                    || !["object", "text"].includes(field.expectedKind as string)
                    || (field.required !== undefined && typeof field.required !== "boolean")
                ) return undefined;
                let suggestions: CommandArgument["suggestions"];
                if (field.suggestions !== undefined) {
                    if (
                        !record(field.suggestions) || !reference(field.suggestions.provider)
                        || !text(field.suggestions.source)
                    ) return undefined;
                    suggestions = { provider: field.suggestions.provider, source: field.suggestions.source };
                }
                let bound: CommandArgument["bound"];
                if (field.bound !== undefined) {
                    if (
                        !record(field.bound) || !text(field.bound.value) || !text(field.bound.label)
                        || (field.expectedKind === "object" && !reference(field.bound.value))
                    ) return undefined;
                    bound = { value: field.bound.value, label: field.bound.label };
                }
                args[slot] = {
                    label: field.label,
                    expectedKind: field.expectedKind as "object" | "text",
                    required: field.required as boolean | undefined,
                    suggestions,
                    bound,
                };
            }
            const slots = [...value.template.matchAll(/\{([^{}]+)\}/g)].map(match => match[1]);
            if (
                slots.length !== fields.length || slots.some(slot => !Object.hasOwn(args, slot))
                || new Set(slots).size !== slots.length
            ) return undefined;
            return { kind: "command", template: value.template, arguments: args, ...(action ? { action } : {}) };
        }
        default:
            return undefined;
    }
}

export function decodeAnnotations(value: unknown): AnnotationTable {
    const result: Record<string, SemanticAnnotation> = Object.create(null);
    if (!record(value)) return result;
    let total = 0;
    for (const id of Object.keys(value).slice(0, MAX_ANNOTATIONS)) {
        if (!ANNOTATION_ID.test(id)) continue;
        const annotation = decodeAnnotation(value[id]);
        if (!annotation) continue;
        total += id.length + JSON.stringify(annotation).length;
        if (total > MAX_ANNOTATION_TABLE_SIZE) break;
        result[id] = annotation;
    }
    return result;
}
