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

import { useEffect, useState } from "react";
import { ChangeReviewClient, effectLabels, type ReviewReceipt, type ReviewStatus } from "../lib/change-review";

export function ReviewResult({ client, status, onContinue, onFinish, canFinish }: {
    client: ChangeReviewClient;
    status: ReviewStatus;
    onContinue: () => void;
    onFinish: () => void;
    canFinish: boolean;
}) {
    const [result, setResult] = useState<ReviewReceipt | null>(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState("");
    useEffect(() => {
        let cancelled = false;
        setBusy(true);
        setError("");
        setResult(null);
        void client.result(status.generation).then(value => {
            if (!cancelled) setResult(value);
        }).catch(failure => {
            if (!cancelled) setError(String(failure));
        }).finally(() => {
            if (!cancelled) setBusy(false);
        });
        return () => {
            cancelled = true;
        };
    }, [client, status.generation]);
    const more = async () => {
        if (!result?.next || busy) return;
        setBusy(true);
        try {
            const page = await client.result(status.generation, result.next);
            setResult({ ...page, decisions: [...result.decisions, ...page.decisions] });
        } catch (failure) {
            setError(String(failure));
        } finally {
            setBusy(false);
        }
    };
    return (
        <section className="change-review-details" aria-label="Committed results" aria-busy={busy}>
            <h3>Results</h3>
            {error && <p role="alert">{error}</p>}
            {result && (
                <>
                    <p>
                        {result.decisionCount} recorded decisions. By {result.actor || "the requesting administrator"},
                        {" "}
                        {new Date(result.completedAt * 1000).toLocaleString()}.
                    </p>
                    <p>
                        {status.status === "partial"
                            ? "The selected changes are installed. Some work remains. Finish this review to leave that work for a later update."
                            : status.status === "finished"
                            ? "This review is closed. Deferred contents and baselines remain unchanged."
                            : "This review is complete. The recorded effects remain installed."}
                    </p>
                    <table className="change-review-table">
                        <thead>
                            <tr>
                                <th scope="col">Item</th>
                                <th scope="col">Recorded effect</th>
                            </tr>
                        </thead>
                        <tbody>
                            {result.decisions.map((item, index) => (
                                <tr key={`${item.id}-${index}`}>
                                    <th scope="row">
                                        <code>{item.label}</code>
                                    </th>
                                    <td>{effectLabels[item.effect] ?? item.effect}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {!!result.next && (
                        <button type="button" disabled={busy} onClick={() => void more()}>
                            More recorded decisions
                        </button>
                    )}
                    <p>
                        The service retains up to 64 complete review records within 32 MiB. There is no general rollback
                        command.
                    </p>
                </>
            )}
            {canFinish && (
                <div className="change-group-actions">
                    <button type="button" onClick={onContinue}>Continue remaining work</button>
                    <button type="button" onClick={onFinish}>Finish review</button>
                </div>
            )}
        </section>
    );
}
