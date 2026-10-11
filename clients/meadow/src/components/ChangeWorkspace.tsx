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

import { jsObjectRefToCurie, ObjectArgument } from "@moor/web-sdk";
import { useCallback, useEffect, useRef, useState } from "react";
import { ChangeReviewClient, type ChangeTarget, type ReviewStatus } from "../lib/change-review";
import {
    type ChangePackage,
    type ChangeServiceClient,
    type CompletedReview,
    type Upstream,
} from "../lib/change-service";
import { ChangeReview } from "./ChangeReview";
import { DialogSheet } from "./DialogSheet";
import "../styles/change-workspace.css";

interface Props {
    visible: boolean;
    service: ChangeServiceClient;
    authToken: string;
    isWizard: boolean;
    onClose: () => void;
    connectionRevision?: number;
}
function objectReferences(input: string): string[] {
    return input.split(/[\s,]+/).filter(Boolean).map(value => {
        const reference = jsObjectRefToCurie(value);
        if (!reference) throw new Error(`Invalid object reference: ${value}`);
        return reference;
    });
}
function bindingArguments(input: string): Record<string, ObjectArgument> {
    const names = new Set<string>();
    return Object.fromEntries(
        input.split("\n").filter(line => line.trim()).map(line => {
            const parts = line.trim().split(/\s*=\s*/);
            if (parts.length !== 2 || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(parts[0])) {
                throw new Error("Use one NAME = object reference per line for constant bindings.");
            }
            const refs = objectReferences(parts[1]);
            if (refs.length !== 1) throw new Error("Each constant binding needs one object reference.");
            if (names.has(parts[0].toLowerCase())) throw new Error(`Duplicate constant binding: ${parts[0]}`);
            names.add(parts[0].toLowerCase());
            return [parts[0], new ObjectArgument(refs[0])];
        }),
    );
}

