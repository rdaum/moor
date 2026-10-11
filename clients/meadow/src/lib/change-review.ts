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

import { buildStructuredArgs, SemanticAnnotation, StructuredArgument } from "@moor/web-sdk";
import { changeServiceFailure } from "./change-service";
import { invokeVerbFlatBuffer } from "./rpc-fb";

export type ChangeTarget = Extract<SemanticAnnotation, { kind: "change" }>;
export const changeLabels = {
    conflict: "Conflicts",
    upstream: "Upstream edits",
    local: "Local edits",
    converged: "Already match",
    unbased: "Without a baseline",
    unchanged: "Unchanged",
    local_only: "Local only",
    incoming_only: "Upstream only",
} as const;
export type Classification = keyof typeof changeLabels;
export type Choice = "incoming" | "local" | "edited" | "defer" | "unresolved";
export interface CompilerDiagnostic {
    message: string;
    startLineNumber: number;
    startColumn: number;
    endLineNumber: number;
    endColumn: number;
}
export interface ReviewDiagnostic {
    code: string;
    label: string;
    message?: string;
}
export interface DiagnosticPage {
    rows: ReviewDiagnostic[];
    next: number;
    total: number;
}
export interface ReviewReceipt {
    completedAt: number;
    decisionCount: number;
    decisions: { id: string; label: string; choice: string; accepted: string }[];
}
export interface ChangeRow {
    id: string;
    label: string;
    objectKey: string;
    objectLabel: string;
    memberLabel: string;
    classification: Classification;
    eligible: boolean;
    blockers: string[];
    choices: Choice[];
    default: Choice;
    choice?: { choice: Choice; program?: string; validated?: boolean };
    base?: string;
    live: string;
    incoming: string;
    live_text?: string;
    incoming_text?: string;
    base_text?: string;
    base_fetch_available?: boolean;
    read_only?: boolean;
    field?: string;
    live_present?: boolean;
    incoming_present?: boolean;
    inspection_error?: string;
    validation?: CompilerDiagnostic[];
}
export interface ReviewPage {
    generation: number;
    rows: ChangeRow[];
    cursor: number[];
    counts: Partial<Record<Classification, number>>;
    decision_counts: { selected: number; unresolved: number; blocked: number };
    operation: "update" | "adopt";
}
export interface ReviewStatus {
    generation: number;
    package: string;
    status: string;
    error?: { message?: string };
    provenance?: Record<string, unknown>;
    receipt?: ReviewReceipt;
}
export interface InspectionPage {
    revision: string;
    rows: ChangeRow[];
    counts: Partial<Record<Classification, number>>;
    next: number;
}

