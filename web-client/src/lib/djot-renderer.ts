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
import "./prism-moo";

/**
 * Configuration options for djot rendering
 */
export interface DjotRenderOptions {
    /**
     * Custom link handler - if provided, links will be rendered as spans with this data attribute
     * If not provided, regular anchor tags will be used
     */
    linkHandler?: {
        className: string;
        dataAttribute: string;
    };
    /**
     * Whether to add special table styling class
     */
    addTableClass?: boolean;
    /**
     * Additional allowed HTML tags for sanitization
     */
    additionalAllowedTags?: string[];
}

/**
 * Standard allowed HTML tags for content rendering
 */
export const CONTENT_ALLOWED_TAGS = [
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
];

/**
 * Standard allowed HTML attributes for content rendering
 */
export const CONTENT_ALLOWED_ATTR = [
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
];

/**
 * Validates a URL to prevent XSS attacks.
 * Only allows safe protocols and blocks javascript: data: and other dangerous schemes.
 */
export function isSafeUrl(url: string): boolean {
    if (!url || typeof url !== "string") {
        return false;
    }

    url = url.trim();

    if (!url) {
        return false;
    }

    if (url.toLowerCase().startsWith("javascript:")) {
        return false;
    }

    if (url.toLowerCase().startsWith("data:")) {
        return false;
    }

    if (url.toLowerCase().startsWith("vbscript:")) {
        return false;
    }

    if (url.toLowerCase().startsWith("file:")) {
        return false;
    }

    if (/^[/#?]|^\.\.?\//.test(url)) {
        return true;
    }

    try {
        const urlObj = new URL(url);
        return urlObj.protocol === "http:" || urlObj.protocol === "https:";
    } catch {
        return false;
    }
}

/**
 * Escapes underscores in identifier-like contexts to prevent unwanted emphasis.
 * Matches patterns like: $obj.property_name, variable_name, func_call(), etc.
 * This prevents underscores within identifiers from being interpreted as italic markers.
 */
function escapeIdentifierUnderscores(content: string): string {
    return content.replace(
        /(\$?[a-zA-Z0-9_.]+_[a-zA-Z0-9_.]*)/g,
        (match) => {
            // Only escape if this looks like an identifier (contains word chars around the underscore)
            // and doesn't have spaces (which would indicate intentional formatting)
            if (!/\s/.test(match) && /[a-zA-Z0-9]_[a-zA-Z0-9]/.test(match)) {
                return match.replace(/_/g, "\\_");
            }
            return match;
        },
    );
}

/**
 * Processes ANSI escape codes in text nodes recursively
 */
function processTextNodesForAnsi(node: Node, ansi_up: AnsiUp): void {
    if (node.nodeType === Node.TEXT_NODE) {
        const text = node.textContent || "";
        if (text.includes("\x1b[")) {
            const ansiHtml = ansi_up.ansi_to_html(text);
            const span = document.createElement("span");
            span.innerHTML = ansiHtml;
            node.parentNode?.replaceChild(span, node);
        }
    } else if (node.nodeType === Node.ELEMENT_NODE) {
        const element = node as Element;
        if (element.tagName !== "PRE" && element.tagName !== "CODE") {
            Array.from(node.childNodes).forEach(child => processTextNodesForAnsi(child, ansi_up));
        }
    }
}

/**
 * Processes code blocks for syntax highlighting and ANSI codes
 */
function processCodeBlocks(container: HTMLElement, ansi_up: AnsiUp): void {
    const preElements = container.querySelectorAll("pre");
    preElements.forEach((pre) => {
        const codeElement = pre.querySelector("code");

        if (codeElement) {
            const classes = codeElement.className.split(/\s+/);
            const languageClass = classes.find(cls => cls.startsWith("language-"));

            if (languageClass) {
                const language = languageClass.replace("language-", "");

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
                return;
            }
        }

        // ANSI code processing for non-syntax-highlighted pre blocks
        const text = pre.textContent || "";
        if (text.includes("\x1b[")) {
            const ansiHtml = ansi_up.ansi_to_html(text).replace(/\n/g, "<br>");
            pre.innerHTML = ansiHtml;
        }
    });
}

/**
 * Converts anchor tags to moo-link spans and adds table classes in HTML content
 */
function convertLinksAndTables(container: HTMLElement): void {
    // Convert all <a> tags to moo-link spans
    const links = container.querySelectorAll("a");
    links.forEach(link => {
        const href = link.getAttribute("href") || "";
        const linkText = link.textContent || href;

        if (!isSafeUrl(href)) {
            // Replace unsafe links with plain text
            const textNode = document.createTextNode(linkText);
            link.parentNode?.replaceChild(textNode, link);
            return;
        }

        // Create span to replace the link
        const span = document.createElement("span");
        span.className = "moo-link";
        span.setAttribute("data-url", href);
        span.style.color = "var(--color-link)";
        span.style.textDecoration = "underline";
        span.style.cursor = "pointer";
        span.title = href;
        span.textContent = linkText;

        link.parentNode?.replaceChild(span, link);
    });

    // Add narrative-table class to all tables
    const tables = container.querySelectorAll("table");
    tables.forEach(table => {
        const existingClass = table.getAttribute("class") || "";
        const newClass = existingClass ? `${existingClass} narrative-table` : "narrative-table";
        table.setAttribute("class", newClass);
    });
}

/**
 * Processes HTML content - handles ANSI escape codes in text and syntax highlighting in code blocks
 * This is the final rendering step for all content types
 */
export function processHtmlContent(html: string): string {
    const tempDiv = document.createElement("div");
    tempDiv.innerHTML = html;
    const ansi_up = new AnsiUp();

    processCodeBlocks(tempDiv, ansi_up);
    processTextNodesForAnsi(tempDiv, ansi_up);

    return tempDiv.innerHTML;
}

/**
 * Renders HTML content with link/table conversion, ANSI codes, and syntax highlighting
 * This is used for text/html content type
 */
export function renderHtmlContent(html: string): string {
    // First sanitize
    const sanitizedHtml = DOMPurify.sanitize(html, {
        ALLOWED_TAGS: CONTENT_ALLOWED_TAGS,
        ALLOWED_ATTR: CONTENT_ALLOWED_ATTR,
        ALLOWED_URI_REGEXP: /^(https?|mailto|tel|callto|cid|xmpp):/i,
    });

    // Convert links and tables
    const tempDiv = document.createElement("div");
    tempDiv.innerHTML = sanitizedHtml;
    convertLinksAndTables(tempDiv);

    // Process ANSI and syntax highlighting
    const ansi_up = new AnsiUp();
    processCodeBlocks(tempDiv, ansi_up);
    processTextNodesForAnsi(tempDiv, ansi_up);

    return tempDiv.innerHTML;
}

/**
 * Renders plain text to HTML, converting ANSI escape codes
 */
export function renderPlainText(text: string): string {
    const ansi_up = new AnsiUp();
    const htmlFromAnsi = ansi_up.ansi_to_html(text);

    // Sanitize with minimal allowed tags for plain text
    const sanitizedHtml = DOMPurify.sanitize(htmlFromAnsi, {
        ALLOWED_TAGS: ["span", "div", "br"],
        ALLOWED_ATTR: ["style", "class"],
    });

    return sanitizedHtml;
}

/**
 * Renders djot content to HTML with ANSI code support and syntax highlighting
 */
export function renderDjot(content: string, options: DjotRenderOptions = {}): string {
    const {
        linkHandler,
        addTableClass = false,
        additionalAllowedTags = [],
    } = options;

    // Escape underscores in identifier-like contexts
    const escapedContent = escapeIdentifierUnderscores(content);

    // Parse djot and render to HTML
    const djotAst = parse(escapedContent);
    const overrides: any = {};

    // Link handling
    if (linkHandler) {
        overrides.link = (node: any, _context: any) => {
            const href = node.destination || "";

            let linkText = "";
            if (node.children && node.children.length > 0) {
                linkText = node.children.map((child: any) => {
                    if (child.tag === "str") {
                        return child.text || "";
                    }
                    return "";
                }).join("");
            }

            if (!linkText.trim()) {
                linkText = href;
            }

            if (!isSafeUrl(href)) {
                return linkText;
            }

            return `<span class="${linkHandler.className}" ${linkHandler.dataAttribute}="${href}" style="color: var(--color-link); text-decoration: underline; cursor: pointer;" title="${href}">${linkText}</span>`;
        };
    }

    // Table handling
    if (addTableClass) {
        overrides.table = (node: any, context: any) => {
            return `<table class="narrative-table">${context.renderChildren(node)}</table>`;
        };
        overrides.thead = (node: any, context: any) => {
            return `<thead>${context.renderChildren(node)}</thead>`;
        };
        overrides.tbody = (node: any, context: any) => {
            return `<tbody>${context.renderChildren(node)}</tbody>`;
        };
        overrides.tr = (node: any, context: any) => {
            return `<tr>${context.renderChildren(node)}</tr>`;
        };
        overrides.th = (node: any, context: any) => {
            return `<th>${context.renderChildren(node)}</th>`;
        };
        overrides.td = (node: any, context: any) => {
            return `<td>${context.renderChildren(node)}</td>`;
        };
    }

    const djotHtml = renderHTML(djotAst, { overrides });

    // Sanitize HTML
    const sanitizedHtml = DOMPurify.sanitize(djotHtml, {
        ALLOWED_TAGS: [...CONTENT_ALLOWED_TAGS, ...additionalAllowedTags],
        ALLOWED_ATTR: CONTENT_ALLOWED_ATTR,
    });

    // Process for ANSI codes and syntax highlighting
    return processHtmlContent(sanitizedHtml);
}
