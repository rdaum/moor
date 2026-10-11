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
import { ChangeReviewClient, type RecoveryPage } from "../lib/change-review";

export function ReviewRecovery({ client, generation }: { client: ChangeReviewClient; generation: number }) {
    const [page, setPage] = useState<RecoveryPage | null>(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState("");
    useEffect(() => {
        let cancelled = false;
        setPage(null);
        setBusy(true);
        void client.recovery(generation).then(next => {
            if (!cancelled) setPage(next);
        })
            .catch(failure => {
                if (!cancelled) setError(String(failure));
            })
            .finally(() => {
                if (!cancelled) setBusy(false);
            });
        return () => {
            cancelled = true;
        };
    }, [client, generation]);
    const more = async () => {
        if (!page?.next || busy) return;
        setBusy(true);
        try {
            const next = await client.recovery(generation, page.next);
            setPage({ ...next, items: [...page.items, ...next.items] });
        } catch (failure) {
            setError(String(failure));
        } finally {
            setBusy(false);
        }
    };
    return (
        <section className="change-review-details" aria-label="Saved work" aria-busy={busy}>
            <h3>Saved work that needs another check</h3>
            <p>
                Changed or missing members keep their earlier decisions here. Review the current contents before saving
                a choice again.
            </p>
            {error && <p role="alert">{error}</p>}
            {page?.total === 0 && <p>No saved decisions need recovery.</p>}
            {page?.items.map(item => (
                <article key={item.id}>
                    <h4>{item.label}</h4>
                    <p>{item.reason}</p>
                    {item.program !== undefined && (
                        <textarea
                            aria-label={`Saved program for ${item.label}`}
                            readOnly
                            value={item.program}
                            rows={8}
                        />
                    )}
                </article>
            ))}
            {!!page?.next && <button type="button" disabled={busy} onClick={() => void more()}>More saved work</button>}
        </section>
    );
}
