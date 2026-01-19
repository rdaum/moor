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
//

import React, { useCallback, useState } from "react";
import { DialogSheet } from "./DialogSheet";

interface ExternalLinkModalProps {
    /** The URL the user wants to navigate to */
    url: string;
    /** Name of the actor who shared the link (if available) */
    actorName?: string;
    /** The verb/action that produced this link (if available) */
    verb?: string;
    /** Called when user confirms navigation. trustDomain indicates if they want to remember this domain. */
    onConfirm: (trustDomain: boolean) => void;
    /** Called when user cancels navigation */
    onCancel: () => void;
}

/**
 * Modal shown when user clicks an external http/https link.
 * Displays context about who shared the link and the full URL,
 * with option to trust the domain for future clicks.
 */
export const ExternalLinkModal: React.FC<ExternalLinkModalProps> = ({
    url,
    actorName,
    verb,
    onConfirm,
    onCancel,
}) => {
    const [trustDomain, setTrustDomain] = useState(false);

    // Parse URL components for display
    let protocol = "";
    let hostname = "";
    let path = "";

    try {
        const urlObj = new URL(url);
        protocol = urlObj.protocol;
        hostname = urlObj.hostname;
        path = urlObj.pathname + urlObj.search + urlObj.hash;
    } catch {
        // If URL parsing fails, just show the raw URL
        hostname = url;
    }

    const handleVisitSite = useCallback(() => {
        onConfirm(trustDomain);
    }, [onConfirm, trustDomain]);

    const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
        // Enter key on the form (not in the checkbox) should trigger visit
        if (e.key === "Enter" && e.target === e.currentTarget) {
            e.preventDefault();
            handleVisitSite();
        }
    }, [handleVisitSite]);

    // Format the context message
    const contextMessage = actorName
        ? verb
            ? `${actorName} shared this link via ${verb}`
            : `${actorName} shared this link`
        : null;

    // Build the screen reader description that summarizes the dialog
    const srDescription = contextMessage
        ? `${contextMessage}. Link destination: ${url}. This link will take you to an external website.`
        : `Link destination: ${url}. This link will take you to an external website.`;

    return (
        <DialogSheet
            title="External Link"
            titleId="external-link-modal-title"
            onCancel={onCancel}
            maxWidth="480px"
            role="alertdialog"
            ariaDescribedBy="external-link-description"
        >
            <div className="dialog-sheet-content" onKeyDown={handleKeyDown}>
                {/* Screen reader description - always present */}
                <p id="external-link-description" className="sr-only">
                    {srDescription}
                </p>

                {/* Visual context: who shared this link */}
                {contextMessage && (
                    <p className="external-link-context" aria-hidden="true">
                        {contextMessage}
                    </p>
                )}

                {/* Visual URL display with highlighted hostname */}
                <div className="external-link-url-display" aria-hidden="true">
                    <span className="external-link-protocol">{protocol}//</span>
                    <span className="external-link-hostname">{hostname}</span>
                    <span className="external-link-path">{path}</span>
                </div>

                {/* Visual warning message */}
                <p className="external-link-warning" aria-hidden="true">
                    This link will take you to an external website. Make sure you trust this destination before
                    proceeding.
                </p>

                {/* Trust domain checkbox */}
                <label className="external-link-trust-label">
                    <input
                        type="checkbox"
                        checked={trustDomain}
                        onChange={(e) => setTrustDomain(e.target.checked)}
                        className="external-link-trust-checkbox"
                        aria-describedby="trust-domain-hint"
                    />
                    <span>
                        Don't ask again for <strong>{hostname}</strong>
                    </span>
                </label>
                <span id="trust-domain-hint" className="sr-only">
                    Check this to allow future links to {hostname} to open without confirmation
                </span>
            </div>

            <div className="dialog-sheet-footer">
                <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={onCancel}
                >
                    Cancel
                </button>
                <button
                    type="button"
                    className="btn btn-primary"
                    onClick={handleVisitSite}
                    autoFocus
                >
                    Visit Site
                </button>
            </div>
        </DialogSheet>
    );
};