function object(value: unknown): Record<string, unknown> {
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid change review response.");
    return value as Record<string, unknown>;
}
function number(value: unknown): number {
    if (!Number.isSafeInteger(value) || (value as number) < 0) throw new Error("Invalid change review number.");
    return value as number;
}
function text(value: unknown): string {
    if (typeof value !== "string") throw new Error("Invalid change review text.");
    return value;
}
function choice(value: unknown): Choice {
    if (!["incoming", "local", "edited", "defer", "unresolved"].includes(value as string)) {
        throw new Error("Invalid review choice.");
    }
    return value as Choice;
}
function compilerDiagnostic(value: unknown): CompilerDiagnostic {
    const item = object(value);
    return {
        message: text(item.message),
        startLineNumber: Math.max(1, number(item.line ?? 1)),
        startColumn: Math.max(1, number(item.column ?? 1)),
        endLineNumber: Math.max(1, number(item.end_line ?? item.line ?? 1)),
        endColumn: Math.max(2, number(item.end_column ?? 2)),
    };
}
function receipt(value: unknown): ReviewReceipt {
    const item = object(value);
    if (!Array.isArray(item.decisions)) throw new Error("Invalid review receipt.");
    return {
        completedAt: number(item.completed_at),
        decisionCount: number(item.decision_count),
        decisions: item.decisions.map(value => {
            const decision = object(value);
            return {
                id: text(decision.id),
                label: text(decision.label ?? decision.id),
                choice: text(decision.choice),
                accepted: text(decision.accepted),
            };
        }),
    };
}
export function decodeChangeRow(value: unknown, details = false): ChangeRow {
    const row = object(value);
    const classification = text(row.classification);
    if (!Object.hasOwn(changeLabels, classification) || !Array.isArray(row.choices) || !Array.isArray(row.blockers)) {
        throw new Error("Invalid change review row.");
    }
    const label = text(row.label);
    const field = typeof row.field === "string" ? row.field : "program";
    const separator = field === "program" ? ":" : ".";
    const member = field === "object" ? "" : typeof row.name === "string"
        ? field === "program" ? row.name.replace(/^\d+:/, "") : row.name
        : field === "program" && Array.isArray(row.names) && row.names.length
        ? text(row.names[0])
        : label.slice(label.lastIndexOf(separator) + 1);
    const objectLabel = member && label.endsWith(separator + member)
        ? label.slice(0, -(member.length + 1))
        : label;
    const saved = object(row.choice ?? {});
    return {
        read_only: row.read_only === true || row.read_only === 1,
        field,
        objectKey: row.object ? JSON.stringify(row.object) : objectLabel,
        objectLabel,
        memberLabel: member ? separator + member : objectLabel,
        live_present: row.live_present !== false && row.live_present !== 0,
        incoming_present: row.incoming_present !== false && row.incoming_present !== 0,
        inspection_error: typeof row.inspection_error === "string" ? row.inspection_error : undefined,
        validation: Array.isArray(row.validation) ? row.validation.map(compilerDiagnostic) : [],
        id: text(row.id),
        label,
        classification: classification as Classification,
        eligible: row.eligible === true || row.eligible === 1,
        default: choice(row.default),
        choices: row.choices.map(choice),
        blockers: row.blockers.map(text),
        ...(saved.choice
            ? {
                choice: {
                    choice: choice(saved.choice),
                    validated: saved.validated === true,
                    ...(typeof saved.program === "string" ? { program: saved.program } : {}),
                },
            }
            : {}),
        base: typeof row.base === "string" ? row.base : undefined,
        base_fetch_available: row.base_fetch_available === true || row.base_fetch_available === 1,
        live: text(row.live),
        incoming: text(row.incoming),
        ...(details ? { live_text: text(row.live_text), incoming_text: text(row.incoming_text) } : {}),
        ...((row.base_text_available === true || row.base_text_available === 1) && typeof row.base_text === "string"
            ? { base_text: row.base_text }
            : {}),
    };
}

/** A matching content hash gives us the accepted source without storing another copy. */
export function baselineText(row: ChangeRow): string | undefined {
    if (row.base_text !== undefined) return row.base_text;
    if (!row.base) return undefined;
    if (row.base === row.live) return row.live_text;
    if (row.base === row.incoming) return row.incoming_text;
    return undefined;
}

export class ChangeReviewClient {
    constructor(private token: string, private target: ChangeTarget) {}

