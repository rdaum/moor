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

import React, { useCallback, useEffect, useMemo, useRef } from "react";
import { renderDjot, renderHtmlContent, renderPlainText } from "../lib/djot-renderer";

/** Metadata about the event that produced this content */
export interface EventMetadata {
    verb?: string;
    actorName?: string;
    thisName?: string;
    dobjName?: string;
}

interface ContentRendererProps {
    content: string | string[];
    contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback" | "text/x-uri";
    onLinkClick?: (
        url: string,
        position?: { x: number; y: number },
        metadata?: { actorName?: string; verb?: string },
    ) => void;
    onLinkHoldStart?: (url: string, position: { x: number; y: number }) => void;
    onLinkHoldEnd?: () => void;
    isStale?: boolean;
    /** Whether to enable emoji conversion for this content. Defaults to false. */
    enableEmoji?: boolean;
    /** Metadata about the event that produced this content (for link context) */
    eventMetadata?: EventMetadata;
}

const HOLD_THRESHOLD_MS = 300;

export const ContentRenderer: React.FC<ContentRendererProps> = ({
    content,
    contentType = "text/plain",
    onLinkClick,
    onLinkHoldStart,
    onLinkHoldEnd,
    isStale = false,
    enableEmoji = false,
    eventMetadata,
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

    // Ref to container for updating tabindex on stale change
    const containerRef = useRef<HTMLSpanElement>(null);

    // Update tabindex on links when stale state or content changes
    useEffect(() => {
        if (!containerRef.current) return;
        const links = containerRef.current.querySelectorAll("[data-url]");
        links.forEach((link) => {
            (link as HTMLElement).tabIndex = isStale ? -1 : 0;
        });
    }, [isStale, content]);

    // Handle content that might be an array or string
    const getContentString = useCallback((joinWith: string = "\n") => {
        if (Array.isArray(content)) {
            return content.join(joinWith);
        }
        return typeof content === "string" ? content : String(content);
    }, [content]);

    // Check if a URL is an external link (http/https)
    const isExternalLink = (url: string) => url.startsWith("http://") || url.startsWith("https://");

    // Unified click handler for moo-link spans (all moo-link-* variants)
    const handleLinkClick = useCallback((e: React.MouseEvent) => {
        const target = e.target as HTMLElement;
        // Check for data-url attribute which all our link spans have
        const url = target.getAttribute("data-url");
        if (!url || !onLinkClick) return;

        // For stale/historical content, only allow external links (http/https)
        // MOO command links should remain disabled to prevent re-executing old commands
        if (isStale && !isExternalLink(url)) return;

        e.preventDefault();
        // Pass click position and event metadata for context
        onLinkClick(url, { x: e.clientX, y: e.clientY }, {
            actorName: eventMetadata?.actorName,
            verb: eventMetadata?.verb,
        });
    }, [onLinkClick, isStale, eventMetadata]);

    // Keyboard handler for Enter/Space on focused links
    const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
        if (e.key !== "Enter" && e.key !== " ") return;

        const target = e.target as HTMLElement;
        const url = target.getAttribute("data-url");
        if (!url || !onLinkClick) return;

        // For stale/historical content, only allow external links
        if (isStale && !isExternalLink(url)) return;

        e.preventDefault();
        // Use element position for popovers since there's no mouse position
        const rect = target.getBoundingClientRect();
        onLinkClick(url, { x: rect.left + rect.width / 2, y: rect.bottom }, {
            actorName: eventMetadata?.actorName,
            verb: eventMetadata?.verb,
        });
    }, [onLinkClick, isStale, eventMetadata]);

    // Touch start: begin tracking for hold detection on inspect links
    const handleTouchStart = useCallback((e: React.TouchEvent) => {
        // Ignore touches when content is stale
        if (isStale) return;

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
    }, [onLinkHoldStart, isStale]);

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

    const staleClass = isStale ? " content-stale" : "";

    // Helper to wrap content with sr-only link hint when links are present
    const wrapWithLinkHint = (contentElement: React.ReactElement, html: string) => {
        // Check if the HTML contains interactive links (data-url attributes)
        const hasLinks = html.includes("data-url=");
        if (!hasLinks || isStale) {
            return contentElement;
        }
        return (
            <>
                {contentElement}
                <span className="sr-only">
                    Interactive links available. Press Shift+Tab to navigate.
                </span>
            </>
        );
    };

    const renderedContent = useMemo(() => {
        switch (contentType) {
            case "text/html": {
                const htmlContent = getContentString("\n");
                const processedHtml = renderHtmlContent(htmlContent, enableEmoji);

                return wrapWithLinkHint(
                    <span
                        dangerouslySetInnerHTML={{ __html: processedHtml }}
                        onClick={handleLinkClick}
                        onKeyDown={handleKeyDown}
                        onTouchStart={handleTouchStart}
                        onTouchEnd={handleTouchEnd}
                        onTouchCancel={handleTouchCancel}
                        onContextMenu={handleContextMenu}
                        className={`content-html${staleClass}`}
                    />,
                    processedHtml,
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
                        enableEmoji,
                    });

                    return wrapWithLinkHint(
                        <span
                            className={`text_djot content-html${staleClass}`}
                            dangerouslySetInnerHTML={{ __html: processedDjotHtml }}
                            onClick={handleLinkClick}
                            onKeyDown={handleKeyDown}
                            onTouchStart={handleTouchStart}
                            onTouchEnd={handleTouchEnd}
                            onTouchCancel={handleTouchCancel}
                            onContextMenu={handleContextMenu}
                        />,
                        processedDjotHtml,
                    );
                } catch (error) {
                    console.warn("Failed to parse djot content:", error);
                    return (
                        <span className={`content-text${staleClass}`}>
                            {content}
                        </span>
                    );
                }
            }

            case "text/traceback": {
                const tracebackContent = getContentString("\n");
                return (
                    <pre className={`traceback_narrative${staleClass}`}>
                        {tracebackContent}
                    </pre>
                );
            }

            case "text/x-uri": {
                const uri = getContentString("").trim();
                return (
                    <iframe
                        src={uri}
                        className={`content-iframe${staleClass}`}
                        title="Welcome content"
                        sandbox="allow-same-origin allow-scripts allow-popups allow-forms"
                    />
                );
            }

            case "text/plain":
            default: {
                const plainContent = getContentString("\n");
                const renderedHtml = renderPlainText(plainContent, enableEmoji);

                return wrapWithLinkHint(
                    <span
                        dangerouslySetInnerHTML={{ __html: renderedHtml }}
                        onClick={handleLinkClick}
                        onKeyDown={handleKeyDown}
                        className={`content-text${staleClass}`}
                    />,
                    renderedHtml,
                );
            }
        }
    }, [
        contentType,
        getContentString,
        handleLinkClick,
        handleKeyDown,
        handleTouchStart,
        handleTouchEnd,
        handleTouchCancel,
        handleContextMenu,
        content,
        staleClass,
        isStale,
        enableEmoji,
    ]);

    return <span ref={containerRef} className="content-renderer">{renderedContent}</span>;
};
