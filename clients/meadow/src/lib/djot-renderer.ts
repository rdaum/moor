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

import { type HTMLRenderer, parse, renderHTML, type Visitor } from "@djot/djot";
import { ANNOTATION_ID, AnnotationTable } from "@moor/web-sdk";
import { AnsiUp } from "ansi_up";
import DOMPurify from "dompurify";
import Prism from "prismjs";
import { convertEmoticons } from "../components/EmojiToggle";
import { semanticIconMarkup } from "./semantic-icons";
import { parseHttpUrl } from "./url-policy";
import "./prism-moo";

/**
 * Configuration options for djot rendering
 */
export interface DjotRenderOptions {
    annotations?: AnnotationTable;
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
    /**
     * Whether to convert emoticons to emoji (defaults to false)
     */
    enableEmoji?: boolean;
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
    "data-moor-annotation",
    "tabindex",
    "role",
    "aria-label",
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

    return parseHttpUrl(url) !== null;
}

/**
 * URL regex for detecting http/https URLs in plain text.
 * Matches URLs that are not already inside HTML tags.
 * Excludes:
 * - Whitespace
 * - HTML tag characters: < >
 * - Straight quotes: " '
 * - Curly/smart quotes: " " ' ' (U+201C, U+201D, U+2018, U+2019)
 * - Common grouping characters that URLs are often wrapped in: ) ]
 */
