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

/**
 * Converts content to screen-reader-friendly plain text.
 *
 * This function processes various content types (plain text, HTML, djot) and
 * converts them to text suitable for screen reader announcement, stripping
 * formatting but preserving semantic meaning.
 *
 * @param content - The content to convert (string or array of strings)
 * @param contentType - The type of content
 * @param accessibilityText - Optional pre-computed accessibility text from server metadata
 * @returns Plain text suitable for screen reader announcement
 */
export function toScreenReaderText(
    content: string | string[],
    contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback",
    accessibilityText?: string,
): string {
    // If the MOO provided accessibility text, use it directly
    if (accessibilityText) {
        return accessibilityText;
    }

    // Convert content to string
    const contentStr = Array.isArray(content) ? content.join("\n") : content;

    // Strip ANSI escape codes
    const withoutAnsi = stripAnsiCodes(contentStr);

    // Convert based on content type
    switch (contentType) {
        case "text/html":
            return htmlToScreenReaderText(withoutAnsi);
        case "text/djot":
            return djotToScreenReaderText(withoutAnsi);
        case "text/traceback":
            return `Error traceback: ${withoutAnsi}`;
        case "text/plain":
        default:
            return withoutAnsi;
    }
}

/**
 * Strips ANSI escape codes from text
 */
function stripAnsiCodes(text: string): string {
    // ANSI escape code pattern
    // eslint-disable-next-line no-control-regex
    return text.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, "");
}

/**
 * Converts HTML to screen-reader-friendly text.
 * Extracts text content and adds semantic hints for important elements.
 */
function htmlToScreenReaderText(html: string): string {
    // Create a temporary DOM element to parse HTML
    const temp = document.createElement("div");
    temp.innerHTML = html;

    return extractTextWithSemantics(temp);
}

/**
 * Converts djot to screen-reader-friendly text.
 * Similar to HTML conversion but handles djot-specific constructs.
 */
function djotToScreenReaderText(djot: string): string {
    // For now, treat djot as plain text with some markdown-like semantics
    // We could enhance this to parse djot more specifically if needed

    let text = djot;

    // Convert headings: # Heading -> "Heading"
    text = text.replace(/^#{1,6}\s+(.+)$/gm, "$1");

    // Convert links: [text](url) -> "text, link"
    text = text.replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1, link");

    // Convert inline code: `code` -> "code"
    text = text.replace(/`([^`]+)`/g, "$1");

    // Convert bold/italic: *text* or _text_ -> "text"
    text = text.replace(/[*_]([^*_]+)[*_]/g, "$1");

    return text.trim();
}

/**
 * Recursively extracts text from DOM elements with semantic hints
 */
function extractTextWithSemantics(element: Element): string {
    const parts: string[] = [];

    for (const node of element.childNodes) {
        if (node.nodeType === Node.TEXT_NODE) {
            const text = node.textContent?.trim();
            if (text) {
                parts.push(text);
            }
        } else if (node.nodeType === Node.ELEMENT_NODE) {
            const el = node as Element;
            const tagName = el.tagName.toLowerCase();

            switch (tagName) {
                case "h1":
                case "h2":
                case "h3":
                case "h4":
                case "h5":
                case "h6":
                    {
                        const text = extractTextWithSemantics(el);
                        if (text) {
                            parts.push(`Heading: ${text}`);
                        }
                    }
                    break;

                case "a":
                    {
                        const text = extractTextWithSemantics(el);
                        const href = el.getAttribute("href");
                        if (text) {
                            parts.push(href ? `${text}, link to ${href}` : `${text}, link`);
                        }
                    }
                    break;

                case "code":
                case "pre":
                    {
                        const text = extractTextWithSemantics(el);
                        if (text) {
                            parts.push(`code: ${text}`);
                        }
                    }
                    break;

                case "br":
                    parts.push("\n");
                    break;

                case "p":
                case "div":
                case "span":
                default:
                    {
                        const text = extractTextWithSemantics(el);
                        if (text) {
                            parts.push(text);
                        }
                    }
                    break;
            }
        }
    }

    return parts.join(" ").replace(/\s+/g, " ").trim();
}
