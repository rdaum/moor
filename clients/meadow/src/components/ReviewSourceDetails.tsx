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

import type { DiagnosticPage, ReviewDiagnostic, ReviewStatus } from "../lib/change-review";

const explanations: Record<string, string> = {
    missing_source: "No upstream definition was supplied for this object.",
    live_definition_unmatched: "No matching upstream verb definition. The live verb stays unchanged.",
    unsupported_creation:
        "This object needs an explicit proposed binding in package configuration before creation can be reviewed.",
    outside_managed_scope:
        "The source includes an object outside this package's managed scope. It cannot be installed by this review.",
    unsupported_removal:
        "Complete source omits this installed member. Removal is unsupported, so the member stays unchanged.",
    unsupported_or_ambiguous_definition:
        "The upstream verb cannot be matched to an existing definition. Definition changes are not supported.",
    definition_fields_unmanaged:
        "The source includes verb metadata or different ownership or flags. Apply does not update these fields.",
    property_fields_unmanaged:
        "Existing property values and overrides remain unchanged. Only approved absent properties can be created.",
};
const sourceFields: Record<string, string> = {
    repository: "Repository",
    url: "URL",
    revision: "Branch or revision",
    commit: "Commit",
    path: "Subdirectory",
    transport: "Fetched with",
    tree: "Tree",
    etag: "ETag",
    digest: "Source digest",
};
const outcomes: Record<string, string> = {
    incoming: "Accepted upstream",
    local: "Kept local",
    edited: "Used edited program",
    defer: "Deferred",
};
const noteTitles: Record<string, string> = {
    missing_source: "Objects missing from the source",
    live_definition_unmatched: "Local verbs missing from the source",
    unsupported_creation: "New objects without an assigned ID",
    outside_managed_scope: "Objects outside this package",
    unsupported_removal: "Removals are not supported",
    unsupported_or_ambiguous_definition: "Verb definitions that cannot be matched",
    definition_fields_unmanaged: "Verb ownership, permissions, and metadata",
    property_fields_unmanaged: "Existing property values and overrides",
};
function sourceValue(value: unknown): string {
    if (typeof value === "string") return value;
    if (value && typeof value === "object" && !Array.isArray(value)) {
        return Object.entries(value).map(([key, entry]) =>
            key === "ref" || key === "commit" ? String(entry) : `${key}: ${String(entry)}`
        ).join(", ");
    }
    return String(value);
}

export function ReviewSourceDetails({ view, status, diagnostics, busy, onMore }: {
    view: "source" | "unchanged" | "results";
    status: ReviewStatus | null;
    diagnostics: DiagnosticPage | null;
    busy: boolean;
    onMore: () => void;
}) {
    const provenance = Object.keys(sourceFields)
        .map(key => [key, status?.provenance?.[key]] as const)
        .filter(([, value]) => value != null && sourceValue(value) !== "");
    const receipt = status?.receipt;
    const notes = new Map<string, ReviewDiagnostic[]>();
    for (const item of diagnostics?.rows ?? []) {
        notes.set(item.code, [...(notes.get(item.code) ?? []), item]);
    }
    return (
        <section
            className="change-review-details"
            aria-label={view === "source" ? "Upstream source" : view === "results" ? "Apply receipt" : "Left unchanged"}
        >
            {view === "source" && (
                <>
                    <h3>Upstream source</h3>
                    <p>This identifies the source used for this review.</p>
                    <table className="change-review-table change-review-source-table">
                        <tbody>
                            {provenance.map(([key, value]) => (
                                <tr key={key}>
                                    <th scope="row">{sourceFields[key]}</th>
                                    <td>
                                        <code>{sourceValue(value)}</code>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </>
            )}
            {view === "unchanged" && (
                <>
                    <h3>Left unchanged</h3>
                    <p>
                        Apply does not change these parts of the supplied source. Expand a reason to see the affected
                        items.
                    </p>
                    {[...notes].map(([code, items]) => (
                        <details className="change-review-note" key={code}>
                            <summary>
                                {noteTitles[code] ?? code.replaceAll("_", " ")} <span>({items.length})</span>
                            </summary>
                            <p>{explanations[code] ?? "These items cannot be applied by this review."}</p>
                            <table className="change-review-table">
                                <thead>
                                    <tr>
                                        <th scope="col">Item</th>
                                        {items.some(item => item.message) && <th scope="col">Reason</th>}
                                    </tr>
                                </thead>
                                <tbody>
                                    {items.map((item, index) => (
                                        <tr key={index}>
                                            <th scope="row">
                                                <code>{item.label}</code>
                                            </th>
                                            {items.some(item => item.message) && (
                                                <td>{item.message ?? explanations[code]}</td>
                                            )}
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </details>
                    ))}
                    {diagnostics && diagnostics.next > 0 && (
                        <button type="button" disabled={busy} onClick={onMore}>Load more items</button>
                    )}
                </>
            )}
            {view === "results" && receipt && (
                <>
                    <h3>Results</h3>
                    <p>
                        {receipt.decisionCount} choices applied at{" "}
                        {new Date(receipt.completedAt * 1000).toLocaleString()}.
                    </p>
                    <table className="change-review-table">
                        <thead>
                            <tr>
                                <th scope="col">Item</th>
                                <th scope="col">Result</th>
                            </tr>
                        </thead>
                        <tbody>
                            {receipt.decisions.map(item => (
                                <tr key={item.id}>
                                    <th scope="row">
                                        <code>{item.label}</code>
                                    </th>
                                    <td>{outcomes[item.choice] ?? item.choice}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {receipt.decisions.length < receipt.decisionCount && (
                        <p>Details are available for the first {receipt.decisions.length} choices.</p>
                    )}
                </>
            )}
        </section>
    );
}
