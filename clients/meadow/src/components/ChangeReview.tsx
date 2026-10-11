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

import Editor from "@monaco-editor/react";
import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import {
    baselineText,
    changeLabels,
    ChangeReviewClient,
    ChangeRow,
    ChangeTarget,
    Choice,
    Classification,
    CompilerDiagnostic,
    DiagnosticPage,
    InspectionPage,
    ReviewPage,
    ReviewStatus,
} from "../lib/change-review";
import { ChangeServiceClient } from "../lib/change-service";
import { ChangeGroupEditor } from "./ChangeGroupEditor";
import { ReviewDiffEditor } from "./ReviewDiffEditor";
import { ReviewProgramEditor } from "./ReviewProgramEditor";
import { ReviewSourceDetails } from "./ReviewSourceDetails";
import "../lib/monaco";
import { registerMooLanguage } from "../lib/monaco-moo";
import { DialogSheet } from "./DialogSheet";
import { useTheme } from "./ThemeProvider";
import { monacoThemeFor } from "./themeSupport";
import "../styles/change-review.css";

interface Props {
    visible: boolean;
    target: ChangeTarget;
    authToken: string;
    onClose: () => void;
    connectionRevision?: number;
}
const choices: Record<Choice, string> = {
    incoming: "Use upstream",
    local: "Keep local",
    edited: "Use edited program",
    defer: "Skip",
    unresolved: "Choose a version",
};
const blockers: Record<string, string> = {
    untrusted_target_authority: "Ownership or permissions prevent updating this verb.",
    object_identity_mismatch: "The installed object and upstream object have different identities.",
    adoption_required: "Accept an upstream baseline before updating this verb.",
    change_group_required: "Declare which changes must be installed together before selecting this program.",
    change_group_member_missing: "This group requires a member that is missing from the supplied source.",
    verb_name_collision:
        "A local or inherited verb already uses a conflicting name. Choose a distinct name and review again.",
    initialization_required: "Review the initial value and permissions, then permit initialization below.",
    initial_value_required: "Supply an explicit initial value for the new property, then stage the source again.",
    inherited_effects_require_approval:
        "This object has descendants. Permit their inherited property explicitly before applying.",
    property_name_collision:
        "A property with this name already exists on this object, an ancestor, or a descendant. Creation is blocked.",
    object_address_occupied:
        "This object ID is occupied. Discard the review, choose another explicit binding, and fetch again.",
    object_binding_changed:
        "The installed object no longer matches its creation receipt. Inspect the object before changing package rules.",
    creation_requires_wizard: "Creating an object requires wizard authority.",
    invalid_object_parent: "A parent is missing or inheritance contains a cycle. Correct the source and review again.",
    invalid_object_location:
        "A location is missing or containment contains a cycle. Correct the source and review again.",
    missing_inherited_property:
        "An override names a property that no parent defines. Correct the source and review again.",
};
const fieldLabels: Record<string, string> = {
    program: "Verb",
    property: "Property",
    attribute: "Attribute",
    object: "Object",
};
const rowLabels: Record<Classification, string> = {
    conflict: "Conflict",
    upstream: "Upstream",
    local: "Local",
    converged: "Match",
    unbased: "No baseline",
    unchanged: "Unchanged",
    local_only: "Local only",
    incoming_only: "Upstream only",
};
const categories: Classification[] = [
    "conflict",
    "upstream",
    "local",
    "local_only",
    "incoming_only",
    "unbased",
    "converged",
];
type ReviewFilter = Classification | "all";
const noCompilerDiagnostics: CompilerDiagnostic[] = [];

