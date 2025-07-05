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

import { parse, renderHTML } from "@djot/djot";
import DOMPurify from "dompurify";
import React, { useMemo } from "react";

interface ContentRendererProps {
    content: string | string[];
    contentType?: "text/plain" | "text/djot" | "text/html";
}

export const ContentRenderer: React.FC<ContentRendererProps> = ({
    content,
    contentType = "text/plain",
}) => {
    const renderedContent = useMemo(() => {
        // Handle content that might be an array or string
        const getContentString = (joinWith: string = "\n") => {
            if (Array.isArray(content)) {
                return content.join(joinWith);
            }
            return typeof content === "string" ? content : String(content);
        };

        switch (contentType) {
            case "text/html":
                // For HTML, join array elements with newlines
                const htmlContent = getContentString("\n");
                // Sanitize HTML content for security
                const sanitizedHtml = DOMPurify.sanitize(htmlContent, {
                    ALLOWED_TAGS: [
                        "p",
                        "br",
                        "div",
                        "span",
                        "strong",
                        "b",
                        "em",
                        "i",
                        "u",
                        "s",
                        "ul",
                        "ol",
                        "li",
                        "a",
                        "img",
                        "pre",
                        "code",
                        "blockquote",
                        "h1",
                        "h2",
                        "h3",
                        "h4",
                        "h5",
                        "h6",
                        "hr",
                        "table",
                        "thead",
                        "tbody",
                        "tr",
                        "td",
                        "th",
                        "small",
                        "sup",
                        "sub",
                    ],
                    ALLOWED_ATTR: [
                        "href",
                        "src",
                        "alt",
                        "title",
                        "class",
                        "id",
                        "target",
                        "rel",
                        "style",
                        "width",
                        "height",
                    ],
                    ALLOWED_URI_REGEXP:
                        /^(?:(?:(?:f|ht)tps?|mailto|tel|callto|cid|xmpp):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))/i,
                });

                return (
                    <div
                        dangerouslySetInnerHTML={{ __html: sanitizedHtml }}
                        style={{
                            wordWrap: "break-word",
                            overflowWrap: "break-word",
                        }}
                    />
                );

            case "text/djot":
                try {
                    // For djot, join array elements with newlines
                    const djotContent = getContentString("\n");

                    // Parse djot markdown and render to HTML
                    const djotAst = parse(djotContent);
                    const djotHtml = renderHTML(djotAst);

                    // Sanitize the rendered HTML
                    const sanitizedDjotHtml = DOMPurify.sanitize(djotHtml, {
                        ALLOWED_TAGS: [
                            "p",
                            "br",
                            "div",
                            "span",
                            "strong",
                            "b",
                            "em",
                            "i",
                            "u",
                            "s",
                            "ul",
                            "ol",
                            "li",
                            "a",
                            "img",
                            "pre",
                            "code",
                            "blockquote",
                            "h1",
                            "h2",
                            "h3",
                            "h4",
                            "h5",
                            "h6",
                            "hr",
                            "table",
                            "thead",
                            "tbody",
                            "tr",
                            "td",
                            "th",
                            "small",
                            "sup",
                            "sub",
                        ],
                        ALLOWED_ATTR: [
                            "href",
                            "src",
                            "alt",
                            "title",
                            "class",
                            "id",
                            "target",
                            "rel",
                            "style",
                            "width",
                            "height",
                        ],
                    });

                    return (
                        <div
                            className="text_djot"
                            dangerouslySetInnerHTML={{ __html: sanitizedDjotHtml }}
                            style={{
                                wordWrap: "break-word",
                                overflowWrap: "break-word",
                            }}
                        />
                    );
                } catch (error) {
                    // Fallback to plain text if djot parsing fails
                    console.warn("Failed to parse djot content:", error);
                    return (
                        <div
                            style={{
                                whiteSpace: "pre-wrap",
                                wordWrap: "break-word",
                            }}
                        >
                            {content}
                        </div>
                    );
                }

            case "text/plain":
            default:
                // For plain text, handle arrays by rendering each item as a separate line
                if (Array.isArray(content)) {
                    return (
                        <div
                            style={{
                                whiteSpace: "pre-wrap",
                                wordWrap: "break-word",
                            }}
                        >
                            {content.map((item, index) => <div key={index}>{String(item)}</div>)}
                        </div>
                    );
                }

                return (
                    <div
                        style={{
                            whiteSpace: "pre-wrap",
                            wordWrap: "break-word",
                        }}
                    >
                        {String(content)}
                    </div>
                );
        }
    }, [content, contentType]);

    return renderedContent;
};
