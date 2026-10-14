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

// Copyright (C) 2026 The mooR Authors
// SPDX-License-Identifier: GPL-3.0-or-later

import { SemanticAnnotation } from "@moor/web-sdk";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { checkReferences } from "../context/ArgumentContext";
import { useFloatingCard } from "../hooks/useFloatingCard";
import { Suggestion } from "../hooks/useSuggestions";
import { SemanticIcon } from "./SemanticIcon";
import { SuggestionInput } from "./SuggestionInput";

type CommandAnnotation = Extract<SemanticAnnotation, { kind: "command" }>;
interface Props {
    annotation: CommandAnnotation;
    position: { x: number; y: number };
    authToken?: string | null;
    revision?: number;
    onSubmit: (command: string, context?: CommandAnnotation) => boolean;
    onClose: () => void;
}
interface Draft {
    text: string;
    value: string;
    reference?: string;
}

export function annotationCommand(annotation: CommandAnnotation, drafts: Record<string, Draft>): string {
    if (annotation.command !== undefined) return annotation.command;
    let command = annotation.template;
    for (const [slot, field] of Object.entries(annotation.arguments)) {
        const value = drafts[slot]?.value.trim() ?? "";
        if (field.required !== false && !value) throw new Error(`Enter ${field.label.toLowerCase()}.`);
        if (/[\r\n]/.test(value)) throw new Error("Enter a single-line argument.");
        command = command.replace(`{${slot}}`, () => value);
    }
    return command.trim();
}