export function ChangeReview({ target, authToken, onClose, connectionRevision = 0 }: Props) {
    const id = useId();
    const { theme } = useTheme();
    const client = useMemo(() => new ChangeReviewClient(authToken, target), [authToken, target]);
    const [generation, setGeneration] = useState(target.generation);
    const [status, setStatus] = useState<ReviewStatus | null>(null);
    const [groupEditor, setGroupEditor] = useState<{ generation: number; group?: string } | null>(null);
    const [allowDescendants, setAllowDescendants] = useState(false);
    const [service, setService] = useState<ChangeServiceClient | null>(null);
    const [diagnostics, setDiagnostics] = useState<DiagnosticPage | null>(null);
    const [page, setPage] = useState<ReviewPage | null>(null);
    const [category, setCategory] = useState<ReviewFilter>("all");
    const [inspection, setInspection] = useState<InspectionPage | null>(null);
    const [rows, setRows] = useState<ChangeRow[]>([]);
    const [row, setRow] = useState<ChangeRow | null>(null);
    const [busy, setBusy] = useState(true);
    const [error, setError] = useState("");
    const [notice, setNotice] = useState("");
    const [editing, setEditing] = useState(false);
    const [draft, setDraft] = useState("");
    const [initialDraft, setInitialDraft] = useState("");
    const [comparison, setComparison] = useState("local-upstream");
    const [sideBySide, setSideBySide] = useState(true);
    const [collapsedObjects, setCollapsedObjects] = useState<Set<string>>(() => new Set());
    const [confirmation, setConfirmation] = useState<"apply" | "discard" | "refresh" | null>(null);
    const pendingNavigation = useRef<(() => void) | null>(null);
    const sequence = useRef(0);
    const dirty = editing && draft !== initialDraft;
    const connection = useRef(connectionRevision);
    const reviewState = useRef({ dirty, generation, row: row?.id, category });
    reviewState.current = { dirty, generation, row: row?.id, category };
    useEffect(() => {
        if (!dirty) return;
        const prevent = (event: BeforeUnloadEvent) => {
            event.preventDefault();
            event.returnValue = "";
        };
        window.addEventListener("beforeunload", prevent);
        return () => window.removeEventListener("beforeunload", prevent);
    }, [dirty]);

    const selectDetail = useCallback((detail: ChangeRow | null) => {
        setRow(detail);
        setAllowDescendants(false);
        setEditing(false);
        setComparison("local-upstream");
        const saved = detail?.choice?.program ?? detail?.live_text ?? "";
        setDraft(saved);
        setInitialDraft(saved);
    }, []);
    const fail = useCallback(
        (failure: unknown) => setError(failure instanceof Error ? failure.message : "Could not load the review."),
        [],
    );

    const load = useCallback(async (version: number, selected?: string, filter?: ReviewFilter) => {
        const request = ++sequence.current;
        setBusy(true);
        setError("");
        selectDetail(null);
        try {
            const available = await ChangeServiceClient.connect(authToken, target.provider);
            if (request !== sequence.current) return;
            setService(available);
            if (!available.supports("status")) throw new Error("This service does not expose review status.");
            const current = await client.status();
            if (request !== sequence.current) return;
            setStatus(current);
            if (!["ready", "partial", "rejected"].includes(current.status)) {
                setPage(null);
                setDiagnostics(null);
                return;
            }
            if (!available.supports("review") || !available.supports("details")) {
                throw new Error("This service does not expose review details.");
            }
            const detail = selected ? await client.details(version, selected) : null;
            const nextCategory = filter ?? "all";
            const [overview, inspected, checks] = await Promise.all([
                client.page(version, [], nextCategory === "all" ? "changed" : nextCategory),
                available.supports("inspection")
                    ? client.inspection(
                        version,
                        1,
                        nextCategory === "all" ? "" : nextCategory,
                    )
                    : Promise.resolve({ rows: [], counts: {}, revision: "", next: 0 }),
                available.supports("diagnostics") ? client.diagnostics(version) : Promise.resolve(null),
            ]);
            const shownRows = [...overview.rows, ...inspected.rows].sort((a, b) => a.label.localeCompare(b.label));
            const first = detail ?? (shownRows[0] ? await client.details(version, shownRows[0].id) : null);
            if (request !== sequence.current) return;
            setGeneration(version);
            setCategory(nextCategory);
            setPage(overview);
            setInspection(inspected);
            setDiagnostics(checks);
            setRows(overview.rows);
            selectDetail(first);
        } catch (failure) {
            if (request === sequence.current) fail(failure);
        } finally {
            if (request === sequence.current) setBusy(false);
        }
    }, [client, selectDetail, fail, authToken, target.provider]);
    const invalidate = useCallback(() => {
        sequence.current++;
    }, []);
    useEffect(() => {
        void load(target.generation, target.row);
        return invalidate;
    }, [load, target.generation, target.row, invalidate]);

    useEffect(() => {
        if (connection.current === connectionRevision) return;
        connection.current = connectionRevision;
        let cancelled = false;
        void client.status().then(current => {
            if (cancelled) return;
            setStatus(current);
            const local = reviewState.current;
            if (local.dirty) {
                if (current.generation !== local.generation) {
                    setError(
                        "Review changed while disconnected. Your unsaved text is retained. Copy it before reloading the review.",
                    );
                }
            } else void load(current.generation, local.row, local.category);
        }).catch(failure => {
            if (!cancelled) fail(failure);
        });
        return () => {
            cancelled = true;
        };
    }, [connectionRevision, client, load, fail]);

    useEffect(() => {
        if (!["applying", "fetching"].includes(status?.status ?? "")) return;
        let cancelled = false;
        let timer: ReturnType<typeof setTimeout>;
        const poll = async () => {
            try {
                const current = await client.status();
                if (cancelled) return;
                setStatus(current);
                if (["applying", "fetching"].includes(current.status)) timer = setTimeout(poll, 1000);
                else if (["partial", "ready", "rejected"].includes(current.status)) await load(current.generation);
            } catch (failure) {
                if (!cancelled) fail(failure);
            }
        };
        timer = setTimeout(poll, 500);
        return () => {
            cancelled = true;
            clearTimeout(timer);
        };
    }, [status?.status, client, load, fail]);

    const navigate = (action: () => void) => {
        if (dirty) {
            pendingNavigation.current = action;
            setConfirmation("discard");
        } else action();
    };
    const select = async (selected: ChangeRow) => {
        const request = ++sequence.current;
        setBusy(true);
        setError("");
        try {
            const detail = await client.details(generation, selected.id);
            if (request === sequence.current) selectDetail(detail);
        } catch (failure) {
            if (request === sequence.current) fail(failure);
        } finally {
            if (request === sequence.current) setBusy(false);
        }
    };
    const more = async () => {
        if (!page || (!page.cursor.length && !inspection?.next)) return;
        const request = ++sequence.current;
        setBusy(true);
        try {
            const [next, inspected] = await Promise.all([
                page.cursor.length
                    ? client.page(generation, page.cursor, category === "all" ? "changed" : category)
                    : null,
                inspection?.next
                    ? client.inspection(
                        generation,
                        inspection.next,
                        category === "all" ? "" : category,
                        inspection.revision,
                    )
                    : null,
            ]);
            if (request !== sequence.current) return;
            if (next) {
                setRows(previous => [...previous, ...next.rows]);
                setPage(next);
            }
            if (inspected) {
                setInspection(previous => ({ ...inspected, rows: [...(previous?.rows ?? []), ...inspected.rows] }));
            }
        } catch (failure) {
            if (request === sequence.current) fail(failure);
        } finally {
            if (request === sequence.current) setBusy(false);
        }
    };
    const resolve = async (selected: Choice) => {
        if (!row || row.read_only) return;
        const request = ++sequence.current;
        setBusy(true);
        setError("");
        try {
            const saved = await client.resolve(generation, row.id, selected, selected === "edited" ? draft : "");
            if (request !== sequence.current) return;
            setNotice(saved.errors.length ? saved.errors.join("\n") : "Choice saved. Live code has not changed.");
            await load(saved.generation, row.id, category);
        } catch (failure) {
            if (request === sequence.current) {
                fail(failure);
                setBusy(false);
            }
        }
    };
    const initializeProperty = async () => {
        if (!row || busy || dirty) return;
        const request = ++sequence.current;
        setBusy(true);
        setError("");
        try {
            const next = await client.initializeProperty(generation, row.id, allowDescendants);
            if (request !== sequence.current) return;
            await load(next, row.id, category);
            setNotice(
                "Initialization permitted. Saved choices were cleared. Select the required group before applying.",
            );
        } catch (failure) {
            if (request === sequence.current) {
                fail(failure);
                setBusy(false);
            }
        }
    };
    const resolveGroup = async (name: string, choice: "incoming" | "defer") => {
        if (busy || dirty) return;
        const request = ++sequence.current;
        setBusy(true);
        setError("");
        try {
            const next = await client.resolveGroup(generation, name, choice);
            if (request !== sequence.current) return;
            await load(next, row?.id, category);
            setNotice(
                choice === "incoming"
                    ? "Group selected. Review the programs, then apply when ready."
                    : "Group skipped. Live code has not changed.",
            );
        } catch (failure) {
            if (request === sequence.current) {
                fail(failure);
                setBusy(false);
            }
        }
    };
    const loadBase = async (files?: FileList | null) => {
        if (!row || dirty || busy || !service?.supports("base_source")) return;
        const request = ++sequence.current;
        setBusy(true);
        setError("");
        try {
            const units = Array.from(files ?? []);
            if (units.reduce((sum, file) => sum + file.size, 0) > service.capabilities.maxSourceBytes) {
                throw new Error("Historical source exceeds the service limit.");
            }
            const sources = await Promise.all(units.map(async file => ({ label: file.name, text: await file.text() })));
            const result = await client.baseSource(generation, row.id, sources);
            if (request !== sequence.current) return;
            await load(result.generation, row.id, category);
            setComparison("three-way");
            setNotice("Accepted source verified against this verb's baseline.");
        } catch (failure) {
            if (request === sequence.current) fail(failure);
        } finally {
            if (request === sequence.current) setBusy(false);
        }
    };
    const apply = async () => {
        setConfirmation(null);
        const request = ++sequence.current;
        setBusy(true);
        setError("");
        try {
            const result = await client.apply(generation);
            if (request !== sequence.current) return;
            setStatus(result);
            selectDetail(null);
            if (["partial", "ready", "rejected"].includes(result.status)) await load(result.generation);
            else setPage(null);
        } catch (failure) {
            if (request === sequence.current) fail(failure);
        } finally {
            if (request === sequence.current) setBusy(false);
        }
    };

    const refresh = async () => {
        setConfirmation(null);
        const request = ++sequence.current;
        setBusy(true);
        setError("");
        setNotice("");
        try {
            const next = await client.refresh(status?.generation ?? generation);
            if (request !== sequence.current) return;
            await load(next, undefined, category);
        } catch (failure) {
            if (request === sequence.current) fail(failure);
        } finally {
            if (request === sequence.current) setBusy(false);
        }
    };

    const displayedRows = [...rows, ...(inspection?.rows ?? [])].sort((a, b) => a.label.localeCompare(b.label));
    const groups = new Map<string, { label: string; object?: ChangeRow; members: ChangeRow[] }>();
    for (const item of displayedRows) {
        let group = groups.get(item.objectKey);
        if (!group) {
            group = { label: item.objectLabel, members: [] };
            groups.set(item.objectKey, group);
        }
        if (item.objectLabel.length > group.label.length) group.label = item.objectLabel;
        if (item.field === "object") group.object = item;
        else group.members.push(item);
    }
    const toggleObject = (key: string) =>
        setCollapsedObjects(previous => {
            const next = new Set(previous);
            if (next.has(key)) next.delete(key);
            else next.add(key);
            return next;
        });
    const itemButton = (item: ChangeRow, object = false) => {
        const blocked = !item.read_only && !item.eligible;
        const description = `${item.label}, ${fieldLabels[item.field ?? "program"]}, ${rowLabels[item.classification]}${
            blocked ? ", Blocked" : ""
        }`;
        return (
            <button
                type="button"
                className="change-review-item"
                title={description}
                aria-label={description}
                aria-current={row?.id === item.id ? "true" : undefined}
                disabled={busy}
                onClick={() =>
                    navigate(() => {
                        void select(item);
                    })}
            >
                <code>{object ? item.objectLabel : item.memberLabel}</code>
                {(category === "all" || blocked) && (
                    <small>{blocked ? "Blocked" : rowLabels[item.classification]}</small>
                )}
            </button>
        );
    };
    const counts: Partial<Record<Classification, number>> = { ...page?.counts };
    for (const [classification, count] of Object.entries(inspection?.counts ?? {})) {
        const key = classification as Classification;
        counts[key] = (counts[key] ?? 0) + (count ?? 0);
    }
    const total = Object.entries(counts).reduce(
        (sum, [key, count]) => key === "unchanged" ? sum : sum + (count ?? 0),
        0,
    );
    const baseline = row ? baselineText(row) : undefined;
    const original = comparison === "local-upstream" ? row?.live_text : baseline;
    const modified = comparison === "baseline-local" ? row?.live_text : row?.incoming_text;
    const selectedCount = page?.decision_counts.selected ?? 0;
    const applying = status?.status === "applying";
    const statusText = busy
        ? "Loading…"
        : applying
        ? "Applying updates…"
        : status?.status === "fetching"
        ? "Fetching upstream source…"
        : status?.status === "interrupted"
        ? "Apply was interrupted."
        : status?.status === "failed"
        ? "The upstream fetch failed."
        : status?.status === "complete"
        ? "Updates applied."
        : status?.status === "discarded"
        ? "This review was discarded."
        : "Compare local contents with the fetched source.";
    const canApply = Boolean(
        service?.supports("apply") && service.capabilities.fields.includes("program") && page && selectedCount
            && !page.decision_counts.unresolved && !busy && !dirty
            && !error && !applying,
    );

    return (
        <div className="change-review-shell" aria-busy={busy || applying}>
            <DialogSheet
                title={`${status?.package ?? "Changes"} · Review ${target.review}`}
                titleId={id}
                onCancel={() => navigate(onClose)}
                maxWidth="1440px"
            >
                <div className="change-review-toolbar">
                    <span role="status">{statusText}</span>
                    <small>Generation {status?.generation ?? generation}</small>
                    {service?.supports("refresh") && status
                        && ["ready", "partial", "rejected", "interrupted"].includes(status.status) && (
                        <button
                            type="button"
                            disabled={busy}
                            onClick={() => setConfirmation("refresh")}
                        >
                            Refresh comparison
                        </button>
                    )}
                    <button type="button" onClick={() => navigate(onClose)}>Close</button>
                </div>
                {error && (
                    <div role="alert" className="change-review-error">
                        {error}
                        <button
                            type="button"
                            disabled={busy}
                            onClick={() =>
                                navigate(() => {
                                    void client.status().then(current => load(current.generation, undefined, category))
                                        .catch(fail);
                                })}
                        >
                            Reload review
                        </button>
                    </div>
                )}
                {notice && <p role="status" className="change-review-notice">{notice}</p>}
                {status?.error?.message && <p role="alert">{status.error.message}</p>}
                <ReviewSourceDetails
                    status={status}
                    diagnostics={diagnostics}
                    busy={busy}
                    onMore={() => {
                        if (!diagnostics?.next) return;
                        const request = sequence.current;
                        setBusy(true);
                        void client.diagnostics(generation, diagnostics.next).then(next => {
                            if (request === sequence.current) {
                                setDiagnostics({ ...next, rows: [...diagnostics.rows, ...next.rows] });
                            }
                        }).catch(failure => {
                            if (request === sequence.current) fail(failure);
                        }).finally(() => {
                            if (request === sequence.current) setBusy(false);
                        });
                    }}
                />
                {page && !applying && service?.supports("set_group") && page.operation !== "adopt" && (
                    <details
                        className="change-review-groups"
                        open={rows.some(row => row.operation?.startsWith("add_"))
                            || Boolean(page.groupValidation.length)}
                    >
                        <summary>Changes that belong together ({page.groups.length} groups)</summary>
                        <p>
                            A group keeps a program and its required additions together. Select every required group,
                            then review the choices before applying.
                        </p>
                        <p>
                            New objects and members appear only when you apply. A conflicting address or name blocks
                            creation. Existing property values stay unchanged.
                        </p>
                        {page.groups.map(group => (
                            <div className="change-review-group" key={group.name}>
                                <strong>{group.name}</strong>
                                <span>{group.count} required changes</span>
                                {group.requires.length > 0 && <span>Also requires: {group.requires.join(", ")}</span>}
                                {!group.complete && <span>Some required changes are missing or blocked.</span>}
                                <button
                                    type="button"
                                    disabled={busy || dirty}
                                    onClick={() => setGroupEditor({ generation, group: group.name })}
                                >
                                    Edit {group.name}
                                </button>
                                {service.supports("resolve_group") && (
                                    <>
                                        <button
                                            type="button"
                                            disabled={busy || dirty || !group.complete || Boolean(error)}
                                            onClick={() => void resolveGroup(group.name, "incoming")}
                                        >
                                            Select {group.name}
                                        </button>
                                        <button
                                            type="button"
                                            disabled={busy || dirty || Boolean(error)}
                                            onClick={() => void resolveGroup(group.name, "defer")}
                                        >
                                            Skip {group.name}
                                        </button>
                                    </>
                                )}
                            </div>
                        ))}
                        {page.groupValidation.map(message => <p key={message} role="status">{message}</p>)}
                        <button
                            type="button"
                            disabled={busy || dirty || Boolean(error)}
                            onClick={() => setGroupEditor({ generation })}
                        >
                            Create a change group
                        </button>
                    </details>
                )}
                {page && !applying && (
                    <div className="change-review-workspace" aria-busy={busy}>
                        <aside className="change-review-sidebar" aria-label="Changes">
                            <label htmlFor={`${id}-filter`}>Show</label>
                            <select
                                id={`${id}-filter`}
                                value={category}
                                disabled={busy}
                                onChange={event =>
                                    navigate(() => {
                                        void load(generation, undefined, event.target.value as ReviewFilter);
                                    })}
                            >
                                <option value="all">All differences ({total})</option>
                                {(category === "unchanged" ? [...categories, "unchanged" as const] : categories).map(
                                    key => (
                                        <option key={key} value={key}>
                                            {changeLabels[key]} ({counts[key] ?? 0})
                                        </option>
                                    ),
                                )}
                            </select>
                            <div className="change-review-programs">
                                {[...groups.entries()].map(([key, group]) => (
                                    <section className="change-review-object" key={key} aria-label={group.label}>
                                        <div className="change-review-object-heading">
                                            {group.members.length > 0 && (
                                                <button
                                                    type="button"
                                                    className="change-review-disclosure"
                                                    aria-label={`${
                                                        collapsedObjects.has(key) ? "Expand" : "Collapse"
                                                    } ${group.label}`}
                                                    aria-expanded={!collapsedObjects.has(key)}
                                                    onClick={() => toggleObject(key)}
                                                >
                                                    <span aria-hidden="true">
                                                        {collapsedObjects.has(key) ? "▸" : "▾"}
                                                    </span>
                                                </button>
                                            )}
                                            {group.object
                                                ? itemButton(group.object, true)
                                                : (
                                                    <button
                                                        type="button"
                                                        className="change-review-item"
                                                        aria-label={`${
                                                            collapsedObjects.has(key) ? "Expand" : "Collapse"
                                                        } members of ${group.label}`}
                                                        aria-expanded={!collapsedObjects.has(key)}
                                                        onClick={() => toggleObject(key)}
                                                        title={group.label}
                                                    >
                                                        <code>{group.label}</code>
                                                    </button>
                                                )}
                                        </div>
                                        {!collapsedObjects.has(key) && group.members.length > 0
                                            && (
                                                <ul>
                                                    {group.members.map(item => (
                                                        <li key={item.id}>{itemButton(item)}</li>
                                                    ))}
                                                </ul>
                                            )}
                                    </section>
                                ))}
                                {!displayedRows.length && (
                                    <p>
                                        No {category === "all" ? "differences" : changeLabels[category].toLowerCase()}.
                                    </p>
                                )}
                                {(!!page.cursor.length || !!inspection?.next) && (
                                    <button
                                        type="button"
                                        disabled={busy}
                                        onClick={() => {
                                            void more();
                                        }}
                                    >
                                        Load more ({displayedRows.length} shown)
                                    </button>
                                )}
                            </div>
                            <small>
                                {page.counts.unchanged ?? 0} unchanged members.
                            </small>
                        </aside>
                        <main className="change-review-code">
                            {row
                                ? (
                                    <>
                                        <header>
                                            <h3>
                                                <code>{row.label}</code>
                                            </h3>
                                            <span>{changeLabels[row.classification]}</span>
                                        </header>
                                        {row.operation === "add_object" && (
                                            <section aria-label="New object definition">
                                                <h4>Create at the chosen object ID</h4>
                                                <p>
                                                    Review the full definition below, including owners, flags, initial
                                                    properties, overrides, and programs.
                                                </p>
                                                <p>
                                                    Apply creates this object only if its ID is still absent. It
                                                    installs all these contents together. It does not run initialization
                                                    verbs or choose another ID after a conflict.
                                                </p>
                                                <p>
                                                    Include the object and every program that needs it in a change
                                                    group. Required new parents, owners, and locations must apply too.
                                                </p>
                                            </section>
                                        )}
                                        {row.operation === "retain_object" && (
                                            <p>
                                                This object is already installed. Selecting this row keeps its structure
                                                and creation receipt. Separate program rows control code updates.
                                                Property data stays unchanged.
                                            </p>
                                        )}
                                        {row.operation === "add_property" && (
                                            <section aria-label="Property initialization">
                                                <h4>Initial value, only if absent</h4>
                                                <p>
                                                    Review the value, owner, flags, and metadata below. Apply uses them
                                                    only to create this property. Existing data stays unchanged.
                                                </p>
                                                <p>
                                                    Flags: r permits reading and w permits writing. With c, each
                                                    descendant's owner also owns the inherited property. The o flag
                                                    permits concurrent value writes. The last committed write wins.
                                                </p>
                                                {(row.descendants ?? 0) > 0 && (
                                                    <label>
                                                        <input
                                                            type="checkbox"
                                                            checked={allowDescendants}
                                                            disabled={busy || dirty}
                                                            onChange={event =>
                                                                setAllowDescendants(event.target.checked)}
                                                        />
                                                        Permit the {row.descendants}{" "}
                                                        existing descendants to inherit this new property
                                                    </label>
                                                )}
                                                <p>
                                                    This permission becomes a package rule. Saving it clears the
                                                    review's choices. It does not create the property until you apply
                                                    its group.
                                                </p>
                                                {service?.supports("initialize_property") && (
                                                    <button
                                                        type="button"
                                                        disabled={busy || dirty
                                                            || ((row.descendants ?? 0) > 0 && !allowDescendants)}
                                                        onClick={() => void initializeProperty()}
                                                    >
                                                        Permit initialization and clear choices
                                                    </button>
                                                )}
                                                {row.initialization_approved && (
                                                    <p>Absent-only initialization is already permitted.</p>
                                                )}
                                            </section>
                                        )}
                                        {row.operation === "retain_property" && (
                                            <p>
                                                This property already exists. Selecting its group keeps the live value,
                                                permissions, metadata, and accepted baseline unchanged. The upstream
                                                value is for comparison only.
                                            </p>
                                        )}
                                        {row.definition_text && (
                                            <section aria-label="New verb definition">
                                                <h4>Definition to install</h4>
                                                <p>
                                                    Apply creates this verb with the owner, permissions, and metadata
                                                    shown here. The program appears below.
                                                </p>
                                                <pre>{row.definition_text}</pre>
                                                <p>
                                                    Flags: r permits reading, x permits calls from programs, and d
                                                    enables error reporting. Arguments control command matching.
                                                </p>
                                            </section>
                                        )}
                                        {!row.read_only && !row.eligible && (
                                            <p role="status">
                                                {row.blockers.map(reason => blockers[reason] ?? reason).join(" ")}
                                            </p>
                                        )}
                                        {row.live_present !== false && row.incoming_present !== false && (
                                            <div className="change-review-comparison">
                                                <label htmlFor={`${id}-comparison`}>Compare</label>
                                                <select
                                                    id={`${id}-comparison`}
                                                    value={comparison}
                                                    onChange={event => setComparison(event.target.value)}
                                                >
                                                    <option value="local-upstream">Local → Upstream</option>
                                                    {baseline !== undefined && (
                                                        <>
                                                            <option value="three-way">Three-way comparison</option>
                                                            <option value="baseline-local">
                                                                Accepted baseline → Local
                                                            </option>
                                                            <option value="baseline-upstream">
                                                                Accepted baseline → Upstream
                                                            </option>
                                                        </>
                                                    )}
                                                </select>
                                                <div
                                                    className="change-review-layout"
                                                    role="group"
                                                    aria-label="Diff layout"
                                                >
                                                    <button
                                                        type="button"
                                                        aria-pressed={sideBySide}
                                                        onClick={() => setSideBySide(true)}
                                                    >
                                                        Side by side
                                                    </button>
                                                    <button
                                                        type="button"
                                                        aria-pressed={!sideBySide}
                                                        onClick={() => setSideBySide(false)}
                                                    >
                                                        Unified
                                                    </button>
                                                </div>
                                                {baseline === undefined && (
                                                    <small>
                                                        {row.base
                                                            ? "Load the accepted source to compare all three versions."
                                                            : "This item has no accepted baseline."}
                                                    </small>
                                                )}
                                            </div>
                                        )}
                                        {!row.read_only && row.field === "program" && row.base
                                            && service?.supports("base_source") && (
                                            <div className="change-review-base-source">
                                                {row.base_fetch_available && (
                                                    <button
                                                        type="button"
                                                        disabled={busy || dirty}
                                                        onClick={() => void loadBase()}
                                                    >
                                                        Fetch accepted source
                                                    </button>
                                                )}
                                                <label>
                                                    Upload accepted objdefs
                                                    <input
                                                        type="file"
                                                        multiple
                                                        accept=".moo"
                                                        disabled={busy || dirty}
                                                        onChange={event => {
                                                            if (event.currentTarget.files?.length) {
                                                                void loadBase(event.currentTarget.files);
                                                            }
                                                            event.currentTarget.value = "";
                                                        }}
                                                    />
                                                </label>
                                                {dirty && (
                                                    <small>
                                                        Save your edited program before loading accepted source.
                                                    </small>
                                                )}
                                            </div>
                                        )}
                                        {row.read_only && (
                                            <p className="change-review-inspection-note">
                                                {row.live_present === false
                                                    ? "No local definition."
                                                    : row.incoming_present === false
                                                    ? "No upstream definition."
                                                    : ""} Read only.
                                            </p>
                                        )}
                                        {row.inspection_error && <p role="status">{row.inspection_error}</p>}
                                        <div
                                            className="change-review-pane-labels"
                                            hidden={comparison === "three-way"}
                                            data-layout={sideBySide ? "split" : "unified"}
                                        >
                                            <span>
                                                {!sideBySide && row.live_present !== false
                                                    && row.incoming_present !== false && "− "}
                                                {row.live_present === false
                                                    ? "Upstream"
                                                    : comparison === "local-upstream"
                                                    ? "Local"
                                                    : "Accepted baseline"}
                                            </span>
                                            {row.live_present !== false && row.incoming_present !== false && (
                                                <span>
                                                    {!sideBySide && "+ "}
                                                    {comparison === "baseline-local" ? "Local" : "Upstream"}
                                                </span>
                                            )}
                                        </div>
                                        <div className="change-review-diff">
                                            {comparison === "three-way" && baseline !== undefined
                                                ? (
                                                    <div className="change-review-three-way">
                                                        {[{ label: "Accepted baseline", text: baseline }, {
                                                            label: "Local",
                                                            text: row.live_text ?? "",
                                                        }, { label: "Upstream", text: row.incoming_text ?? "" }].map((
                                                            pane,
                                                            index,
                                                        ) => (
                                                            <section key={pane.label} aria-label={pane.label}>
                                                                <h4>{pane.label}</h4>
                                                                {index === 0
                                                                    ? (
                                                                        <Editor
                                                                            defaultValue={pane.text}
                                                                            language="moo"
                                                                            theme={monacoThemeFor(theme)}
                                                                            beforeMount={registerMooLanguage}
                                                                            options={{
                                                                                readOnly: true,
                                                                                automaticLayout: true,
                                                                                minimap: { enabled: false },
                                                                                wordWrap: "on",
                                                                                scrollBeyondLastLine: false,
                                                                            }}
                                                                        />
                                                                    )
                                                                    : (
                                                                        <ReviewDiffEditor
                                                                            original={baseline}
                                                                            modified={pane.text}
                                                                            language="moo"
                                                                            theme={monacoThemeFor(theme)}
                                                                            beforeMount={registerMooLanguage}
                                                                            options={{
                                                                                readOnly: true,
                                                                                originalEditable: false,
                                                                                automaticLayout: true,
                                                                                minimap: { enabled: false },
                                                                                renderSideBySide: false,
                                                                                useInlineViewWhenSpaceIsLimited: false,
                                                                                diffWordWrap: "on",
                                                                                scrollBeyondLastLine: false,
                                                                            }}
                                                                        />
                                                                    )}
                                                            </section>
                                                        ))}
                                                    </div>
                                                )
                                                : row.live_present === false || row.incoming_present === false
                                                ? (
                                                    <Editor
                                                        key={row.id}
                                                        value={row.live_present === false
                                                            ? row.incoming_text
                                                            : row.live_text}
                                                        language="moo"
                                                        theme={monacoThemeFor(theme)}
                                                        beforeMount={registerMooLanguage}
                                                        options={{
                                                            readOnly: true,
                                                            automaticLayout: true,
                                                            minimap: { enabled: false },
                                                            wordWrap: "on",
                                                            scrollBeyondLastLine: false,
                                                        }}
                                                    />
                                                )
                                                : (
                                                    <ReviewDiffEditor
                                                        key={row.id}
                                                        original={original ?? ""}
                                                        modified={modified ?? ""}
                                                        language="moo"
                                                        theme={monacoThemeFor(theme)}
                                                        beforeMount={registerMooLanguage}
                                                        options={{
                                                            readOnly: true,
                                                            originalEditable: false,
                                                            automaticLayout: true,
                                                            minimap: { enabled: false },
                                                            renderSideBySide: sideBySide,
                                                            useInlineViewWhenSpaceIsLimited: false,
                                                            diffWordWrap: "on",
                                                            scrollBeyondLastLine: false,
                                                        }}
                                                    />
                                                )}
                                        </div>
                                        {editing && (
                                            <section className="change-review-draft">
                                                <h4>Proposed program</h4>
                                                <ReviewProgramEditor
                                                    language="moo"
                                                    theme={monacoThemeFor(theme)}
                                                    initialValue={initialDraft}
                                                    diagnostics={draft === initialDraft
                                                        ? row.validation ?? noCompilerDiagnostics
                                                        : noCompilerDiagnostics}
                                                    onChange={value => setDraft(value ?? "")}
                                                    beforeMount={registerMooLanguage}
                                                    options={{
                                                        readOnly: busy,
                                                        automaticLayout: true,
                                                        minimap: { enabled: false },
                                                        wordWrap: "on",
                                                        scrollBeyondLastLine: false,
                                                    }}
                                                />
                                            </section>
                                        )}
                                        {!row.read_only && service?.supports("resolve")
                                            && service.capabilities.fields.includes("program") && (
                                            <div className="change-review-resolution">
                                                <span>On apply: {choices[row.choice?.choice ?? row.default]}</span>
                                                {row.choices.filter(value =>
                                                    value !== "edited" && service.capabilities.choices.includes(value)
                                                ).map(value => (
                                                    <button
                                                        type="button"
                                                        key={value}
                                                        disabled={busy || !!error || !row.eligible && value !== "defer"}
                                                        onClick={() =>
                                                            navigate(() => {
                                                                void resolve(value);
                                                            })}
                                                    >
                                                        {value === "incoming" && page.operation === "adopt"
                                                            ? "Accept baseline"
                                                            : choices[value]}
                                                    </button>
                                                ))}
                                                {row.eligible && row.choices.includes("edited")
                                                    && service.capabilities.choices.includes("edited") && (
                                                    <button
                                                        type="button"
                                                        disabled={busy || !!error}
                                                        onClick={() => {
                                                            if (editing) void resolve("edited");
                                                            else setEditing(true);
                                                        }}
                                                    >
                                                        {editing ? "Save proposed program" : "Edit resolution"}
                                                    </button>
                                                )}
                                            </div>
                                        )}
                                    </>
                                )
                                : (
                                    <p>
                                        {busy
                                            ? "Loading source…"
                                            : total > 0
                                            ? "Select an item from the list."
                                            : "No differences to review."}
                                    </p>
                                )}
                        </main>
                    </div>
                )}
                {confirmation && (
                    <div className="change-review-confirmation" role="alert">
                        <p>
                            {confirmation === "discard"
                                ? "Discard the unsaved draft?"
                                : confirmation === "refresh"
                                ? "Compare the fetched source with the running MOO again? This clears all saved choices and any unsaved draft."
                                : page?.operation === "adopt"
                                ? `Accept ${selectedCount} baselines? Live code will stay as it is.`
                                : `Apply ${selectedCount} choices to the running MOO?`}
                        </p>
                        {confirmation === "apply" && page?.operation !== "adopt" && (
                            <p>
                                Selected programs may depend on omitted property, definition, or object changes. This
                                review does not establish that compatibility.
                            </p>
                        )}
                        <button type="button" onClick={() => setConfirmation(null)}>Cancel</button>
                        <button
                            type="button"
                            onClick={() => {
                                if (confirmation === "apply") void apply();
                                else if (confirmation === "refresh") void refresh();
                                else {
                                    setConfirmation(null);
                                    pendingNavigation.current?.();
                                    pendingNavigation.current = null;
                                }
                            }}
                        >
                            {confirmation === "apply"
                                ? "Confirm apply"
                                : confirmation === "refresh"
                                ? "Confirm refresh"
                                : "Discard draft"}
                        </button>
                    </div>
                )}
                {page && (
                    <footer className="change-review-footer">
                        <small>
                            {page.decision_counts.unresolved
                                ? `${page.decision_counts.unresolved} choices or group requirements remain unresolved. `
                                : ""}
                            {page.decision_counts.blocked
                                ? `${page.decision_counts.blocked} blocked members will be kept. `
                                : ""}
                        </small>
                        <button
                            type="button"
                            className="btn-primary"
                            disabled={!canApply}
                            onClick={() => setConfirmation("apply")}
                        >
                            {page.operation === "adopt"
                                ? `Accept ${selectedCount} baselines`
                                : `Apply ${selectedCount} ${selectedCount === 1 ? "choice" : "choices"}`}
                        </button>
                    </footer>
                )}
            </DialogSheet>
            {groupEditor && page && (
                <ChangeGroupEditor
                    client={client}
                    generation={groupEditor.generation}
                    groups={page.groups}
                    group={groupEditor.group}
                    onClose={() => setGroupEditor(null)}
                    onSaved={next => {
                        setGroupEditor(null);
                        void load(next, row?.id, category);
                    }}
                />
            )}
        </div>
    );
}
