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

import { useEffect, useId, useState } from "react";
import { ChangeGroup, ChangeReviewClient, ChangeRow } from "../lib/change-review";
import { DialogSheet } from "./DialogSheet";

interface Props {
    client: ChangeReviewClient;
    generation: number;
    groups: ChangeGroup[];
    group?: string;
    onSaved: (generation: number) => void;
    onClose: () => void;
}

/** Group policy is explicit administrator input. A fetched program cannot declare its own authority. */
export function ChangeGroupEditor({ client, generation, groups, group, onSaved, onClose }: Props) {
    const id = useId();
    const [rows, setRows] = useState<ChangeRow[]>([]);
    const [name, setName] = useState(group ?? "");
    const [members, setMembers] = useState<Set<string>>(new Set());
    const [requires, setRequires] = useState<Set<string>>(
        new Set(groups.find(item => item.name === group)?.requires ?? []),
    );
    const [dirty, setDirty] = useState(false);
    const [busy, setBusy] = useState(true);
    const [error, setError] = useState("");
    useEffect(() => {
        let cancelled = false;
        void (async () => {
            const all: ChangeRow[] = [];
            let cursor: number[] = [];
            do {
                const page = await client.page(generation, cursor);
                if (cancelled) return;
                all.push(...page.rows);
                cursor = page.cursor;
            } while (cursor.length);
            setRows(all);
            setMembers(new Set(all.filter(row => row.group === group && group).map(row => row.member!)));
            setBusy(false);
        })().catch(failure => {
            if (!cancelled) {
                setError(failure instanceof Error ? failure.message : "Could not load group members.");
                setBusy(false);
            }
        });
        return () => {
            cancelled = true;
        };
    }, [client, generation, group]);
    useEffect(() => {
        if (!dirty) return;
        const prevent = (event: BeforeUnloadEvent) => {
            event.preventDefault();
            event.returnValue = "";
        };
        window.addEventListener("beforeunload", prevent);
        return () => window.removeEventListener("beforeunload", prevent);
    }, [dirty]);
    const close = () => {
        if (!dirty || window.confirm("Discard the unsaved group definition?")) onClose();
    };
    const save = async (remove = false) => {
        if (busy) return;
        setBusy(true);
        setError("");
        try {
            const next = await client.setGroup(
                generation,
                name.trim(),
                remove ? [] : [...members],
                remove ? [] : [...requires],
            );
            setDirty(false);
            onSaved(next);
        } catch (failure) {
            setError(failure instanceof Error ? failure.message : "Could not save the group.");
            setBusy(false);
        }
    };
    return (
        <DialogSheet
            title={group ? `Edit group: ${group}` : "Create a change group"}
            titleId={`${id}-title`}
            onCancel={close}
            maxWidth="760px"
        >
            <div className="change-group-editor" aria-busy={busy}>
                <p>
                    A group contains changes that must be installed together. Include each new member and every program
                    that needs it.
                </p>
                <p>
                    The review cannot discover all dependencies from program text. You must identify them. Missing
                    declarations block new additions.
                </p>
                {error && <p role="alert">{error}</p>}
                <label>
                    Group name<input
                        value={name}
                        maxLength={64}
                        disabled={Boolean(group) || busy}
                        onChange={event => {
                            setName(event.target.value);
                            setDirty(true);
                        }}
                    />
                </label>
                <fieldset disabled={busy}>
                    <legend>Required changes</legend>
                    {rows.filter(row => row.member).map(row => (
                        <label key={row.id}>
                            <input
                                type="checkbox"
                                checked={members.has(row.member!)}
                                disabled={Boolean(row.group && row.group !== group)}
                                onChange={event => {
                                    const selected = event.target.checked;
                                    setMembers(previous => {
                                        const next = new Set(previous);
                                        if (selected) next.add(row.member!);
                                        else next.delete(row.member!);
                                        return next;
                                    });
                                    setDirty(true);
                                }}
                            />
                            {row.label}
                            {row.operation === "add_verb"
                                ? " — new verb"
                                : row.operation === "add_property"
                                ? " — new property"
                                : row.operation === "retain_property"
                                ? " — keep existing property"
                                : row.operation === "add_object"
                                ? " — new object and contents"
                                : row.operation === "retain_object"
                                ? " — keep existing object"
                                : " — existing program"}
                            {row.group && row.group !== group && ` (belongs to ${row.group})`}
                        </label>
                    ))}
                </fieldset>
                {groups.some(item => item.name !== group) && (
                    <fieldset disabled={busy}>
                        <legend>Other groups required by this group</legend>
                        {groups.filter(item => item.name !== group).map(item => (
                            <label key={item.name}>
                                <input
                                    type="checkbox"
                                    checked={requires.has(item.name)}
                                    onChange={event => {
                                        const selected = event.target.checked;
                                        setRequires(previous => {
                                            const next = new Set(previous);
                                            if (selected) next.add(item.name);
                                            else next.delete(item.name);
                                            return next;
                                        });
                                        setDirty(true);
                                    }}
                                />
                                {item.name}
                            </label>
                        ))}
                    </fieldset>
                )}
                <p>
                    Saving or removing a group clears this review's saved choices. It changes package rules, but it does
                    not change running code.
                </p>
                <div className="change-group-actions">
                    <button type="button" disabled={busy} onClick={close}>Cancel</button>
                    {group && (
                        <button type="button" disabled={busy} onClick={() => void save(true)}>
                            Remove group and clear choices
                        </button>
                    )}
                    <button type="button" disabled={busy || !name.trim() || !members.size} onClick={() => void save()}>
                        Save group and clear choices
                    </button>
                </div>
            </div>
        </DialogSheet>
    );
}
