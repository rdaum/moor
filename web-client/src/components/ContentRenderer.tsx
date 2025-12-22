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

import React, { useCallback, useMemo } from "react";
import { renderDjot, renderHtmlContent, renderPlainText } from "../lib/djot-renderer";

interface ContentRendererProps {
    content: string | string[];
    contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback" | "text/x-uri";
    onLinkClick?: (url: string) => void;
}

export const ContentRenderer: React.FC<ContentRendererProps> = ({
    content,
    contentType = "text/plain",
    onLinkClick,
}) => {
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
            onLinkClick(url);
        }
    }, [onLinkClick]);

    const renderedContent = useMemo(() => {
        switch (contentType) {
            case "text/html": {
                const htmlContent = getContentString("\n");
                const processedHtml = renderHtmlContent(htmlContent);

                return (
                    <div
                        dangerouslySetInnerHTML={{ __html: processedHtml }}
                        onClick={handleLinkClick}
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
    }, [contentType, getContentString, handleLinkClick, content]);

    return renderedContent;
};
