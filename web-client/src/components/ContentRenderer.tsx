// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

import React, { useCallback, useMemo, useRef } from "react";
import { renderDjot, renderHtmlContent, renderPlainText } from "../lib/djot-renderer";

interface ContentRendererProps {
    content: string | string[];
    contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback" | "text/x-uri";
    onLinkClick?: (url: string, position?: { x: number; y: number }) => void;
    onLinkHoldStart?: (url: string, position: { x: number; y: number }) => void;
    onLinkHoldEnd?: () => void;
}

const HOLD_THRESHOLD_MS = 300;

export const ContentRenderer: React.FC<ContentRendererProps> = ({
    content,
    contentType = "text/plain",
    onLinkClick,
    onLinkHoldStart,
    onLinkHoldEnd,
}) => {
    // Touch state tracking for tap vs hold detection
    const touchStateRef = useRef<
        {
            url: string;
            position: { x: number; y: number };
            timer: number | null;
            isHolding: boolean;
        } | null
    >(null);

    // Handle content that might be an array or string
    const getContentString = useCallback((joinWith: string = "\n") => {
        if (Array.isArray(content)) {
            return content.join(joinWith);
        }
        return typeof content === "string" ? content : String(content);
    }, [content]);

    // Unified click handler for moo-link spans (all moo-link-* variants)
    const handleLinkClick = useCallback((e: React.MouseEvent) => {
        const target = e.target as HTMLElement;
        // Check for data-url attribute which all our link spans have
        const url = target.getAttribute("data-url");
        if (url && onLinkClick) {
            e.preventDefault();
            // Pass click position for popovers
            onLinkClick(url, { x: e.clientX, y: e.clientY });
        }
    }, [onLinkClick]);

    // Touch start: begin tracking for hold detection on inspect links
    const handleTouchStart = useCallback((e: React.TouchEvent) => {
        const target = e.target as HTMLElement;
        const url = target.getAttribute("data-url");

        // Only handle inspect links with hold behavior
        if (!url?.startsWith("moo://inspect/") || !onLinkHoldStart) return;

        // Prevent native long-press context menu / text selection
        e.preventDefault();

        const touch = e.touches[0];
        const position = { x: touch.clientX, y: touch.clientY };

        // Clear any existing state
        if (touchStateRef.current?.timer) {
            clearTimeout(touchStateRef.current.timer);
        }

        // Start hold detection timer
        const timer = window.setTimeout(() => {
            if (touchStateRef.current) {
                touchStateRef.current.isHolding = true;
                onLinkHoldStart(url, position);
            }
        }, HOLD_THRESHOLD_MS);

        touchStateRef.current = { url, position, timer, isHolding: false };
    }, [onLinkHoldStart]);

    // Touch end: either complete tap or end hold preview
    const handleTouchEnd = useCallback((e: React.TouchEvent) => {
        const state = touchStateRef.current;
        if (!state) return;

        // Clear the hold timer
        if (state.timer) {
            clearTimeout(state.timer);
        }

        if (state.isHolding) {
            // Was holding - dismiss the preview
            e.preventDefault();
            onLinkHoldEnd?.();
        } else {
            // Was a quick tap - let click handler show persistent popover
            // The click event will fire naturally
        }

        touchStateRef.current = null;
    }, [onLinkHoldEnd]);

    // Touch cancel: clean up state
    const handleTouchCancel = useCallback(() => {
        if (touchStateRef.current?.timer) {
            clearTimeout(touchStateRef.current.timer);
        }
        if (touchStateRef.current?.isHolding) {
            onLinkHoldEnd?.();
        }
        touchStateRef.current = null;
    }, [onLinkHoldEnd]);

    // Prevent context menu on inspect links (Firefox long-press)
    const handleContextMenu = useCallback((e: React.MouseEvent) => {
        const target = e.target as HTMLElement;
        const url = target.getAttribute("data-url");
        if (url?.startsWith("moo://inspect/")) {
            e.preventDefault();
        }
    }, []);

    const renderedContent = useMemo(() => {
        switch (contentType) {
            case "text/html": {
                const htmlContent = getContentString("\n");
                const processedHtml = renderHtmlContent(htmlContent);

                return (
                    <div
                        dangerouslySetInnerHTML={{ __html: processedHtml }}
                        onClick={handleLinkClick}
                        onTouchStart={handleTouchStart}
                        onTouchEnd={handleTouchEnd}
                        onTouchCancel={handleTouchCancel}
                        onContextMenu={handleContextMenu}
                        className="content-html"
                    />
                );
            }

            case "text/djot": {
                try {
                    const djotContent = getContentString("\n");
                    const processedDjotHtml = renderDjot(djotContent, {
                        linkHandler: {
                            className: "moo-link",
                            dataAttribute: "data-url",
                        },
                        addTableClass: true,
                    });

                    return (
                        <div
                            className="text_djot content-html"
                            dangerouslySetInnerHTML={{ __html: processedDjotHtml }}
                            onClick={handleLinkClick}
                            onTouchStart={handleTouchStart}
                            onTouchEnd={handleTouchEnd}
                            onTouchCancel={handleTouchCancel}
                            onContextMenu={handleContextMenu}
                        />
                    );
                } catch (error) {
                    console.warn("Failed to parse djot content:", error);
                    return (
                        <div className="content-text">
                            {content}
                        </div>
                    );
                }
            }

            case "text/traceback": {
                const tracebackContent = getContentString("\n");
                return (
                    <pre className="traceback_narrative">
                        {tracebackContent}
                    </pre>
                );
            }

            case "text/x-uri": {
                const uri = getContentString("").trim();
                return (
                    <iframe
                        src={uri}
                        className="content-iframe"
                        title="Welcome content"
                        sandbox="allow-same-origin allow-scripts allow-popups allow-forms"
                    />
                );
            }

            case "text/plain":
            default: {
                const plainContent = getContentString("\n");
                const renderedHtml = renderPlainText(plainContent);

                return (
                    <div
                        dangerouslySetInnerHTML={{ __html: renderedHtml }}
                        className="content-text"
                    />
                );
            }
        }
    }, [
        contentType,
        getContentString,
        handleLinkClick,
        handleTouchStart,
        handleTouchEnd,
        handleTouchCancel,
        handleContextMenu,
        content,
    ]);

    return renderedContent;
};
