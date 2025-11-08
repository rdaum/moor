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
import { AnsiUp } from "ansi_up";
import DOMPurify from "dompurify";
import Prism from "prismjs";
import React, { useMemo } from "react";
import "../lib/prism-moo";

/**
 * Validates a URL to prevent XSS attacks.
 * Only allows safe protocols and blocks javascript: data: and other dangerous schemes.
 */
function isSafeUrl(url: string): boolean {
    if (!url || typeof url !== "string") {
        return false;
    }

    // Trim whitespace
    url = url.trim();

    // Block empty URLs
    if (!url) {
        return false;
    }

    // Block javascript: protocol
    if (url.toLowerCase().startsWith("javascript:")) {
        return false;
    }

    // Block data: URIs
    if (url.toLowerCase().startsWith("data:")) {
        return false;
    }

    // Block vbscript: protocol
    if (url.toLowerCase().startsWith("vbscript:")) {
        return false;
    }

    // Block file: protocol
    if (url.toLowerCase().startsWith("file:")) {
        return false;
    }

    // Allow relative URLs (starting with /, ./, ../, #, or ?)
    if (/^[/#?]|^\.\.?\//.test(url)) {
        return true;
    }

    // Allow http and https protocols only
    try {
        const urlObj = new URL(url);
        return urlObj.protocol === "http:" || urlObj.protocol === "https:";
    } catch {
        // If URL parsing fails, it's likely malformed - block it
        return false;
    }
}

interface ContentRendererProps {
    content: string | string[];
    contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback";
    onLinkClick?: (url: string) => void;
}

export const ContentRenderer: React.FC<ContentRendererProps> = ({
    content,
    contentType = "text/plain",
    onLinkClick,
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
            case "text/html": {
                // For HTML, join array elements with newlines
                const htmlContent = getContentString("\n");
                // Add hook to DOMPurify
                DOMPurify.addHook("afterSanitizeElements", function(node) {
                    const element = node as Element;
                    if (element.tagName === "TABLE") {
                        const existingClass = element.getAttribute("class") || "";
                        const newClass = existingClass ? `${existingClass} narrative-table` : "narrative-table";
                        element.setAttribute("class", newClass);
                    } else if (element.tagName === "A") {
                        // Convert links to moo-link spans (same as djot does)
                        const href = element.getAttribute("href") || "";
                        const linkText = element.textContent || href;

                        // Validate URL before using it
                        if (!isSafeUrl(href)) {
                            // Replace unsafe links with plain text
                            const textNode = document.createTextNode(linkText);
                            element.parentNode?.replaceChild(textNode, element);
                            return;
                        }

                        // Create a span element to replace the link
                        const span = document.createElement("span");
                        span.className = "moo-link";
                        span.setAttribute("data-url", href);
                        span.style.color = "var(--color-link)";
                        span.style.textDecoration = "underline";
                        span.style.cursor = "pointer";
                        span.title = href;
                        span.textContent = linkText;

                        // Replace the link with the span
                        element.parentNode?.replaceChild(span, element);
                    }
                });

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
                        "dl",
                        "dt",
                        "dd",
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
                        "data-url",
                    ],
                    ALLOWED_URI_REGEXP: /^(https?|mailto|tel|callto|cid|xmpp):/i,
                });

                // Remove the hook after use to avoid affecting other calls
                DOMPurify.removeHook("afterSanitizeElements");

                // Process <pre> blocks: ANSI codes and syntax highlighting
                const tempDiv = document.createElement("div");
                tempDiv.innerHTML = sanitizedHtml;
                const preElements = tempDiv.querySelectorAll("pre");

                if (preElements.length > 0) {
                    const ansi_up = new AnsiUp();
                    preElements.forEach((pre) => {
                        const codeElement = pre.querySelector("code");

                        // Check if this is a code block with language-* class
                        if (codeElement) {
                            const classes = codeElement.className.split(/\s+/);
                            const languageClass = classes.find(cls => cls.startsWith("language-"));

                            if (languageClass) {
                                const language = languageClass.replace("language-", "");

                                // Apply Prism syntax highlighting for supported languages
                                if (Prism.languages[language]) {
                                    const code = codeElement.textContent || "";
                                    const highlightedHtml = Prism.highlight(
                                        code,
                                        Prism.languages[language],
                                        language,
                                    );
                                    codeElement.innerHTML = highlightedHtml;
                                    codeElement.classList.add(`language-${language}`);
                                }
                                return; // Skip ANSI processing for syntax-highlighted code
                            }
                        }

                        // ANSI code processing for non-syntax-highlighted blocks
                        const text = pre.textContent || "";
                        if (text.includes("\x1b[")) {
                            // Convert ANSI to HTML, then newlines to <br> for innerHTML
                            const ansiHtml = ansi_up.ansi_to_html(text).replace(/\n/g, "<br>");
                            pre.innerHTML = ansiHtml;
                        }
                    });
                }

                const processedHtml = tempDiv.innerHTML;

                return (
                    <div
                        dangerouslySetInnerHTML={{ __html: processedHtml }}
                        onClick={(e) => {
                            // Handle clicks on moo-link spans
                            const target = e.target as HTMLElement;
                            if (target.classList.contains("moo-link")) {
                                e.preventDefault();
                                const url = target.getAttribute("data-url");
                                if (url && onLinkClick) {
                                    onLinkClick(url);
                                }
                            }
                        }}
                        className="content-html"
                    />
                );
            }

            case "text/djot": {
                try {
                    // For djot, join array elements with newlines
                    const djotContent = getContentString("\n");

                    // Parse djot markdown and render to HTML
                    const djotAst = parse(djotContent);
                    const djotHtml = renderHTML(djotAst, {
                        overrides: {
                            link: (node: any, _context: any) => {
                                const href = node.destination || "";

                                // Extract link text from djot AST
                                let linkText = "";
                                if (node.children && node.children.length > 0) {
                                    linkText = node.children.map((child: any) => {
                                        if (child.tag === "str") {
                                            return child.text || "";
                                        }
                                        // Handle other node types if needed
                                        return "";
                                    }).join("");
                                }

                                // Only fall back to URL if we truly have no text
                                if (!linkText.trim()) {
                                    linkText = href;
                                }

                                // Validate URL before creating the link
                                if (!isSafeUrl(href)) {
                                    // Return plain text for unsafe URLs
                                    return linkText;
                                }

                                // Convert ALL links to moo-link spans that will call #0:handle_client_url
                                return `<span class="moo-link" data-url="${href}" style="color: var(--color-link); text-decoration: underline; cursor: pointer;" title="${href}">${linkText}</span>`;
                            },
                            table: (node: any, context: any) => {
                                return `<table class="narrative-table">${context.renderChildren(node)}</table>`;
                            },
                            thead: (node: any, context: any) => {
                                return `<thead>${context.renderChildren(node)}</thead>`;
                            },
                            tbody: (node: any, context: any) => {
                                return `<tbody>${context.renderChildren(node)}</tbody>`;
                            },
                            tr: (node: any, context: any) => {
                                return `<tr>${context.renderChildren(node)}</tr>`;
                            },
                            th: (node: any, context: any) => {
                                return `<th>${context.renderChildren(node)}</th>`;
                            },
                            td: (node: any, context: any) => {
                                return `<td>${context.renderChildren(node)}</td>`;
                            },
                        } as any,
                    });

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
                            "data-url",
                        ],
                    });

                    return (
                        <div
                            className="text_djot content-html"
                            dangerouslySetInnerHTML={{ __html: sanitizedDjotHtml }}
                            onClick={(e) => {
                                // Handle clicks on moo-link spans
                                const target = e.target as HTMLElement;
                                if (target.classList.contains("moo-link")) {
                                    e.preventDefault();
                                    const url = target.getAttribute("data-url");
                                    if (url && onLinkClick) {
                                        onLinkClick(url);
                                    }
                                }
                            }}
                        />
                    );
                } catch (error) {
                    // Fallback to plain text if djot parsing fails
                    console.warn("Failed to parse djot content:", error);
                    return (
                        <div className="content-text">
                            {content}
                        </div>
                    );
                }
            }

            case "text/traceback": {
                // For traceback, render as plain text with special traceback styling
                const tracebackContent = getContentString("\n");
                return (
                    <pre className="traceback_narrative">
                        {tracebackContent}
                    </pre>
                );
            }

            case "text/plain":
            default: {
                // For plain text, convert ANSI codes to HTML and render safely
                const plainContent = getContentString("\n");

                // Convert ANSI escape codes to HTML using ansi_up
                const ansi_up = new AnsiUp();
                const htmlFromAnsi = ansi_up.ansi_to_html(plainContent);

                // Sanitize the HTML output from ANSI conversion
                const sanitizedAnsiHtml = DOMPurify.sanitize(htmlFromAnsi, {
                    ALLOWED_TAGS: ["span", "div", "br"],
                    ALLOWED_ATTR: ["style", "class"],
                });

                return (
                    <div
                        dangerouslySetInnerHTML={{ __html: sanitizedAnsiHtml }}
                        className="content-text"
                    />
                );
            }
        }
    }, [content, contentType]);

    return renderedContent;
};