    private async invoke(
        method:
            | "status"
            | "review"
            | "details"
            | "base_source"
            | "inspection"
            | "diagnostics"
            | "resolve"
            | "apply"
            | "refresh",
        args: StructuredArgument[],
    ) {
        const { result } = await invokeVerbFlatBuffer(
            this.token,
            this.target.provider,
            method,
            buildStructuredArgs(args),
        ).catch(failure => {
            throw changeServiceFailure(failure);
        });
        const response = object(result);
        if (response.schema !== 1 || response.review_id !== this.target.review) {
            throw new Error("Unexpected change review response.");
        }
        return response;
    }
    async status(): Promise<ReviewStatus> {
        const status = await this.invoke("status", [this.target.review]);
        return {
            generation: number(status.generation),
            package: text(status.package),
            status: text(status.status),
            provenance: object(status.provenance ?? {}),
            ...(status.decision_count !== undefined
                ? { receipt: receipt(status) }
                : status.last_receipt
                ? { receipt: receipt(status.last_receipt) }
                : {}),
            ...(status.error && object(status.error).message
                ? { error: { message: text(object(status.error).message) } }
                : {}),
        };
    }
    async diagnostics(generation: number, offset = 1): Promise<DiagnosticPage> {
        const result = await this.invoke("diagnostics", [this.target.review, generation, offset]);
        if (result.generation !== generation || !Array.isArray(result.diagnostics)) {
            throw new Error("Review changed. Reload it before continuing.");
        }
        return {
            rows: result.diagnostics.map(value => {
                const item = object(value);
                return {
                    code: text(item.code),
                    label: text(item.label ?? ""),
                    ...(typeof item.message === "string" ? { message: item.message } : {}),
                };
            }),
            next: number(result.next),
            total: number(result.total),
        };
    }
    async page(generation: number, cursor: number[] = [], classification = ""): Promise<ReviewPage> {
        const page = await this.invoke("review", [this.target.review, generation, cursor, classification]);
        if (page.generation !== generation || !Array.isArray(page.rows) || !Array.isArray(page.cursor)) {
            throw new Error("Review changed. Reload it before continuing.");
        }
        const cursorValues = page.cursor.map(number);
        if (
            cursorValues.length
            && (cursorValues.length !== 3 || cursorValues[0] !== this.target.review || cursorValues[1] !== generation)
        ) {
            throw new Error("Invalid review cursor.");
        }
        const rawCounts = object(page.counts);
        const counts = Object.fromEntries(Object.keys(changeLabels).map(key => [key, number(rawCounts[key] ?? 0)]));
        const decisions = object(page.decision_counts);
        return {
            generation,
            rows: page.rows.map(row => decodeChangeRow(row)),
            cursor: cursorValues,
            counts,
            decision_counts: {
                selected: number(decisions.selected),
                unresolved: number(decisions.unresolved),
                blocked: number(decisions.blocked),
            },
            operation: page.operation === "adopt" ? "adopt" : "update",
        };
    }
    async details(generation: number, row: string): Promise<ChangeRow> {
        const result = await this.invoke("details", [this.target.review, generation, row]);
        const detail = decodeChangeRow(result.row, true);
        if (result.generation !== generation || detail.id !== row) {
            throw new Error("Review changed. Reload it before continuing.");
        }
        return detail;
    }
    async baseSource(generation: number, row: string, sources: { label: string; text: string }[] = []) {
        const result = await this.invoke("base_source", [this.target.review, generation, row, sources]);
        const detail = decodeChangeRow(result.row, true);
        if (result.generation !== generation + 1 || detail.id !== row || detail.base_text === undefined) {
            throw new Error("Unexpected historical source response. Reload the review.");
        }
        return { generation: result.generation as number, row: detail };
    }
    async inspection(generation: number, offset = 1, classification = "", revision = ""): Promise<InspectionPage> {
        const result = await this.invoke("inspection", [
            this.target.review,
            generation,
            offset,
            classification,
            "",
            revision,
        ]);
        if (result.generation !== generation || !Array.isArray(result.rows)) {
            throw new Error("Review changed. Reload it before continuing.");
        }
        const counts = object(result.counts);
        return {
            revision: text(result.revision),
            rows: result.rows.map(row => decodeChangeRow(row)),
            counts: Object.fromEntries(Object.entries(counts).map(([key, value]) => [key, number(value)])),
            next: number(result.next),
        };
    }
    async resolve(generation: number, row: string, selected: Choice, program = "") {
        const result = await this.invoke("resolve", [this.target.review, generation, row, selected, program]);
        const next = number(result.generation);
        if (next !== generation + 1 || !Array.isArray(result.validation)) {
            throw new Error("Unexpected choice response. Reload the review.");
        }
        const errors = result.validation.map(object).filter(item => item.id === row && item.valid === false)
            .map(item => `Line ${number(item.line ?? 1)}: ${text(item.message)}`);
        return { generation: next, errors };
    }
    async apply(generation: number): Promise<ReviewStatus> {
        await this.invoke("apply", [this.target.review, generation]);
        return this.status();
    }
    async refresh(generation: number): Promise<number> {
        const result = await this.invoke("refresh", [this.target.review, generation]);
        const next = number(result.generation);
        if (next !== generation + 1) throw new Error("Unexpected refresh response. Reload the review.");
        return next;
    }
}
