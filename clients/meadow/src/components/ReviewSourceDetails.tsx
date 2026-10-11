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

import type { DiagnosticPage, ReviewStatus } from "../lib/change-review";

const explanations: Record<string, string> = {
    missing_source: "No upstream definition was supplied for this object.",
    live_definition_unmatched: "No matching upstream verb definition. The live verb stays unchanged.",
    unsupported_creation: "This upstream object does not exist locally. Object creation is not supported.",
    unsupported_or_ambiguous_definition:
        "The upstream verb cannot be matched to an existing definition. Definition changes are not supported.",
    definition_fields_unmanaged:
        "The source includes verb metadata or different ownership or flags. Apply does not update these fields.",
    property_fields_unmanaged:
        "Existing property values and overrides remain unchanged. Only approved absent properties can be created.",
};
const sourceFields: Record<string, string> = {
    transport: "Transport",
    repository: "Repository",
    revision: "Requested revision",
    commit: "Resolved commit",
    tree: "Tree",
    path: "Subdirectory",
    url: "URL",
    etag: "ETag",
    digest: "Source digest",
};
const outcomes: Record<string, string> = {
    incoming: "Accepted upstream",
    local: "Kept local",
    edited: "Used edited program",
    defer: "Deferred",
};
export function ReviewSourceDetails({ status, diagnostics, busy, onMore }: {
    status: ReviewStatus | null;
    diagnostics: DiagnosticPage | null;
    busy: boolean;
    onMore: () => void;
}) {
    const provenance = Object.entries(status?.provenance ?? {}).filter(([key]) => Object.hasOwn(sourceFields, key));
    const receipt = status?.receipt;
    return (
        <details className="change-review-source">
            <summary>Source and checks{diagnostics ? ` (${diagnostics.total})` : ""}</summary>
            <p>
                Apply updates selected programs and installs declared verb and property additions. Grouping does not
                establish compatibility with omitted structural changes.
            </p>
            {provenance.length > 0 && (
                <dl>
                    {provenance.map(([key, value]) => (
                        <div key={key}>
                            <dt>{sourceFields[key]}</dt>
                            <dd>{typeof value === "string" ? value : JSON.stringify(value)}</dd>
                        </div>
                    ))}
                </dl>
            )}
            {diagnostics && (
                <>
                    {diagnostics.rows.map((item, index) => (
                        <p key={`${item.code}:${index}`}>
                            <strong>{item.label}</strong>{" "}
                            {item.message ?? explanations[item.code] ?? item.code.replaceAll("_", " ")}
                        </p>
                    ))}
                    {diagnostics.next > 0 && (
                        <button type="button" disabled={busy} onClick={onMore}>More checks</button>
                    )}
                </>
            )}
            {receipt && (
                <section aria-label="Apply receipt">
                    <h3>Committed outcome</h3>
                    <p>
                        {receipt.decisionCount} decisions committed at{" "}
                        {new Date(receipt.completedAt * 1000).toLocaleString()}.
                    </p>
                    <p>
                        Program decisions acknowledge upstream baselines. Local and edited choices can retain different
                        code. Existing properties retain their live state and accepted baselines.
                    </p>
                    <ul>
                        {receipt.decisions.map(item => (
                            <li key={item.id}>
                                <code>{item.label}</code>: {outcomes[item.choice] ?? item.choice}
                            </li>
                        ))}
                    </ul>
                    {receipt.decisions.length < receipt.decisionCount && (
                        <p>The receipt retains details for the first {receipt.decisions.length} decisions.</p>
                    )}
                </section>
            )}
        </details>
    );
}
