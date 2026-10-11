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

import { effectLabels, type PreparedReview } from "../lib/change-review";

export function ReviewEffects({ plan, busy, onMore }: {
    plan: PreparedReview;
    busy: boolean;
    onMore: () => void;
}) {
    return (
        <section className="change-review-effects" aria-label="Planned effects">
            <h3>What applying will do</h3>
            <p>These counts cover the whole review, including automatic choices on pages you have not opened.</p>
            <dl className="change-review-effect-counts">
                {Object.entries(plan.counts).filter(([, count]) => count > 0).map(([effect, count]) => (
                    <div key={effect}>
                        <dt>{effectLabels[effect] ?? effect}</dt>
                        <dd>{count}</dd>
                    </div>
                ))}
            </dl>
            <table className="change-review-table">
                <thead>
                    <tr>
                        <th scope="col">Item</th>
                        <th scope="col">Effect</th>
                        <th scope="col">Choice</th>
                    </tr>
                </thead>
                <tbody>
                    {plan.effects.map(item => (
                        <tr key={item.id}>
                            <th scope="row">
                                <code>{item.label}</code>
                            </th>
                            <td>{effectLabels[item.effect] ?? item.effect}</td>
                            <td>{item.defaulted ? "Automatic" : "Saved"}</td>
                        </tr>
                    ))}
                </tbody>
            </table>
            {!!plan.next && <button type="button" disabled={busy} onClick={onMore}>More planned effects</button>}
            {!plan.canApply && <p role="status">Nothing is ready to apply, or a saved decision needs attention.</p>}
        </section>
    );
}