const PLAIN_TEXT_URL_REGEX = /https?:\/\/[^\s<>"')\]\u201C\u201D\u2018\u2019]+/g;

/**
 * Cleans up a detected URL by removing trailing punctuation that's likely
 * sentence-ending rather than part of the URL.
 */
function cleanupDetectedUrl(url: string): string {
    // Remove trailing punctuation that's usually not part of URLs
    return url.replace(/[.,;:!?]+$/, "");
}

/**
 * Converts plain text URLs to clickable span elements.
 * Processes text nodes in HTML, leaving existing tags and links untouched.
 */
function convertPlainTextUrls(html: string): string {
    const tempDiv = document.createElement("div");
    tempDiv.innerHTML = html;

    processTextNodesForUrls(tempDiv);

    return tempDiv.innerHTML;
}

/**
 * Recursively processes text nodes to detect and wrap URLs.
 */
function processTextNodesForUrls(node: Node): void {
    if (node.nodeType === Node.TEXT_NODE) {
        const text = node.textContent || "";
        // Reset regex state for each text node
        PLAIN_TEXT_URL_REGEX.lastIndex = 0;

        if (PLAIN_TEXT_URL_REGEX.test(text)) {
            // Reset regex state after test
            PLAIN_TEXT_URL_REGEX.lastIndex = 0;

            const fragment = document.createDocumentFragment();
            let lastIndex = 0;
            let match;

            while ((match = PLAIN_TEXT_URL_REGEX.exec(text)) !== null) {
                // Add text before the URL
                if (match.index > lastIndex) {
                    fragment.appendChild(
                        document.createTextNode(text.slice(lastIndex, match.index)),
                    );
                }

                const rawUrl = match[0];
                const url = cleanupDetectedUrl(rawUrl);
                const trailingPunctuation = rawUrl.slice(url.length);

                if (isSafeUrl(url)) {
                    // Create clickable span with external link styling
                    const span = document.createElement("span");
                    span.className = "moo-link-external moo-link-detected";
                    span.setAttribute("data-url", url);
                    span.setAttribute("tabindex", "0");
                    span.setAttribute("role", "link");
                    span.title = url;
                    span.textContent = url;
                    fragment.appendChild(span);

                    // Add any trailing punctuation as plain text
                    if (trailingPunctuation) {
                        fragment.appendChild(document.createTextNode(trailingPunctuation));
                    }
                } else {
                    // Keep as plain text if not safe
                    fragment.appendChild(document.createTextNode(rawUrl));
                }

                lastIndex = match.index + rawUrl.length;
            }

            // Add remaining text after the last URL
            if (lastIndex < text.length) {
                fragment.appendChild(document.createTextNode(text.slice(lastIndex)));
            }

            node.parentNode?.replaceChild(fragment, node);
        }
    } else if (node.nodeType === Node.ELEMENT_NODE) {
        const element = node as Element;
        // Skip elements that already have data-url (already a link)
        // Skip pre/code elements to avoid breaking code blocks
        // Skip anchor tags
        if (
            element.hasAttribute("data-moor-annotation")
            || element.hasAttribute("data-url")
            || element.tagName === "PRE"
            || element.tagName === "CODE"
            || element.tagName === "A"
        ) {
            return;
        }
        // Process child nodes (copy to array since we may modify the DOM)
        Array.from(node.childNodes).forEach(child => processTextNodesForUrls(child));
    }
}

/**
 * Determines the CSS class for a link based on its URL scheme.
 * Returns the appropriate moo-link-* class for styling.
 */
export function getLinkClass(_url: string): string {
    return "moo-link-external";
}

/**
 * Escapes underscores in identifier-like contexts to prevent unwanted emphasis.
 * Matches patterns like: $obj.property_name, variable_name, func_call(), etc.
 * This prevents underscores within identifiers from being interpreted as italic markers.
 *
 * IMPORTANT: This function preserves content within backticks (inline code) and code blocks,
 * as those should not have their underscores escaped.
 */
function escapeIdentifierUnderscores(content: string): string {
    // Split content by code blocks and inline code to preserve them
    const parts: Array<{ text: string; isCode: boolean }> = [];
    let currentPos = 0;

    // Match both inline code (`...`) and code blocks (```...```)
    // We need to handle these carefully to not escape underscores inside them
    const codeRegex = /(`+)([^`]+?)\1/g;
    let match;

    while ((match = codeRegex.exec(content)) !== null) {
        // Add text before the code
        if (match.index > currentPos) {
            parts.push({ text: content.slice(currentPos, match.index), isCode: false });
        }
        // Add the code itself (including backticks)
        parts.push({ text: match[0], isCode: true });
        currentPos = match.index + match[0].length;
    }

    // Add remaining text after the last code block
    if (currentPos < content.length) {
        parts.push({ text: content.slice(currentPos), isCode: false });
    }

    // Process only non-code parts
    return parts.map(part => {
        if (part.isCode) {
            return part.text;
        }

        return part.text.replace(
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
    }).join("");
}

/**
 * Processes ANSI escape codes and emoji conversion in text nodes recursively
 * @param node - The DOM node to process
 * @param ansi_up - The AnsiUp instance for ANSI code conversion
 * @param enableEmoji - Whether to convert emoticons to emoji (defaults to false)
 */
function processTextNodesForAnsi(node: Node, ansi_up: AnsiUp, enableEmoji: boolean = false): void {
    if (node.nodeType === Node.TEXT_NODE) {
        let text = node.textContent || "";

        // Apply emoji conversion if enabled for this content
        if (enableEmoji) {
            text = convertEmoticons(text);
        }

        if (text.includes("\x1b[") || (enableEmoji && text !== node.textContent)) {
            const ansiHtml = ansi_up.ansi_to_html(text);
            const span = document.createElement("span");
            span.innerHTML = ansiHtml;
            node.parentNode?.replaceChild(span, node);
        }
    } else if (node.nodeType === Node.ELEMENT_NODE) {
        const element = node as Element;
        if (element.tagName !== "PRE" && element.tagName !== "CODE") {
            Array.from(node.childNodes).forEach(child => processTextNodesForAnsi(child, ansi_up, enableEmoji));
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
 * Converts anchor tags to styled spans and adds table classes in HTML content
 */
function convertLinksAndTables(container: HTMLElement): void {
    // Convert all <a> tags to clickable spans with appropriate styling
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

        // Create span to replace the link with URL-based styling
        // tabindex="0" + role="link" makes it keyboard-focusable and recognized by screen readers
        const span = document.createElement("span");
        span.className = getLinkClass(href);
        span.setAttribute("data-url", href);
        span.setAttribute("tabindex", "0");
        span.setAttribute("role", "link");
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
 * Processes HTML content - handles ANSI escape codes in text, syntax highlighting in code blocks,
 * and detects plain text URLs to make them clickable.
 * This is the final rendering step for all content types
 * @param html - The HTML string to process
 * @param enableEmoji - Whether to convert emoticons to emoji (defaults to false)
 */
export function processHtmlContent(html: string, enableEmoji: boolean = false): string {
    const tempDiv = document.createElement("div");
    tempDiv.innerHTML = html;
    const ansi_up = new AnsiUp();

    processCodeBlocks(tempDiv, ansi_up);
    processTextNodesForAnsi(tempDiv, ansi_up, enableEmoji);

    // Detect plain text URLs in text nodes and convert to clickable spans
    processTextNodesForUrls(tempDiv);

    return tempDiv.innerHTML;
}

/**
 * Renders HTML content with link/table conversion, ANSI codes, and syntax highlighting
 * This is used for text/html content type
 * @param html - The HTML string to render
 * @param enableEmoji - Whether to convert emoticons to emoji (defaults to false)
 */
export function renderHtmlContent(html: string, enableEmoji: boolean = false, annotations?: AnnotationTable): string {
    // Sanitize before adding application-owned interaction attributes.
    const sanitizedHtml = DOMPurify.sanitize(html, {
        ALLOWED_TAGS: CONTENT_ALLOWED_TAGS,
        ALLOWED_ATTR: CONTENT_ALLOWED_ATTR,
        ALLOWED_URI_REGEXP: /^(https?):/i,
    });

    // Convert links and tables
    const tempDiv = document.createElement("div");
    tempDiv.innerHTML = sanitizedHtml;
    convertLinksAndTables(tempDiv);

    // Detect plain text URLs in text nodes and convert to clickable spans
    processTextNodesForUrls(tempDiv);

    // Process ANSI and syntax highlighting
    const ansi_up = new AnsiUp();
    processCodeBlocks(tempDiv, ansi_up);
    processTextNodesForAnsi(tempDiv, ansi_up, enableEmoji);

    return bindAnnotationMarkers(tempDiv.innerHTML, annotations);
}

/**
 * Renders plain text to HTML, converting ANSI escape codes and detecting URLs
 * @param text - The plain text to render
 * @param enableEmoji - Whether to convert emoticons to emoji (defaults to false)
 */
export function renderPlainText(text: string, enableEmoji: boolean = false): string {
    // Apply emoji conversion if enabled for this content
    const processedText = enableEmoji ? convertEmoticons(text) : text;

    const ansi_up = new AnsiUp();
    const htmlFromAnsi = ansi_up.ansi_to_html(processedText);

    // Convert plain text URLs to clickable links
    const withUrls = convertPlainTextUrls(htmlFromAnsi);

    // Sanitize with minimal allowed tags for plain text
    // Include attributes needed for clickable URL spans
    const sanitizedHtml = DOMPurify.sanitize(withUrls, {
        ALLOWED_TAGS: ["span", "div", "br"],
        ALLOWED_ATTR: [
            "style",
            "class",
            "data-url",
            "tabindex",
            "title",
            "role",
            "aria-label",
        ],
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
        enableEmoji = false,
    } = options;

    // Escape underscores in identifier-like contexts
    const escapedContent = escapeIdentifierUnderscores(content);

    // Parse djot and render to HTML
    const djotAst = parse(escapedContent);
    const overrides: Visitor<HTMLRenderer, string> = {};

    // Override doc element to not wrap in a div (prevents screenreader "group" announcement)
    overrides.doc = (node, context) => {
        return context.renderChildren(node);
    };

    overrides.span = (node, context) => {
        const id = node.attributes?.annotation;
        const children = context.renderChildren(node);
        return typeof id === "string" && ANNOTATION_ID.test(id)
            ? `<span data-moor-annotation="${id}">${children}</span>`
            : `<span>${children}</span>`;
    };

    // Link handling
    if (linkHandler) {
        overrides.link = (node, context) => {
            const href = node.destination || "";
            const label = context.renderChildren(node);
            if (!isSafeUrl(href)) return label;
            const element = document.createElement("span");
            element.className = getLinkClass(href);
            element.setAttribute(linkHandler.dataAttribute, href);
            element.title = href;
            element.tabIndex = 0;
            element.setAttribute("role", "link");
            if (label) element.innerHTML = label;
            else element.textContent = href;
            return element.outerHTML;
        };
    }

    // Table handling
    if (addTableClass) {
        overrides.table = (node, context) => {
            return `<table class="narrative-table">${context.renderChildren(node)}</table>`;
        };
    }

    const djotHtml = renderHTML(djotAst, { overrides });

    // Sanitize HTML
    const sanitizedHtml = DOMPurify.sanitize(djotHtml, {
        ALLOWED_TAGS: [...CONTENT_ALLOWED_TAGS, ...additionalAllowedTags],
        ALLOWED_ATTR: CONTENT_ALLOWED_ATTR,
    });

    // Process for ANSI codes, syntax highlighting, and URL detection
    return bindAnnotationMarkers(processHtmlContent(sanitizedHtml, enableEmoji), options.annotations);
}

/** Resolve anchors only against the enclosing event; unknown metadata stays inert. */
export function bindAnnotationMarkers(html: string, annotations?: AnnotationTable): string {
    const container = document.createElement("div");
    container.innerHTML = html;
    for (const element of container.querySelectorAll<HTMLElement>("[data-moor-annotation]")) {
        const id = element.dataset.moorAnnotation ?? "";
        const annotation = annotations && Object.prototype.hasOwnProperty.call(annotations, id)
            ? annotations[id]
            : undefined;
        if (
            !annotation || element.closest("[data-url], a") || element.parentElement?.closest("[data-moor-annotation]")
        ) {
            element.removeAttribute("data-moor-annotation");
            element.removeAttribute("tabindex");
            element.removeAttribute("role");
            continue;
        }
        element.classList.add("semantic-annotation");
        element.tabIndex = 0;
        element.setAttribute("role", "button");
        const action = {
            object: "Inspect",
            command: "Review command",
            help: "Read help",
            verb: "Browse verb",
            property: "Browse property",
            change: "Review changes",
        }[annotation.kind];
        const sourceLink = annotation.kind === "verb" && element.textContent === "↗";
        const label = sourceLink ? `${annotation.receiver}:${annotation.name}` : element.textContent;
        if (sourceLink) element.classList.add("semantic-source-link");
        element.title = sourceLink ? `${action}: ${label}` : action;
        element.setAttribute("aria-label", `${action}: ${label}`);
        // Prose stays undecorated; reference lists identify people.
        const icon = annotation.kind === "object"
            ? (annotation.objectKind === "person" && element.closest(".reference-columns") ? "person" : undefined)
            : annotation.kind === "command"
            ? (annotation.action?.icon ?? (element.closest(".reference-columns") ? "command" : undefined))
            : annotation.kind === "help"
            ? "help"
            : undefined;
        if (annotation.kind === "object" && annotation.objectKind === "person") {
            element.classList.add("semantic-person");
        }
        element.insertAdjacentHTML("afterbegin", semanticIconMarkup(icon));
    }
    return container.innerHTML;
}
