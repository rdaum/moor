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

import { useId, useState } from "react";
import { CustomPronouns, pronounFields } from "../lib/pronouns";

interface CustomPronounEditorProps {
    initial: CustomPronouns;
    preview: (input: CustomPronouns) => Promise<string[]>;
    onApply: (input: CustomPronouns) => Promise<void>;
    onCancel: () => void;
}

export function CustomPronounEditor({ initial, preview, onApply, onCancel }: CustomPronounEditorProps) {
    const id = useId();
    const [draft, setDraft] = useState(initial);
    const [sentences, setSentences] = useState<string[] | null>(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const complete = pronounFields.every(({ key }) => draft[key].trim().length > 0 && draft[key].length <= 64);

    const change = (key: keyof CustomPronouns, value: string) => {
        setDraft(previous => ({ ...previous, [key]: value }));
        setSentences(null);
        setError(null);
    };
    const act = async (apply: boolean) => {
        setBusy(true);
        setError(null);
        if (!apply) setSentences(null);
        try {
            if (apply) await onApply(draft);
            else setSentences(await preview(draft));
        } catch (err) {
            setError(err instanceof Error ? err.message : "Could not update pronouns. Please try again.");
        } finally {
            setBusy(false);
        }
    };

    return (
        <fieldset disabled={busy} className="custom-pronoun-editor">
            <legend>Custom pronouns</legend>
            <p>Fill each blank with the word you want others to use. The examples use ze/zir.</p>
            {pronounFields.map(({ key, label, example }) => (
                <div className="custom-pronoun-field" key={key}>
                    <label htmlFor={`${id}-${key}`}>{label}</label>
                    <input
                        id={`${id}-${key}`}
                        type="text"
                        value={draft[key]}
                        placeholder={example}
                        maxLength={64}
                        onChange={event => change(key, event.target.value)}
                        autoCapitalize="none"
                        spellCheck={false}
                    />
                </div>
            ))}
            <label htmlFor={`${id}-agreement`}>Which sentence fits?</label>
            <select
                id={`${id}-agreement`}
                value={draft.agreement}
                onChange={event => change("agreement", event.target.value)}
            >
                <option value="is">{draft.subject || "___"} is ready; {draft.subject || "___"} has a map.</option>
                <option value="are">{draft.subject || "___"} are ready; {draft.subject || "___"} have a map.</option>
            </select>
            {error && <p role="alert">{error}</p>}
            {sentences && (
                <div aria-live="polite">
                    <p>Here is how others will refer to you:</p>
                    <ul>{sentences.map((sentence, index) => <li key={index}>{sentence}</li>)}</ul>
                </div>
            )}
            <div className="custom-pronoun-actions">
                <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={!complete}
                    onClick={() => void act(false)}
                >
                    Preview sentences
                </button>
                <button type="button" className="btn btn-primary" disabled={!sentences} onClick={() => void act(true)}>
                    Use these pronouns
                </button>
                <button type="button" className="btn btn-secondary" onClick={onCancel}>Cancel</button>
            </div>
        </fieldset>
    );
}