/** Argument collection remains nonmodal so existing output can supply a reference. */
export function AnnotationCommandReview(
    { annotation, position, authToken = null, revision = 0, onSubmit, onClose }: Props,
) {
    const id = useId();
    const actionLabel = annotation.action?.label ?? (annotation.exit ? "Go" : "Run command");
    const actionTitle = annotation.action?.title ?? actionLabel;
    const actionIcon = annotation.action?.icon;
    const { cardRef, isDragging, dragHandlers } = useFloatingCard(position, true, true);
    const submitted = useRef(false);
    useEffect(() => {
        const previous = document.activeElement as HTMLElement | null;
        cardRef.current?.focus({ preventScroll: true });
        return () => {
            // Returning focus to an old transcript link after submission pins new output.
            // Cancellation still returns to the control that opened this card.
            if (!submitted.current && previous?.isConnected) previous.focus({ preventScroll: true });
        };
    }, [cardRef]);
    const fields = useMemo(() => Object.entries(annotation.arguments ?? {}), [annotation]);
    const [drafts, setDrafts] = useState<Record<string, Draft>>(() =>
        Object.fromEntries(fields.map(([slot, field]) => [slot, {
            text: field.bound?.label ?? "",
            value:
                (field.expectedKind === "object" ? field.bound?.value.replace(/^(oid|uuid):/, "#") : field.bound?.value)
                    ?? "",
            reference: field.expectedKind === "object" ? field.bound?.value : undefined,
        }]))
    );
    const [error, setError] = useState<string | null>(null);
    const [invalid, setInvalid] = useState<Record<string, string>>({});
    const [sending, setSending] = useState(false);
    const generation = useRef(0);
    useEffect(() => {
        const current = ++generation.current;
        return () => {
            generation.current = current + 1;
        };
    }, [drafts, authToken, revision, annotation]);
    const contextFor = (active: string) => ({
        template: annotation.template ?? "",
        active,
        bindings: Object.fromEntries(
            Object.entries(drafts).filter(([, draft]) => draft.value).map(([slot, draft]) => [slot, draft.value]),
        ),
    });
    const validate = async () => {
        const failures: Record<string, string> = {};
        for (const [slot, field] of fields) {
            const reference = drafts[slot]?.reference;
            if (!reference || !field.suggestions) continue;
            if (!authToken) {
                failures[slot] = "Reconnect to check this argument.";
                continue;
            }
            const result = await checkReferences(authToken, { ...field.suggestions, context: contextFor(slot) }, [
                reference,
            ]);
            if (!result[reference]?.eligible) {
                failures[slot] = "This selection is no longer available for this argument.";
            }
        }
        return failures;
    };
    useEffect(() => {
        let current = true;
        void validate().then(result => {
            if (current) setInvalid(result);
        }).catch(() => {
            if (current) setError("Could not check the selected arguments.");
        });
        return () => {
            current = false;
        };
    }, [drafts, authToken, revision, annotation]); // eslint-disable-line react-hooks/exhaustive-deps
    let command = annotation.command
        ?? annotation.template.replace(/\{(dobj|iobj)\}/g, (_, slot: string) => drafts[slot]?.value || "…");
    let complete = true;
    try {
        command = annotationCommand(annotation, drafts);
    } catch {
        complete = false;
    }
    const submit = async () => {
        if (sending) return;
        setSending(true);
        const request = generation.current;
        try {
            const checked = await validate();
            if (request !== generation.current) {
                setError("The context changed. Review the command and try again.");
                return;
            }
            setInvalid(checked);
            if (Object.keys(checked).length) return;
            const command = annotationCommand(annotation, drafts);
            const sent = annotation.exit ? onSubmit(command, annotation) : onSubmit(command);
            if (!sent) {
                setError("Not connected. Reconnect before running this command.");
                return;
            }
            submitted.current = true;
            onClose();
        } catch (cause) {
            setError(cause instanceof Error ? cause.message : "Could not send this command.");
        } finally {
            setSending(false);
        }
    };
    return (
        <div
            ref={cardRef}
            className="command-card"
            role="dialog"
            aria-modal="false"
            aria-labelledby={id}
            tabIndex={-1}
            onKeyDown={event => {
                if (event.key === "Escape" && !event.defaultPrevented) {
                    event.preventDefault();
                    onClose();
                }
            }}
        >
            <div className="command-card-header" data-dragging={isDragging || undefined} {...dragHandlers}>
                <h2 id={id}>
                    <SemanticIcon kind={actionIcon} />
                    {actionTitle}
                </h2>
                <button type="button" aria-label="Close command" title="Close" onClick={onClose}>×</button>
            </div>
            <details className="command-card-syntax">
                <summary>
                    Command <code>{command}</code>
                </summary>
                <pre className="annotation-command-preview">{command}</pre>
            </details>
            <div className="command-card-content">
                {fields.map(([slot, field]) => (
                    <div className="annotation-command-field" key={slot}>
                        <label htmlFor={`${id}-${slot}`}>{field.label}</label>
                        <SuggestionInput
                            id={`${id}-${slot}`}
                            argumentLabel={field.label}
                            authToken={authToken}
                            revision={revision}
                            value={drafts[slot]?.text ?? ""}
                            source={field.expectedKind === "object" && field.suggestions
                                ? { ...field.suggestions, context: contextFor(slot) }
                                : undefined}
                            onChange={(text: string, selection?: Suggestion) => {
                                setError(null);
                                setDrafts(current => ({
                                    ...current,
                                    [slot]: {
                                        text,
                                        value: selection?.value ?? text,
                                        reference: selection?.value.startsWith("#")
                                            ? (selection.value.includes("-") ? "uuid:" : "oid:")
                                                + selection.value.slice(1)
                                            : undefined,
                                    },
                                }));
                            }}
                        />
                        {invalid[slot] && <small role="alert">{invalid[slot]}</small>}
                    </div>
                ))}
                {error && <p role="alert">{error}</p>}
            </div>
            <div className="command-card-footer">
                <small>
                    {fields.some(([, field]) => field.expectedKind === "object" && field.suggestions)
                        ? "Choose from suggestions or the transcript."
                        : "Results appear in the transcript."}
                </small>
                <button
                    type="button"
                    disabled={!complete || sending || Object.keys(invalid).length > 0}
                    onPointerDown={event => {
                        // Keep a suggestion menu's blur from moving this button before the click.
                        event.preventDefault();
                    }}
                    onClick={() => void submit()}
                >
                    <SemanticIcon kind={actionIcon} />
                    {actionLabel} <span aria-hidden="true">↵</span>
                </button>
            </div>
        </div>
    );
}