export function ChangeWorkspace({ service, authToken, isWizard, onClose, connectionRevision = 0 }: Props) {
    const [section, setSection] = useState<"updates" | "settings" | "history" | "upload" | "repair">("updates");
    const [packages, setPackages] = useState<ChangePackage[]>([]);
    const [selected, setSelected] = useState(service.capabilities.defaultPackage);
    const [history, setHistory] = useState<CompletedReview[]>([]);
    const [active, setActive] = useState<ReviewStatus | null>(null);
    const [review, setReview] = useState<ChangeTarget | null>(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState("");
    const [editor, setEditor] = useState<"new" | "upstream" | "bindings" | null>(null);
    const [expectedGeneration, setExpectedGeneration] = useState(0);
    const [name, setName] = useState("");
    const [objects, setObjects] = useState("");
    const [bindings, setBindings] = useState("");
    const [complete, setComplete] = useState(false);
    const [owners, setOwners] = useState("");
    const [transport, setTransport] = useState("upload");
    const [url, setUrl] = useState("");
    const [revisionKind, setRevisionKind] = useState("ref");
    const [revision, setRevision] = useState("refs/heads/main");
    const [path, setPath] = useState("");
    const [dirty, setDirty] = useState(false);
    const [files, setFiles] = useState<File[]>([]);
    const [adopting, setAdopting] = useState(false);
    const [confirmDiscard, setConfirmDiscard] = useState(false);
    const sequence = useRef(0);
    const item = packages.find(item => item.name === selected);
    const capabilities = service.capabilities;
    const target = useCallback((id: number, generation: number): ChangeTarget => ({
        kind: "change",
        provider: service.provider,
        review: id,
        generation,
    }), [service.provider]);
    const fail = (failure: unknown) =>
        setError(failure instanceof Error ? failure.message : "Could not complete the request.");
    const load = useCallback(async () => {
        const request = ++sequence.current;
        try {
            const next = await service.packages();
            const packageItem = next.find(item => item.name === selected);
            const [completed, status] = await Promise.all([
                packageItem && service.supports("history") ? service.history(selected) : Promise.resolve([]),
                packageItem?.active && service.supports("status")
                    ? new ChangeReviewClient(authToken, target(packageItem.active, 1)).status()
                    : Promise.resolve(null),
            ]);
            if (request !== sequence.current) return;
            setPackages(next);
            setHistory(completed);
            setActive(status);
            setError("");
        } catch (failure) {
            if (request === sequence.current) fail(failure);
        }
    }, [service, selected, authToken, target]);
    const invalidate = useCallback(() => {
        sequence.current++;
    }, []);
    useEffect(() => {
        if (review) return;
        void load();
        return invalidate;
    }, [load, review, invalidate, connectionRevision]);
    useEffect(() => {
        if (review || !["fetching", "applying"].includes(active?.status ?? "")) return;
        const timer = setTimeout(() => void load(), 1000);
        return () => clearTimeout(timer);
    }, [active, review, load]);
    useEffect(() => {
        if (!dirty && files.length === 0) return;
        const prevent = (event: BeforeUnloadEvent) => {
            event.preventDefault();
            event.returnValue = "";
        };
        window.addEventListener("beforeunload", prevent);
        return () => window.removeEventListener("beforeunload", prevent);
    }, [dirty, files.length]);
    const navigate = (action: () => void) => {
        if ((dirty || files.length > 0) && !window.confirm("Discard unsaved package settings and selected files?")) {
            return;
        }
        setDirty(false);
        setFiles([]);
        action();
    };
    const run = async (action: () => Promise<void>) => {
        setBusy(true);
        setError("");
        try {
            await action();
        } catch (failure) {
            fail(failure);
        } finally {
            setBusy(false);
        }
    };
    const edit = (kind: "new" | "upstream" | "bindings") => {
        setEditor(kind);
        setExpectedGeneration(kind === "new" ? 0 : item?.generation ?? 0);
        setDirty(false);
        const upstream = kind === "new" ? "" : item?.upstream ?? "";
        setTransport(typeof upstream === "string" ? upstream ? "http" : "upload" : "git");
        setUrl(typeof upstream === "string" ? upstream : upstream.repository);
        setRevisionKind(typeof upstream !== "string" && "commit" in upstream.revision ? "commit" : "ref");
        setRevision(
            typeof upstream === "string"
                ? "refs/heads/main"
                : "ref" in upstream.revision
                ? upstream.revision.ref
                : upstream.revision.commit,
        );
        setPath(typeof upstream === "string" ? "" : upstream.path);
        setName("");
        setObjects("");
        setBindings(
            kind === "bindings"
                ? Object.entries(item?.proposedBindings ?? {}).map(([name, reference]) => `${name} = ${reference}`)
                    .join("\n")
                : "",
        );
        setComplete(Boolean(item?.objects.length && item.complete.length === item.objects.length));
        setOwners("");
    };
    const upstream = (): Upstream =>
        transport === "upload" ? "" : transport === "http" ? url : {
            transport: "git",
            repository: url,
            path,
            revision: revisionKind === "ref" ? { ref: revision } : { commit: revision },
        };
    const save = () =>
        run(async () => {
            if (editor === "new") {
                await service.configure({
                    name,
                    generation: 0,
                    objects: objectReferences(objects),
                    constants: bindingArguments(bindings),
                    trustedOwners: objectReferences(owners),
                    upstream: upstream(),
                });
                setSelected(name);
            } else if (item && editor === "bindings") {
                await service.proposeObjects(item.name, expectedGeneration, bindingArguments(bindings), complete);
            } else if (item) {
                await service.upstream(item.name, expectedGeneration, upstream());
            }
            setEditor(null);
            setDirty(false);
            await load();
        });
    const stage = () =>
        run(async () => {
            const size = files.reduce((sum, file) => sum + file.size, 0);
            if (size > capabilities.maxSourceBytes) throw new Error("Source exceeds the service limit.");
            const sources = await Promise.all(files.map(async file => ({ label: file.name, text: await file.text() })));
            const started = await service.stage(selected, sources, adopting ? "adopt" : "update");
            setFiles([]);
            setReview(started);
        });
    if (review) {
        return (
            <ChangeReview
                visible
                target={review}
                authToken={authToken}
                connectionRevision={connectionRevision}
                onClose={() => setReview(null)}
            />
        );
    }
    const gitRestricted = !files.length && typeof item?.upstream !== "string"
        && capabilities.gitAuthorization === "wizard" && !isWizard;
    const fetchTransport = typeof item?.upstream === "string" ? "http" : "git";
    const canStage = service.supports("stage") && !item?.active && !gitRestricted
        && (files.length > 0 || Boolean(item?.upstream) && capabilities.transports.includes(fetchTransport));
    return (
        <>
            <DialogSheet
                title="Changes"
                titleId="change-workspace-title"
                maxWidth="900px"
                onCancel={() => {
                    if (!busy) navigate(onClose);
                }}
            >
                <div className="change-workspace">
                    <p>Find upstream updates, preserve local work, and review what will change before applying.</p>
                    <nav className="change-workspace-actions" aria-label="Update tasks">
                        {([["updates", "Updates"], ["history", "History"], ["settings", "Settings"], [
                            "upload",
                            "Review files",
                        ], ["repair", "Repair history"]] as const).map(([key, label]) => (
                            <button
                                key={key}
                                type="button"
                                aria-pressed={section === key}
                                disabled={busy}
                                onClick={() =>
                                    navigate(() => {
                                        setSection(key);
                                        setEditor(null);
                                        setAdopting(key === "repair");
                                    })}
                            >
                                {label}
                            </button>
                        ))}
                        <button type="button" disabled={busy} onClick={() => navigate(onClose)}>Close</button>
                    </nav>
                    {error && <p role="alert">{error}</p>}
                    <div className="change-workspace-actions">
                        {(packages.length > 1 || section === "settings") && (
                            <label>
                                Package{" "}
                                <select
                                    aria-label="Package"
                                    disabled={busy || editor !== null}
                                    value={selected}
                                    onChange={event => navigate(() => setSelected(event.target.value))}
                                >
                                    {!packages.some(item => item.name === selected) && (
                                        <option value={selected}>{selected || "Select a package"}</option>
                                    )}
                                    {packages.map(item => <option key={item.name}>{item.name}</option>)}
                                </select>
                            </label>
                        )}
                        {section === "settings" && (
                            <button type="button" disabled={busy} onClick={() => void load()}>Reload packages</button>
                        )}
                        {section === "settings" && service.supports("configure") && (
                            <button
                                type="button"
                                disabled={busy || editor !== null || packages.length >= capabilities.maxPackages}
                                onClick={() => navigate(() => edit("new"))}
                            >
                                New package
                            </button>
                        )}
                    </div>
                    {editor
                        ? (
                            <form
                                onSubmit={event => {
                                    event.preventDefault();
                                    void save();
                                }}
                                onChange={() => setDirty(true)}
                            >
                                <fieldset disabled={busy}>
                                    <legend>
                                        {editor === "new"
                                            ? "New package"
                                            : editor === "bindings"
                                            ? "Object bindings and source scope"
                                            : "Upstream configuration"}
                                    </legend>
                                    {editor === "bindings" && (
                                        <>
                                            <p>
                                                Choose an explicit numeric ID for each new object. Preview reserves
                                                nothing. Apply stops if an ID is occupied. It never chooses another
                                                address for you.
                                            </p>
                                            <label>
                                                Proposed object bindings<textarea
                                                    placeholder="HELPER = #9600"
                                                    value={bindings}
                                                    onChange={event => setBindings(event.target.value)}
                                                />
                                            </label>
                                            <p>
                                                Use one NAME = #ID per line. The name supplies an existing objdef
                                                constant. Each new object uses its reviewed initial contents only when
                                                the address is absent. No initialization verb runs automatically.
                                            </p>
                                            <label>
                                                <input
                                                    type="checkbox"
                                                    checked={complete}
                                                    onChange={event => setComplete(event.target.checked)}
                                                />
                                                Source lists every member of each managed object
                                            </label>
                                            <p>
                                                Leave this unchecked for partial source. Omitted members then carry no
                                                removal request. Complete source reports omissions as unsupported
                                                removals. Neither mode deletes anything.
                                            </p>
                                            <p>
                                                Save these rules before fetching source. Then review the new objects and
                                                their dependent programs in change groups.
                                            </p>
                                        </>
                                    )}
                                    {editor === "new" && (
                                        <>
                                            <label>
                                                Package name<input
                                                    required
                                                    maxLength={64}
                                                    value={name}
                                                    onChange={event => setName(event.target.value)}
                                                />
                                            </label>
                                            <label>
                                                Managed objects<textarea
                                                    placeholder="#9500 #9501"
                                                    value={objects}
                                                    onChange={event => setObjects(event.target.value)}
                                                />
                                            </label>
                                            <label>
                                                Object constant bindings<textarea
                                                    placeholder="ROOT = #1"
                                                    value={bindings}
                                                    onChange={event => setBindings(event.target.value)}
                                                />
                                            </label>
                                            <p>
                                                Use one NAME = object reference per line. Source files can define other
                                                constants.
                                            </p>
                                            <label>
                                                Trusted owners<textarea
                                                    placeholder="#2"
                                                    value={owners}
                                                    onChange={event => setOwners(event.target.value)}
                                                />
                                            </label>
                                            <p>Only add owners whose code this package is allowed to update.</p>
                                        </>
                                    )}
                                    {editor !== "bindings" && (
                                        <>
                                            <label>
                                                Source<select
                                                    value={transport}
                                                    onChange={event => setTransport(event.target.value)}
                                                >
                                                    {capabilities.transports.includes("upload") && (
                                                        <option value="upload">Upload only</option>
                                                    )}
                                                    {capabilities.transports.includes("http") && (
                                                        <option value="http">HTTP bundle</option>
                                                    )}
                                                    {capabilities.transports.includes("git") && (
                                                        <option value="git">Git repository</option>
                                                    )}
                                                </select>
                                            </label>
                                            {transport !== "upload" && (
                                                <label>
                                                    {transport === "git" ? "Repository URL" : "Bundle URL"}
                                                    <input
                                                        required
                                                        type="url"
                                                        value={url}
                                                        onChange={event => setUrl(event.target.value)}
                                                    />
                                                </label>
                                            )}
                                            {transport === "git" && (
                                                <>
                                                    <label>
                                                        Revision type<select
                                                            value={revisionKind}
                                                            onChange={event => setRevisionKind(event.target.value)}
                                                        >
                                                            <option value="ref">Full ref</option>
                                                            <option value="commit">Commit</option>
                                                        </select>
                                                    </label>
                                                    <label>
                                                        Revision<input
                                                            required
                                                            value={revision}
                                                            onChange={event => setRevision(event.target.value)}
                                                        />
                                                    </label>
                                                    <label>
                                                        Repository subdirectory<input
                                                            value={path}
                                                            onChange={event => setPath(event.target.value)}
                                                            placeholder="cores/cowbell/src"
                                                        />
                                                    </label>
                                                </>
                                            )}
                                        </>
                                    )}
                                    <div className="change-workspace-actions">
                                        <button type="submit">Save configuration</button>
                                        <button type="button" onClick={() => navigate(() => setEditor(null))}>
                                            Cancel
                                        </button>
                                    </div>
                                </fieldset>
                            </form>
                        )
                        : item && (
                            <>
                                {section === "settings" && (
                                    <section aria-label="Package settings">
                                        <p>
                                            {item.objects.length} managed objects · Configuration generation{" "}
                                            {item.generation}
                                        </p>
                                        <p>
                                            Source scope:{" "}
                                            {item.complete.length === item.objects.length && item.objects.length
                                                ? "complete"
                                                : item.complete.length
                                                ? "mixed"
                                                : "partial"}. Omitted members stay unchanged.
                                        </p>
                                        {isWizard && service.supports("propose_objects") && (
                                            <button
                                                type="button"
                                                disabled={busy || Boolean(item.active)}
                                                onClick={() => navigate(() => edit("bindings"))}
                                            >
                                                Object bindings and source scope
                                            </button>
                                        )}
                                        <p>
                                            Upstream: {typeof item.upstream === "string"
                                                ? item.upstream || "Upload only"
                                                : `${item.upstream.repository} / ${item.upstream.path} (${
                                                    Object.values(item.upstream.revision)[0]
                                                })`}
                                        </p>
                                        {service.supports("upstream") && (
                                            <button
                                                type="button"
                                                disabled={busy || Boolean(item.active)}
                                                onClick={() => navigate(() => edit("upstream"))}
                                            >
                                                Configure upstream
                                            </button>
                                        )}
                                    </section>
                                )}
                                {section === "updates" && !item.active && (
                                    <section className="change-workspace-primary" aria-label="Check upstream">
                                        <h3>{item.name} updates</h3>
                                        <p>
                                            {item.upstream
                                                ? "Check the configured upstream. Nothing changes in the running MOO until you review and apply."
                                                : "Configure an upstream in Settings, or review supplied objdef files."}
                                        </p>
                                        {gitRestricted && <p>Fetching Git source requires wizard authority.</p>}
                                        <button
                                            type="button"
                                            className="btn-primary"
                                            disabled={busy || !canStage}
                                            onClick={() =>
                                                void stage()}
                                        >
                                            Check for updates
                                        </button>
                                    </section>
                                )}
                                {item.active
                                    ? (
                                        <section aria-label="Active review" hidden={section !== "updates"}>
                                            <h3>Review {item.active}</h3>
                                            <p role="status">{active?.status ?? "Loading review status…"}</p>
                                            {active?.error?.message && <p role="alert">{active.error.message}</p>}
                                            {active && service.supports("review") && (
                                                <button
                                                    type="button"
                                                    disabled={busy || ["fetching", "failed"].includes(active.status)}
                                                    onClick={() => setReview(target(item.active, active.generation))}
                                                >
                                                    Open review
                                                </button>
                                            )}
                                            {active?.status === "failed" && service.supports("discard") && (
                                                <button
                                                    type="button"
                                                    disabled={busy}
                                                    onClick={() =>
                                                        void run(async () => {
                                                            await service.discard(item.active, active.generation);
                                                            const started = await service.stage(
                                                                item.name,
                                                                [],
                                                                "update",
                                                            );
                                                            setReview(started);
                                                        })}
                                                >
                                                    Retry after fixing the source or worker
                                                </button>
                                            )}
                                            {active && service.supports("discard") && (
                                                <button
                                                    type="button"
                                                    disabled={busy || active.status === "applying"}
                                                    onClick={() => setConfirmDiscard(true)}
                                                >
                                                    Discard review
                                                </button>
                                            )}
                                        </section>
                                    )
                                    : (
                                        <section
                                            aria-label="Stage source"
                                            hidden={section !== "upload" && section !== "repair"}
                                        >
                                            <h3>
                                                {section === "repair"
                                                    ? "Repair accepted history"
                                                    : "Review supplied files"}
                                            </h3>
                                            {section === "repair" && (
                                                <p>
                                                    Use the original upstream source that this world accepted. Adoption
                                                    records that version for future comparisons without replacing live
                                                    code. It is not a way to merge an unknown version.
                                                </p>
                                            )}
                                            {capabilities.transports.includes("upload") && (
                                                <label>
                                                    Upload objdef files<input
                                                        type="file"
                                                        multiple
                                                        disabled={busy}
                                                        onChange={event =>
                                                            setFiles(Array.from(event.target.files ?? []))}
                                                    />
                                                </label>
                                            )}
                                            <p>
                                                Source limit: {capabilities.maxSourceBytes.toLocaleString()} bytes.{" "}
                                                {files.length
                                                    ? `${files.length} files selected.`
                                                    : "Fetch uses the configured upstream."}
                                            </p>
                                            <label hidden={section !== "repair"}>
                                                <input
                                                    type="checkbox"
                                                    checked={adopting}
                                                    disabled={busy}
                                                    onChange={event => setAdopting(event.target.checked)}
                                                />{" "}
                                                Adopt supplied source as the baseline
                                            </label>
                                            {adopting && (
                                                <p>
                                                    Supply the original upstream source. Applying adoption records its
                                                    baseline without replacing live programs.
                                                </p>
                                            )}
                                            {gitRestricted && <p>Fetching Git source requires wizard authority.</p>}
                                            <button
                                                type="button"
                                                disabled={busy || !canStage}
                                                onClick={() => void stage()}
                                            >
                                                {files.length ? "Review uploaded source" : "Fetch source"}
                                            </button>
                                        </section>
                                    )}
                                {service.supports("history") && (
                                    <section aria-label="Completed reviews" hidden={section !== "history"}>
                                        <h3>Completed reviews</h3>
                                        {!history.length && <p>No retained completed reviews.</p>}
                                        {history.map(receipt => (
                                            <p key={receipt.review}>
                                                <button
                                                    type="button"
                                                    disabled={busy}
                                                    onClick={() =>
                                                        setReview(target(receipt.review, receipt.generation))}
                                                >
                                                    Review {receipt.review}
                                                </button>{" "}
                                                · {receipt.status} · {receipt.decisionCount} decisions · by{" "}
                                                {receipt.actor || "the requesting administrator"} ·{" "}
                                                {new Date(receipt.completedAt * 1000).toLocaleString()}
                                                <small>
                                                    {String(receipt.provenance.commit ?? receipt.provenance.url ?? "")}
                                                </small>
                                            </p>
                                        ))}
                                    </section>
                                )}
                            </>
                        )}
                </div>
            </DialogSheet>
            {confirmDiscard && active && item && (
                <DialogSheet
                    title="Discard review?"
                    titleId="discard-review-title"
                    onCancel={() => setConfirmDiscard(false)}
                >
                    <div className="dialog-sheet-content">
                        This removes staged source, saved choices, and saved drafts. It does not undo applied changes.
                    </div>
                    <div className="dialog-sheet-footer">
                        <button type="button" disabled={busy} onClick={() => setConfirmDiscard(false)}>Cancel</button>
                        <button
                            type="button"
                            disabled={busy}
                            onClick={() =>
                                void run(async () => {
                                    await service.discard(item.active, active.generation);
                                    setConfirmDiscard(false);
                                    await load();
                                })}
                        >
                            Discard saved review
                        </button>
                    </div>
                </DialogSheet>
            )}
        </>
    );
}
