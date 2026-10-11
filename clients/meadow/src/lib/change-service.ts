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

import { ErrorCode } from "@moor/schema/generated/moor-common/error-code";
import { SchedulerErrorUnion } from "@moor/schema/generated/moor-rpc/scheduler-error-union";
import { TaskAbortedException } from "@moor/schema/generated/moor-rpc/task-aborted-exception";
import {
    buildStructuredArgs,
    jsObjectRefToCurie,
    MoorVar,
    ObjectArgument,
    type StructuredArgument,
    VerbCallFailure,
} from "@moor/web-sdk";
import { type ChangeTarget } from "./change-review";
import { getPropertyFlatBuffer, invokeVerbFlatBuffer } from "./rpc-fb";

export interface ChangeCapabilities {
    operations: string[];
    fields: string[];
    choices: string[];
    transports: string[];
    defaultPackage: string;
    maxSourceBytes: number;
    maxPackages: number;
    gitAuthorization: string;
}
export type Upstream = string | {
    transport: "git";
    repository: string;
    revision: { ref: string } | { commit: string };
    path: string;
};
export interface ChangePackage {
    name: string;
    generation: number;
    objects: string[];
    constants: Record<string, unknown>;
    trustedOwners: string[];
    upstream: Upstream;
    active: number;
    proposedBindings: Record<string, string>;
    complete: string[];
}
export interface CompletedReview {
    review: number;
    generation: number;
    status: string;
    completedAt: number;
    decisionCount: number;
}
export interface PackageConfiguration {
    name: string;
    generation: number;
    objects: string[];
    constants: Record<string, StructuredArgument>;
    trustedOwners: string[];
    upstream: Upstream;
}
function record(value: unknown): Record<string, unknown> {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
        throw new Error("Invalid change service response.");
    }
    return value as Record<string, unknown>;
}
function text(value: unknown): string {
    if (typeof value !== "string") throw new Error("Invalid change service text.");
    return value;
}
function integer(value: unknown): number {
    if (!Number.isSafeInteger(value) || (value as number) < 0) throw new Error("Invalid change service number.");
    return value as number;
}
function strings(value: unknown): string[] {
    if (!Array.isArray(value)) throw new Error("Invalid change service list.");
    return value.map(text);
}
function references(value: unknown): string[] {
    if (!Array.isArray(value)) throw new Error("Invalid package bindings.");
    return value.map(item => {
        const reference = jsObjectRefToCurie(item);
        if (!reference) throw new Error("Invalid package object reference.");
        return reference;
    });
}
function decodeUpstream(value: unknown): Upstream {
    if (typeof value === "string") return value;
    const source = record(value);
    if (source.transport !== "git") throw new Error("Unsupported upstream transport.");
    const revision = record(source.revision);
    if (Object.keys(revision).length !== 1) throw new Error("Invalid upstream revision.");
    return {
        transport: "git",
        repository: text(source.repository),
        revision: Object.hasOwn(revision, "ref") ? { ref: text(revision.ref) } : { commit: text(revision.commit) },
        path: text(source.path ?? ""),
    };
}
function decodePackage(name: string, value: unknown): ChangePackage {
    const item = record(value);
    if (item.schema !== 1) throw new Error("Unsupported package schema.");
    return {
        name,
        generation: integer(item.generation),
        objects: references(item.objects),
        constants: record(item.constants),
        trustedOwners: references(item.trusted_owners),
        upstream: decodeUpstream(item.upstream),
        active: integer(item.active),
        proposedBindings: Object.fromEntries(
            Object.entries(record(item.proposed_bindings ?? {})).map(([name, value]) => {
                const reference = jsObjectRefToCurie(value);
                if (!reference) throw new Error("Invalid proposed object binding.");
                return [name, reference];
            }),
        ),
        complete: references(record(item.policy ?? {}).complete ?? []),
    };
}

/** Retain the service's explanation when the RPC task raises an exception. */
export function changeServiceFailure(failure: unknown): Error {
    if (failure instanceof VerbCallFailure) {
        const scheduler = failure.failure.error();
        if (scheduler?.errorType() === SchedulerErrorUnion.TaskAbortedException) {
            const exception = scheduler.error(new TaskAbortedException()) as TaskAbortedException | null;
            const error = exception?.exception()?.error();
            if (error) {
                return new Error(error.msg() || ErrorCode[error.errType()] || "Service request failed.", {
                    cause: failure,
                });
            }
        }
    }
    return failure instanceof Error ? failure : new Error("Service request failed.");
}

/** Discover the core's conventional service binding; authentication and capabilities determine access. */
export async function discoverChangeService(token: string): Promise<ChangeServiceClient | null> {
    const property = await getPropertyFlatBuffer(token, "oid:0", "change_manager");
    const binding = property.value();
    if (binding === null) return null;
    const provider = jsObjectRefToCurie(new MoorVar(binding).asObject());
    if (!provider) throw new Error("Invalid change service binding.");
    return ChangeServiceClient.connect(token, provider);
}

export class ChangeServiceClient {
    private constructor(
        private token: string,
        readonly provider: string,
        readonly capabilities: ChangeCapabilities,
    ) {}

    static async connect(token: string, provider: string): Promise<ChangeServiceClient> {
        const { result } = await invokeVerbFlatBuffer(token, provider, "capabilities", buildStructuredArgs([]));
        const value = record(result);
        if (value.schema !== 1) throw new Error("Unsupported change service schema.");
        const capabilities: ChangeCapabilities = {
            operations: strings(value.operations),
            fields: strings(value.fields),
            choices: strings(value.choices),
            transports: strings(value.transports),
            defaultPackage: text(value.default_package),
            maxSourceBytes: integer(value.max_source_bytes),
            maxPackages: integer(value.max_packages),
            gitAuthorization: text(value.git_authorization),
        };
        if (!capabilities.operations.includes("packages")) throw new Error("This service does not expose packages.");
        return new ChangeServiceClient(token, provider, capabilities);
    }

    supports(operation: string): boolean {
        return this.capabilities.operations.includes(operation);
    }
    private async invoke(method: string, args: StructuredArgument[]) {
        if (!this.supports(method)) throw new Error(`The service does not support ${method}.`);
        const { result } = await invokeVerbFlatBuffer(this.token, this.provider, method, buildStructuredArgs(args))
            .catch(failure => {
                throw changeServiceFailure(failure);
            });
        const response = record(result);
        if (response.schema !== 1) throw new Error("Unsupported change service schema.");
        return response;
    }
    async packages(): Promise<ChangePackage[]> {
        const result = await this.invoke("packages", []);
        return Object.entries(record(result.packages)).map(([name, value]) => decodePackage(name, value));
    }
    async configure(configuration: PackageConfiguration): Promise<ChangePackage> {
        const result = await this.invoke("configure", [
            configuration.name,
            configuration.objects.map(reference => new ObjectArgument(reference)),
            configuration.constants,
            configuration.upstream,
            configuration.generation,
            configuration.trustedOwners.map(reference => new ObjectArgument(reference)),
        ]);
        return decodePackage(configuration.name, result);
    }
    async upstream(name: string, generation: number, upstream: Upstream): Promise<ChangePackage> {
        return decodePackage(name, await this.invoke("upstream", [name, upstream, generation]));
    }
    async proposeObjects(
        name: string,
        generation: number,
        bindings: Record<string, StructuredArgument>,
        complete: boolean,
    ): Promise<ChangePackage> {
        const result = await this.invoke("propose_objects", [name, generation, bindings, complete ? 1 : 0]);
        const configured = decodePackage(name, result);
        if (configured.generation !== generation + 1) {
            throw new Error("Package changed. Reload before changing object bindings.");
        }
        return configured;
    }
    async stage(name: string, sources: { label: string; text: string }[], operation: "update" | "adopt") {
        if (sources.length && !this.capabilities.transports.includes("upload")) {
            throw new Error("The service does not support uploads.");
        }
        const bytes = sources.reduce((sum, source) => sum + new TextEncoder().encode(source.text).length, 0);
        if (bytes > this.capabilities.maxSourceBytes) throw new Error("Source exceeds the service limit.");
        const result = await this.invoke("stage", [name, sources, operation]);
        if (result.package !== name) throw new Error("Unexpected staged package.");
        const target: ChangeTarget = {
            kind: "change",
            provider: this.provider,
            review: integer(result.review_id),
            generation: integer(result.generation),
        };
        return target;
    }
    async discard(review: number, generation: number): Promise<void> {
        const result = await this.invoke("discard", [review, generation]);
        if (result.review_id !== review || result.status !== "discarded") throw new Error("Unexpected discard result.");
    }
    async history(name: string): Promise<CompletedReview[]> {
        const result = await this.invoke("history", [name]);
        if (result.package !== name || !Array.isArray(result.reviews)) throw new Error("Unexpected review history.");
        return result.reviews.map(value => {
            const item = record(value);
            return {
                review: integer(item.review_id),
                generation: integer(item.generation),
                status: text(item.status),
                completedAt: integer(item.completed_at),
                decisionCount: integer(item.decision_count),
            };
        }).sort((left, right) => right.review - left.review);
    }
}
