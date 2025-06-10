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

/* Narrative Module
 *
 * This module handles the core UI for the MOO client, including:
 * - Processing and rendering narrative content from the server
 * - Managing the input area and command history
 * - Handling different content types (text/plain, text/html, text/djot)
 * - Supporting MCP-like editing protocols
 * - Managing presentations (windows, panels) in the UI
 */

import DOMPurify from "dompurify";
import { FloatingWindow } from "van-ui";
import van, { State } from "vanjs-core";

import {
    BaseEvent,
    Context,
    HistoricalEvent,
    HistoryResponse,
    NarrativeEvent,
    Player,
    Presentation,
    PresentationData,
    PresentationManager,
    PresentationModel,
    Spool,
    SpoolType,
    SystemEvent,
    Traceback,
} from "./model";
import { curieORef, MoorRemoteObject } from "./rpc";
import { matchRef } from "./var";
import { launchVerbEditor, showVerbEditor } from "./verb_edit";

// Extract common VanJS elements for better readability
const { div, span, textarea, button } = van.tags;

/**
 * Fetches and displays current presentations from the server
 * This is called at login to restore presentation state
 *
 * @param context - Application context containing auth token and presentation manager
 * @returns Promise<boolean> - true if presentations were successfully loaded
 */
export async function fetchAndDisplayCurrentPresentations(context: Context): Promise<boolean> {
    if (!context.authToken) {
        console.warn("Cannot fetch presentations: no auth token available");
        return false;
    }

    try {
        console.log("Fetching current presentations...");

        const response = await fetch("/api/presentations", {
            headers: {
                "X-Moor-Auth-Token": context.authToken,
            },
        });

        if (!response.ok) {
            console.error(`Failed to fetch presentations: ${response.status} ${response.statusText}`);
            return false;
        }

        const data = await response.json();
        const presentations = data.presentations || [];

        console.log(`Loaded ${presentations.length} current presentations`);

        // Display each presentation
        for (const presentation of presentations) {
            try {
                handlePresent(context, presentation);
            } catch (error) {
                console.error(`Error displaying presentation ${presentation.id}:`, error);
            }
        }

        return true;
    } catch (error) {
        console.error("Error fetching presentations:", error);
        return false;
    }
}

export async function fetchAndDisplayHistory(
    context: Context,
    maxEvents: number = 1000,
    sinceEvent?: string,
): Promise<HistoryResponse | null> {
    if (!context.authToken) {
        console.warn("Cannot fetch history: no auth token available");
        return null;
    }

    try {
        // Set up query parameters - prefer N lines over time-based for initial load
        const params = new URLSearchParams({
            limit: maxEvents.toString(),
        });

        if (sinceEvent) {
            // For pagination, fetch events BEFORE the earliest event we have
            params.append("until_event", sinceEvent);
        } else {
            // Initial load: get recent history with a reasonable time window
            // Use a longer time window (24 hours) to ensure we get content
            params.append("since_seconds", "86400"); // Last 24 hours
        }

        console.log(
            `Fetching history${sinceEvent ? " (pagination)" : " (initial)"}: limit=${maxEvents}${
                sinceEvent ? `, until=${sinceEvent}` : ""
            }`,
        );

        // Set the timestamp boundary for deduplication only on initial load
        if (!sinceEvent) {
            context.setHistoryBoundary();
        }

        const response = await fetch(`/api/history?${params}`, {
            method: "GET",
            headers: {
                "X-Moor-Auth-Token": context.authToken,
                "Content-Type": "application/json",
            },
        });

        if (!response.ok) {
            console.error(`Failed to fetch history: ${response.status} ${response.statusText}`);
            return null;
        }

        const historyData: HistoryResponse = await response.json();

        // Update context state based on response
        context.hasMoreHistory = historyData.meta.has_more_before;

        // Always use the metadata's earliest_event_id for next pagination boundary
        if (historyData.meta.earliest_event_id) {
            context.earliestHistoryEventId = historyData.meta.earliest_event_id;
        }

        if (historyData.events.length === 0) {
            console.log("No historical events to display");
        } else {
            // Display historical events in chronological order (oldest first)
            console.log(`Displaying ${historyData.events.length} historical events`);

            // For prepending (pagination), we need to reverse the order so newest events get prepended first
            const eventsToDisplay = sinceEvent ? [...historyData.events].reverse() : historyData.events;

            for (const event of eventsToDisplay) {
                displayHistoricalEvent(context, event, sinceEvent ? "prepend" : "append");
            }

            // Add a visual separator between history and live events only on initial load
            if (!sinceEvent) {
                addHistorySeparator();
                // Update virtual spacer height for initial load
                updateVirtualSpacerHeight(context);

                // Set initial scroll position to just above the separator
                setTimeout(() => {
                    const narrative = document.getElementById("narrative");
                    const separator = document.querySelector(".history_separator");
                    if (narrative && separator) {
                        const separatorPosition = (separator as HTMLElement).offsetTop;
                        const virtualSpacer = document.getElementById("virtual_history_spacer");
                        const spacerHeight = virtualSpacer ? parseInt(virtualSpacer.style.height) || 0 : 0;
                        narrative.scrollTop = separatorPosition - 100; // Scroll to just above separator
                    }
                }, 50);
            }
        }

        return historyData;
    } catch (error) {
        console.error("Error fetching history:", error);
        context.systemMessage.show("Failed to load message history", 3);
        return null;
    }
}

/**
 * Loads more historical events for infinite scrolling
 *
 * @param context - Application context
 * @param chunkSize - Number of events to fetch (default: 50)
 */
export async function loadMoreHistory(context: Context, chunkSize: number = 50): Promise<boolean> {
    // Don't load if already loading or no more history available
    if (context.historyLoading || !context.hasMoreHistory || !context.earliestHistoryEventId) {
        return false;
    }

    context.historyLoading = true;

    try {
        const historyData = await fetchAndDisplayHistory(context, chunkSize, context.earliestHistoryEventId);
        return historyData !== null && historyData.events.length > 0;
    } finally {
        context.historyLoading = false;
    }
}

/**
 * Updates the virtual spacer height to maintain scrollbar appearance
 *
 * @param context - Application context
 */
function updateVirtualSpacerHeight(context: Context): void {
    const virtualSpacer = document.getElementById("virtual_history_spacer");
    if (!virtualSpacer) return;

    // Increase virtual spacer height based on available history
    // This creates the illusion of more content above
    if (context.hasMoreHistory) {
        const currentHeight = parseInt(virtualSpacer.style.height) || 1000;
        const newHeight = Math.max(currentHeight, 2000); // At least 2000px
        virtualSpacer.style.height = `${newHeight}px`;
    } else {
        // Gradually reduce spacer height when no more history
        virtualSpacer.style.height = "100px";
    }
}

/**
 * Displays a single historical event in the narrative with historical styling
 *
 * @param context - Application context
 * @param event - The historical event to display
 * @param insertMode - Whether to append or prepend the event
 */
function displayHistoricalEvent(
    context: Context,
    event: HistoricalEvent,
    insertMode: "append" | "prepend" = "append",
): void {
    // Convert historical event to narrative event format
    const narrativeEvent: NarrativeEvent = {
        kind: "narrative_message" as any, // Use string to match enum value
        message: convertEventMessage(event.message),
        content_type: event.message.content_type || "text/plain",
        author: event.author,
        is_historical: true, // Mark as historical for visual distinction
        timestamp: new Date(event.timestamp).getTime(),
    };

    // Process the event through the normal narrative pipeline
    processNarrativeMessage(context, narrativeEvent, insertMode);
}

/**
 * Converts the event message from the history API format to narrative format
 *
 * @param message - The message object from history API
 * @returns String content for display
 */
function convertEventMessage(message: any): string {
    if (typeof message === "string") {
        return message;
    }

    if (message.type === "notify") {
        return message.content;
    } else if (message.type === "traceback") {
        return `ERROR: ${message.error}`;
    } else if (message.type === "present") {
        return `[Presentation: ${message.presentation}]`;
    } else if (message.type === "unpresent") {
        return `[Closed: ${message.id}]`;
    }

    // Fallback for unknown message types
    return JSON.stringify(message);
}

/**
 * Adds a visual separator between historical and live events
 */
function addHistorySeparator(): void {
    const output = document.getElementById("output_window");
    if (!output) {
        console.error("Cannot find output window element");
        return;
    }

    const separator = div({
        class: "history_separator",
    }, "─── Live Events ───");

    output.appendChild(separator);

    // Scroll to the separator
    setTimeout(() => {
        const narrative = document.getElementById("narrative");
        if (narrative) {
            narrative.scrollTop = narrative.scrollHeight;
        }
    }, 0);
}

/**
 * Content type constants
 */
const CONTENT_TYPES = {
    PLAIN: "text/plain",
    HTML: "text/html",
    DJOT: "text/djot",
};

/**
 * Presentation target types
 */
const TARGET_TYPES = {
    WINDOW: "window",
    RIGHT_DOCK: "right-dock",
    LEFT_DOCK: "left-dock",
    TOP_DOCK: "top-dock",
    BOTTOM_DOCK: "bottom-dock",
    VERB_EDITOR: "verb-editor",
};

/**
 * MCP command constants
 */
const MCP_PREFIX = "#$# ";
const MCP_COMMAND = {
    EDIT: "edit",
};

/**
 * Default presentation parameters
 */
const DEFAULT_PRESENTATION = {
    WIDTH: 500,
    HEIGHT: 300,
};

/**
 * Creates DOM elements from an HTML string
 *
 * @param html - HTML string to convert to DOM elements
 * @returns Collection of DOM elements created from the HTML
 */
function generateElements(html: string): HTMLCollection {
    const template = document.createElement("template");
    template.innerHTML = html.trim();
    return template.content.children;
}

/**
 * Renders a Djot document as HTML and returns a DOM element
 *
 * @param options - Options object containing djot_text state
 * @returns A div element containing the rendered Djot content
 */
export const displayDjot = ({ djot_text }: { djot_text: State<string> }): HTMLElement => {
    // Parse the Djot text into an AST
    const ast = djot.parse(djot_text.val);

    // Convert the AST to HTML
    const html = djot.renderHTML(ast);

    // Create DOM elements from the HTML
    const elements = generateElements(html);

    // Create a container and append all elements
    const container = div();
    for (let i = 0; i < elements.length; i++) {
        container.appendChild(elements[i]);
    }

    return container;
};

/**
 * Sets up DOMPurify security configuration for HTML sanitization
 *
 * Configures two important security features:
 * 1. Forces all links to open in a new window
 * 2. Restricts URLs to an allowlist of protocols
 */
export function htmlPurifySetup(): void {
    // Allowed URL protocols
    const ALLOWED_PROTOCOLS = ["http", "https"];
    const protocolRegex = new RegExp(
        `^(${ALLOWED_PROTOCOLS.join("|")}):`,
        "gim",
    );

    // Add a hook to make all links open in a new window
    DOMPurify.addHook("afterSanitizeAttributes", (node: Element) => {
        // For elements that can have a target attribute
        if ("target" in node) {
            node.setAttribute("target", "_blank");
        }

        // For SVG links and other non-HTML links
        if (
            !node.hasAttribute("target")
            && (node.hasAttribute("xlink:href") || node.hasAttribute("href"))
        ) {
            node.setAttribute("xlink:show", "new");
        }
    });

    // Add a hook to enforce protocol allowlist
    DOMPurify.addHook("afterSanitizeAttributes", (node: Element) => {
        // Use an anchor element to parse URLs
        const anchor = document.createElement("a");

        // Validate href attributes
        if (node.hasAttribute("href")) {
            anchor.href = node.getAttribute("href") || "";
            if (anchor.protocol && !protocolRegex.test(anchor.protocol)) {
                node.removeAttribute("href");
            }
        }

        // Validate form action attributes
        if (node.hasAttribute("action")) {
            anchor.href = node.getAttribute("action") || "";
            if (anchor.protocol && !protocolRegex.test(anchor.protocol)) {
                node.removeAttribute("action");
            }
        }

        // Validate SVG xlink:href attributes
        if (node.hasAttribute("xlink:href")) {
            anchor.href = node.getAttribute("xlink:href") || "";
            if (anchor.protocol && !protocolRegex.test(anchor.protocol)) {
                node.removeAttribute("xlink:href");
            }
        }
    });
}

/**
 * Sanitizes HTML content for safe display
 *
 * @param author - The source/author of the HTML content
 * @param html - The raw HTML to sanitize
 * @returns Sanitized HTML string
 *
 * @todo Implement signing to verify content from privileged users
 */
function htmlSanitize(author: string, html: string): string {
    // TODO: Implement a signing mechanism so that only content from
    // authorized sources (wizards) can include HTML. The content should
    // be signed by the server and verified here.
    return DOMPurify.sanitize(html);
}

/**
 * Invokes a verb on a remote object
 *
 * Called from dynamically generated links in the narrative content
 *
 * @param author - The object ID on which to invoke the verb
 * @param verb - The verb name to invoke
 * @param argument - Optional argument to pass to the verb
 */
export function actionInvoke(author: string, verb: string, argument?: any): void {
    console.log(`Invoking ${verb} on ${author}`);

    const mrpcObject = new MoorRemoteObject(author, module.context.authToken);
    const verbArguments = argument !== undefined ? [argument] : [];

    mrpcObject.callVerb(verb, verbArguments)
        .then(result => {
            console.log(`Result from ${verb}:`, result);
        })
        .catch(error => {
            console.error(`Error invoking ${verb} on ${author}:`, error);
        });
}

// Keep the legacy function name for backward compatibility
// This allows existing javascript: URLs to continue working
export const action_invoke = actionInvoke;

/**
 * Special renderer for Djot content that handles object verb links
 *
 * This overrides the standard Djot renderer to support special link formats
 * for invoking verbs on MOO objects.
 *
 * Link format: invoke/verb_name/optional_argument
 *
 * @param author - The object ID that authored the content
 * @param ast - Djot AST to render
 * @returns HTML string with special links processed
 *
 * @todo Implement signed tokens for secure verb invocation
 */
export function djotRender(author: string, ast: any): string {
    return djot.renderHTML(ast, {
        overrides: {
            link: (node: any, renderer: any) => {
                const destination = node.destination;

                // Parse the destination format
                const spec = destination.split("/");

                // Validate the format
                if (spec.length > 3) {
                    console.warn(`Invalid destination format: ${destination}`);
                    return "";
                }

                // Process invoke links
                if (spec[0] === "invoke") {
                    const verb = spec[1];
                    // Handle optional argument (third component)
                    const arg = spec.length > 2 ? JSON.stringify(spec[2]) : "null";

                    // TODO: Replace with a secure token mechanism (PASETO)
                    // Create a javascript: URL that will call our action_invoke function
                    const jsHandler = `module.action_invoke("${author}", "${verb}", ${arg})`;
                    const linkText = renderer.render(node.children[0]);

                    return `<a href='javascript:${jsHandler}'>${linkText}</a>`;
                }

                // Default link behavior for non-invoke links
                return "";
            },

            url: (node: any, renderer: any) => {
                // Format automatic links to open in a new tab
                const destination = node.text;
                return `link: <a href='${destination}' target='_blank' rel='noopener noreferrer'>${destination}</a>`;
            },
        },
    });
}

/**
 * Adds content to the narrative window
 *
 * @param contentNode - The HTML element to add to the narrative
 * @param insertMode - Whether to append or prepend the content
 * @throws Error if output_window or narrative elements don't exist
 */
function narrativeInsert(contentNode: HTMLElement, insertMode: "append" | "prepend" = "append"): void {
    const output = document.getElementById("output_window");
    if (!output) {
        console.error("Cannot find output window element");
        return;
    }

    if (insertMode === "prepend") {
        // Find the virtual spacer (should be first child)
        const virtualSpacer = document.getElementById("virtual_history_spacer");
        if (virtualSpacer && virtualSpacer.parentNode === output) {
            // Insert after the virtual spacer
            output.insertBefore(contentNode, virtualSpacer.nextSibling);
        } else {
            // Fallback: insert at the beginning
            output.insertBefore(contentNode, output.firstChild);
        }
    } else {
        // Add the content to the end of the output window
        output.appendChild(contentNode);
    }

    // Find the narrative container
    const narrative = document.getElementById("narrative");
    if (!narrative) {
        console.error("Cannot find narrative element");
        return;
    }

    // For append mode, scroll to bottom. For prepend mode, maintain scroll position
    if (insertMode === "append") {
        setTimeout(() => {
            narrative.scrollTop = narrative.scrollHeight;
            document.body.scrollTop = document.body.scrollHeight;
        }, 0);
    }
}

/**
 * Appends content to the narrative window and scrolls to the bottom (legacy function)
 *
 * @param contentNode - The HTML element to append to the narrative
 */
function narrativeAppend(contentNode: HTMLElement): void {
    narrativeInsert(contentNode, "append");
}

/**
 * Processes a narrative message from the server
 *
 * @param context - Application context
 * @param msg - The narrative event to process
 * @param insertMode - Whether to append or prepend the content
 */
function processNarrativeMessage(
    context: Context,
    msg: NarrativeEvent,
    insertMode: "append" | "prepend" = "append",
): void {
    // Determine content type, defaulting to plain text
    const contentType = msg.content_type || CONTENT_TYPES.PLAIN;

    // Handle MCP-style edit commands
    // Format: #$# edit name: Object:verb upload: @program #object:verb permissions
    if (
        contentType === CONTENT_TYPES.PLAIN && typeof msg.message === "string"
        && msg.message.startsWith(MCP_PREFIX)
    ) {
        if (handleMcpCommand(context, msg.message)) {
            return; // MCP command was handled
        }
    }

    // Continue processing with spool or regular content handling
    processMessageContent(context, msg, contentType, insertMode);
}

/**
 * Handles MCP-style commands in the message stream
 *
 * @param context - Application context
 * @param message - The message containing an MCP command
 * @returns true if the command was handled, false otherwise
 */
function handleMcpCommand(context: Context, message: string): boolean {
    // Extract the command part after the MCP prefix
    const mcpCommand = message.substring(MCP_PREFIX.length);
    console.log(`MCP command: ${mcpCommand}`);

    // Parse the command parts
    const parts = mcpCommand.split(" ");
    if (parts.length < 2) {
        console.warn(`Invalid MCP command (too few parts): ${mcpCommand}`);
        return false;
    }

    const commandType = parts[0];

    // Currently we only support the 'edit' command
    if (commandType !== MCP_COMMAND.EDIT) {
        console.warn(`Unknown MCP command type: ${commandType}`);
        return false;
    }

    // Validate the name parameter
    if (parts[1] !== "name:") {
        console.warn(`Expected 'name:' parameter in MCP command: ${mcpCommand}`);
        return false;
    }

    // Parse the object:verb format
    const name = parts[2];
    const nameParts = name.split(":");
    if (nameParts.length !== 2) {
        console.warn(`Invalid object:verb format in MCP command: ${name}`);
        return false;
    }

    // Extract object reference and verb
    const objectName = nameParts[0];
    const verbName = nameParts[1];
    const object = matchRef(objectName);

    // Extract the upload command
    const uploadParts = mcpCommand.split("upload: ");
    if (uploadParts.length < 2) {
        console.warn(`Missing 'upload:' parameter in MCP command: ${mcpCommand}`);
        return false;
    }
    const uploadCommand = uploadParts[1];

    // Create a new spool for collecting the code
    context.spool = new Spool(
        SpoolType.Verb,
        name,
        object,
        verbName,
        uploadCommand,
    );

    return true; // Command was handled successfully
}

/**
 * Processes the content portion of a message after MCP handling
 *
 * @param context - Application context
 * @param msg - The narrative event to process
 * @param contentType - The content type of the message
 * @param insertMode - Whether to append or prepend the content
 */
function processMessageContent(
    context: Context,
    msg: NarrativeEvent,
    contentType: string,
    insertMode: "append" | "prepend" = "append",
): void {
    // Handle active spool collection for text content
    if (context.spool !== null && contentType === CONTENT_TYPES.PLAIN) {
        // A single period on a line marks the end of spool data collection
        if (msg.message === ".") {
            const spool = context.spool;
            const name = spool.name;
            const code = spool.take();

            // Launch the verb editor with the collected code
            showVerbEditor(context, name, spool.object, spool.entity, code);

            // Clear the spool after launching editor
            context.spool = null;
        } else {
            // Add the line to the spool
            context.spool.append(msg.message as string);
        }
        return;
    }

    // Normalize content to string format
    let content = msg.message;

    // Handle array content by joining with newlines
    if (Array.isArray(content)) {
        content = content.join("\n");
    }

    // We can only process string content
    if (typeof content !== "string") {
        console.warn(`Cannot process non-string content of type: ${contentType}`);
        return;
    }

    // Create a container for the content
    const contentNode = span();

    // Determine styling classes based on content type and historical status
    const isHistorical = msg.is_historical || false;
    const baseClass = isHistorical ? "historical" : "live";

    // Render content based on its type
    if (contentType === CONTENT_TYPES.DJOT) {
        // Parse and render Djot content
        const ast = djot.parse(content);
        const html = djotRender(msg.author, ast);
        const elements = generateElements(html);

        // Add all elements to the content node with appropriate styling
        for (let i = 0; i < elements.length; i++) {
            contentNode.append(div({ class: `text_djot ${baseClass}_djot` }, elements[i]));
        }
    } else if (contentType === CONTENT_TYPES.HTML) {
        // Sanitize and render HTML content
        const html = htmlSanitize(msg.author, content);
        const elements = generateElements(html);

        // Add all elements to the content node with appropriate styling
        for (let i = 0; i < elements.length; i++) {
            contentNode.append(div({ class: `text_html ${baseClass}_html` }, elements[i]));
        }
    } else {
        // Default case: plain text
        contentNode.append(div({ class: `text_narrative ${baseClass}_narrative` }, content));
    }

    // Add the content to the narrative display
    narrativeInsert(contentNode, insertMode);
}

function handleSystemMessage(context: Context, msg: SystemEvent) {
    // pop up into the toast notification at the top
    let content = msg.system_message;
    context.systemMessage.show(content, 2);

    // Also append to the narrative window.
    let content_node = div({ class: "system_message_narrative" }, content);
    narrativeAppend(content_node);
}

/**
 * Creates a content element based on content type
 *
 * @param contentType - MIME type of the content
 * @param content - The raw content string
 * @param sourceId - ID of the content source (for attribution)
 * @returns HTML element containing the rendered content
 */
function createContentElement(
    contentType: string,
    content: string,
    sourceId: string,
): HTMLElement {
    if (contentType === CONTENT_TYPES.HTML) {
        // Sanitize and render HTML content
        const html = htmlSanitize(sourceId, content);
        const container = div();
        const elements = generateElements(html);

        for (let i = 0; i < elements.length; i++) {
            container.appendChild(elements[i]);
        }
        return container;
    } else if (contentType === CONTENT_TYPES.PLAIN) {
        // Simple plain text rendering
        return div(content);
    } else if (contentType === CONTENT_TYPES.DJOT) {
        // Parse and render Djot content
        const ast = djot.parse(content);
        const html = djotRender(sourceId, ast);
        const elements = generateElements(html);
        const container = div();

        for (let i = 0; i < elements.length; i++) {
            container.appendChild(elements[i]);
        }
        return container;
    } else {
        // Fallback for unknown content types
        console.warn(`Unknown content type: ${contentType}, treating as plain text`);
        return div(`[Content with unsupported type: ${contentType}]\n${content}`);
    }
}

/**
 * Handles presentation messages from the server
 *
 * Presentations are UI elements that can appear in various targets (windows, panels)
 * based on their specified target type.
 *
 * @param context - Application context
 * @param msg - The presentation data
 */
function handlePresent(context: Context, msg: Presentation): void {
    // Convert attributes array to a key-value object
    const attrs: Record<string, string> = {};
    for (const [key, value] of msg.attributes) {
        attrs[key] = value;
    }

    // Create the appropriate content element based on content type
    let content: HTMLElement;
    try {
        content = createContentElement(msg.content_type, msg.content, msg.id);
    } catch (error) {
        console.error(`Error creating content for presentation ${msg.id}:`, error);
        return;
    }

    // Create the presentation model
    const model: State<PresentationModel> = van.state({
        id: msg.id,
        closed: van.state(false),
        target: msg.target,
        content: content,
        attrs: attrs,
    });

    // Add to the presentation manager
    context.presentations.val = context.presentations.val.withAdded(msg.id, model);

    // Handle specific presentation targets
    switch (msg.target) {
        case TARGET_TYPES.WINDOW:
            createFloatingWindow(context, model, attrs, content);
            break;

        case TARGET_TYPES.VERB_EDITOR:
            launchVerbEditorFromPresentation(context, attrs);
            break;

        case TARGET_TYPES.RIGHT_DOCK:
        case TARGET_TYPES.LEFT_DOCK:
        case TARGET_TYPES.TOP_DOCK:
        case TARGET_TYPES.BOTTOM_DOCK:
            // Dock presentations are handled by their respective dock components
            // They're automatically displayed when added to the presentation manager
            break;

        default:
            console.warn(`Unknown presentation target: ${msg.target}`);
    }
}

/**
 * Dismisses a presentation on the server side
 *
 * @param context - Application context
 * @param presentationId - ID of the presentation to dismiss
 */
export async function dismissPresentation(context: Context, presentationId: string): Promise<void> {
    if (!context.authToken) {
        console.warn("Cannot dismiss presentation: no auth token available");
        return;
    }

    try {
        const response = await fetch(`/api/presentations/${encodeURIComponent(presentationId)}`, {
            method: "DELETE",
            headers: {
                "X-Moor-Auth-Token": context.authToken,
            },
        });

        if (!response.ok) {
            console.error(
                `Failed to dismiss presentation ${presentationId}: ${response.status} ${response.statusText}`,
            );
            return;
        }

        console.log(`Successfully dismissed presentation: ${presentationId}`);
    } catch (error) {
        console.error(`Error dismissing presentation ${presentationId}:`, error);
    }
}

/**
 * Creates a floating window for a window-target presentation
 *
 * @param context - Application context
 * @param model - The presentation model
 * @param attrs - Presentation attributes
 * @param content - The content element to display
 */
function createFloatingWindow(
    context: Context,
    model: State<PresentationModel>,
    attrs: Record<string, string>,
    content: HTMLElement,
): void {
    // Get window parameters with defaults
    const title = attrs["title"] || model.val.id;
    const width = parseInt(attrs["width"] || `${DEFAULT_PRESENTATION.WIDTH}`, 10);
    const height = parseInt(attrs["height"] || `${DEFAULT_PRESENTATION.HEIGHT}`, 10);

    // Create our own close state that triggers dismiss
    const customClosed = van.state(false);

    // Watch our custom close state and handle user close
    van.derive(() => {
        if (customClosed.val) {
            console.log(`User closed floating window presentation ${model.val.id}, dismissing...`);
            // Call our dismiss handler
            handleUserClosePresentation(context, model.val.id);
        }
    });

    // Also watch for server-initiated closes (from unpresent events)
    van.derive(() => {
        if (model.val.closed.val) {
            console.log(`Server closed presentation ${model.val.id}, closing window...`);
            customClosed.val = true;
        }
    });

    // Create the floating window
    const windowElement = div(
        FloatingWindow(
            {
                parentDom: document.body,
                title: title,
                closed: customClosed, // Use our custom closed state that triggers dismiss
                id: `window-present-${model.val.id}`,
                width,
                height,
                windowClass: "presentation_window",
            },
            div(
                {
                    class: "presentation_window_content",
                },
                content,
            ),
        ),
    );

    // Add to the document body
    van.add(document.body, windowElement);
}

/**
 * Launches a verb editor from a verb-editor presentation
 *
 * @param context - Application context
 * @param attrs - Presentation attributes containing object and verb info
 */
function launchVerbEditorFromPresentation(
    context: Context,
    attrs: Record<string, string>,
): void {
    // Extract object and verb information
    const objectCurie = attrs["object"];
    const verbName = attrs["verb"];

    if (!objectCurie || !verbName) {
        console.error("Missing object or verb in verb-editor presentation", attrs);
        return;
    }

    // Convert CURIE to object reference
    const objectRef = curieORef(objectCurie);
    const editorTitle = `Edit: ${objectCurie}:${verbName}`;

    // Launch the verb editor
    launchVerbEditor(context, editorTitle, objectRef, verbName);
}

/**
 * Handles unpresent messages that remove a presentation
 *
 * @param context - Application context
 * @param id - ID of the presentation to remove
 */
function handleUnpresent(context: Context, id: string): void {
    // Check if the presentation exists
    const presentationManager = context.presentations.val;
    if (!presentationManager.hasPresentation(id)) {
        console.warn(`Cannot unpresent non-existent presentation: ${id}`);
        return;
    }

    // Mark as closed and remove from manager
    const presentation = presentationManager.getPresentation(id);
    if (presentation) {
        presentation.val.closed.val = true;
        context.presentations.val = presentationManager.withRemoved(id);
        console.log(`Closed presentation: ${id}`);
    }
}

/**
 * Handles user-initiated close of a presentation (e.g., clicking close button)
 * This should be called when the user closes a presentation locally
 *
 * @param context - Application context
 * @param id - ID of the presentation to close
 */
export function handleUserClosePresentation(context: Context, id: string): void {
    console.log(`User closed presentation: ${id}`);

    // First dismiss on the server
    dismissPresentation(context, id);

    // Then handle local cleanup
    handleUnpresent(context, id);
}

function handleTraceback(context: Context, traceback: Traceback) {
    let content = traceback.traceback.join("\n");
    let content_node = div({ class: "traceback_narrative" }, content);
    narrativeAppend(content_node);
}

/**
 * Processes an event message from the server
 *
 * This is the main entry point for handling all types of server messages.
 * It parses the JSON message and dispatches to the appropriate handler.
 *
 * @param context - Application context
 * @param msg - The raw message string from the WebSocket
 */
export function handleEvent(context: Context, msg: string): void {
    // Process the event directly (no queuing needed since history is loaded before WebSocket opens)
    processEvent(context, msg);
}

/**
 * Internal function to actually process an event
 */
function processEvent(context: Context, msg: string): void {
    // Parse the JSON message
    let event: any;
    try {
        event = JSON.parse(msg);
    } catch (error) {
        console.error("Failed to parse server message:", error, msg);
        return;
    }

    if (!event) {
        console.warn("Received empty event");
        return;
    }

    // Check for duplicate events based on server timestamp
    // Events sent before history boundary are considered duplicates
    if (event.server_time) {
        const eventTimestamp = new Date(event.server_time).getTime();
        if (context.isHistoricalDuplicate(eventTimestamp)) {
            console.log(`Skipping duplicate historical event (timestamp: ${new Date(eventTimestamp).toISOString()})`);
            return;
        }
    }

    // Dispatch to the appropriate handler based on event type
    try {
        // Check for message property that could be empty string or null
        if ("message" in event) {
            processNarrativeMessage(context, event as NarrativeEvent);
        } else if (event["system_message"]) {
            handleSystemMessage(context, event as SystemEvent);
        } else if (event["present"]) {
            handlePresent(context, event["present"] as Presentation);
        } else if (event["unpresent"]) {
            handleUnpresent(context, event["unpresent"] as string);
        } else if (event["traceback"]) {
            handleTraceback(context, event["traceback"] as Traceback);
        } else {
            console.warn("Unknown event type:", event);
        }
    } catch (error) {
        console.error("Error handling event:", error, event);
    }
}

/**
 * Component that displays the narrative output content
 *
 * @param context - Application context for history state
 * @param player - Reactive player state
 * @returns A VanJS component
 */
const OutputWindow = (context: Context, player: State<Player>): HTMLElement => {
    const outputWindow = div({
        id: "output_window",
        class: "output_window",
        role: "log",
        "aria-live": "polite",
        "aria-atomic": "false",
        // Add ARIA attributes for accessibility
        "aria-label": "Game narrative content",
        "aria-relevant": "additions",
    });

    // Add a virtual spacer at the top to create the illusion of more content
    // This makes the scrollbar appear to have more content above
    const virtualSpacer = div({
        id: "virtual_history_spacer",
        style: "height: 2000px; background: transparent; pointer-events: none;", // Start with significant height
        "aria-hidden": "true",
    });

    outputWindow.appendChild(virtualSpacer);

    return outputWindow;
};

/**
 * Handles navigation through command history
 *
 * @param context - Application context
 * @param inputElement - The input textarea element
 * @param direction - Direction to navigate ('up' or 'down')
 */
function navigateHistory(
    context: Context,
    inputElement: HTMLTextAreaElement,
    direction: "up" | "down",
): void {
    // Skip history navigation if in multiline mode with cursor in middle
    const isMultiline = inputElement.value.includes("\n");
    const cursorAtEdge = inputElement.selectionStart === 0
        || (inputElement.selectionStart === inputElement.selectionEnd
            && inputElement.selectionStart === inputElement.value.length);

    if (isMultiline && !cursorAtEdge) {
        return; // Let default behavior handle cursor movement
    }

    // Adjust history offset based on direction
    if (direction === "up" && context.historyOffset < context.history.length) {
        context.historyOffset += 1;
    } else if (direction === "down" && context.historyOffset > 0) {
        context.historyOffset -= 1;
    } else {
        return; // Cannot navigate further
    }

    // Calculate the history index
    const historyIndex = context.history.length - context.historyOffset;

    // Set input value from history or clear if nothing available
    if (historyIndex >= 0 && historyIndex < context.history.length) {
        const historyValue = context.history[historyIndex];
        inputElement.value = historyValue ? historyValue.trimEnd() : "";
    } else {
        inputElement.value = "";
    }
}

/**
 * Sends input to the server and updates UI
 *
 * @param context - Application context
 * @param inputElement - The input textarea element
 */
function sendInput(context: Context, inputElement: HTMLTextAreaElement): void {
    // Store the input value before clearing
    const input = inputElement.value.trim();
    if (!input) return; // Don't send empty input

    // Find the output window
    const output = document.getElementById("output_window");
    if (!output) {
        console.error("Cannot find output window element");
        return;
    }

    // For actual sent content we split linefeeds to avoid sending multiline content
    const lines = inputElement.value.split("\n");

    for (const line of lines) {
        if (line.trim()) { // Skip empty lines
            // Echo input to the narrative window
            const echo = div(
                { class: "input_echo" },
                `> ${line}`,
            );
            output.appendChild(echo);

            // Send the command to the server
            if (context.ws && context.ws.readyState === WebSocket.OPEN) {
                context.ws.send(line);
            } else {
                console.error("WebSocket not connected");
                // Display an error in the narrative
                output.appendChild(div(
                    { class: "system_message_narrative" },
                    "Error: Not connected to server",
                ));
            }
        }
    }

    // Clear the input area
    inputElement.value = "";

    // Add to command history and reset offset
    if (input) {
        context.history.push(input);
        context.historyOffset = 0;
    }

    // Scroll the narrative to the bottom
    const narrative = document.getElementById("narrative");
    if (narrative) {
        narrative.scrollTop = narrative.scrollHeight;
    }
}

/**
 * Component that provides the command input area
 *
 * @param context - Application context
 * @param player - Reactive player state
 * @returns A VanJS component
 */
const InputArea = (context: Context, player: State<Player>): HTMLElement => {
    // Hide the input area when disconnected
    const hiddenStyle = van.derive(() => player.val.connected ? "display: block;" : "display: none;");

    // Create the textarea element
    const inputElement = textarea({
        id: "input_area",
        style: hiddenStyle,
        disabled: van.derive(() => !player.val.connected),
        class: "input_area",
        placeholder: "Type a command...",
        autocomplete: "off",
        spellcheck: false,
        "aria-label": "Command input",
    });

    // Handle paste events
    inputElement.addEventListener("paste", (e) => {
        // Directly process the pasted content at cursor position
        e.stopPropagation();
        e.preventDefault();

        const pastedData = e.clipboardData?.getData("text") || "";
        if (!pastedData) return;

        // Insert the pasted data at the current cursor position
        const selStart = inputElement.selectionStart || 0;
        const selEnd = inputElement.selectionEnd || 0;

        inputElement.value = inputElement.value.substring(0, selStart)
            + pastedData
            + inputElement.value.substring(selEnd);

        // Place cursor after the pasted content
        const newPosition = selStart + pastedData.length;
        inputElement.selectionStart = newPosition;
        inputElement.selectionEnd = newPosition;
    });

    // Handle keyboard events
    inputElement.addEventListener("keydown", (e) => {
        // Handle history navigation with arrow keys
        if (e.key === "ArrowUp") {
            e.preventDefault();
            navigateHistory(context, inputElement, "up");
        } else if (e.key === "ArrowDown") {
            e.preventDefault();
            navigateHistory(context, inputElement, "down");
        } // Handle Shift+Enter for newlines
        else if (e.key === "Enter" && e.shiftKey) {
            e.preventDefault();

            // Insert a newline at the cursor position
            const cursor = inputElement.selectionStart || 0;
            inputElement.value = inputElement.value.substring(0, cursor)
                + "\n"
                + inputElement.value.substring(inputElement.selectionEnd || cursor);

            // Move cursor after the inserted newline
            inputElement.selectionStart = cursor + 1;
            inputElement.selectionEnd = cursor + 1;
        } // Handle Enter to send input
        else if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            sendInput(context, inputElement);
        }
    });

    return div(inputElement);
};

/**
 * Handles scroll events to trigger infinite history loading and track viewing state
 *
 * @param context - Application context
 * @param narrativeElement - The narrative container element
 */
function handleScrollForHistoryLoading(context: Context, narrativeElement: HTMLElement): void {
    const scrollTop = narrativeElement.scrollTop;
    const scrollHeight = narrativeElement.scrollHeight;
    const clientHeight = narrativeElement.clientHeight;

    // Check if user is near the bottom (within 100px)
    const isNearBottom = (scrollTop + clientHeight) >= (scrollHeight - 100);

    // Update viewing history state
    context.isViewingHistory = !isNearBottom;

    // Only load more history when scrolled near the top (accounting for virtual spacer)
    const scrollThreshold = 200; // pixels from virtual top
    const virtualSpacer = document.getElementById("virtual_history_spacer");
    const virtualSpacerHeight = virtualSpacer ? parseInt(virtualSpacer.style.height) || 0 : 0;

    if (scrollTop <= (virtualSpacerHeight + scrollThreshold) && context.hasMoreHistory && !context.historyLoading) {
        console.log("Loading more history due to scroll position");

        // Store current scroll position to restore after loading
        const currentScrollHeight = narrativeElement.scrollHeight;
        const currentScrollTop = narrativeElement.scrollTop;

        loadMoreHistory(context).then((loaded) => {
            if (loaded) {
                // Update virtual spacer height
                updateVirtualSpacerHeight(context);

                // Restore scroll position after new content is added
                setTimeout(() => {
                    const newScrollHeight = narrativeElement.scrollHeight;
                    const heightDifference = newScrollHeight - currentScrollHeight;
                    narrativeElement.scrollTop = currentScrollTop + heightDifference;
                }, 10);
            }
        });
    }
}

/**
 * Scrolls to the bottom of the narrative (jump to now)
 *
 * @param context - Application context
 */
function jumpToNow(context: Context): void {
    const narrative = document.getElementById("narrative");
    if (narrative) {
        narrative.scrollTop = narrative.scrollHeight;
        context.isViewingHistory = false;
    }
}

/**
 * Component that shows "viewing history" indicator with jump to now button
 *
 * @param context - Application context
 * @returns A VanJS component
 */
const HistoryIndicator = (context: Context): HTMLElement => {
    // Create reactive state for the indicator visibility
    const isVisible = van.state(false);

    // Update visibility based on context state
    const updateVisibility = () => {
        isVisible.val = context.isViewingHistory;
    };

    // Check visibility periodically (simple approach for reactivity)
    setInterval(updateVisibility, 100);

    return div(
        {
            class: van.derive(() => isVisible.val ? "history_indicator" : "history_indicator hidden"),
        },
        span("You're looking at the past..."),
        button(
            {
                onclick: () => jumpToNow(context),
            },
            "Jump to Now",
        ),
    );
};

/**
 * Main narrative component that displays the game interface
 *
 * This component combines the output window and input area to create
 * the primary interaction interface for the MOO client.
 *
 * @param context - Application context
 * @param player - Reactive player state
 * @returns A VanJS component
 */
export const Narrative = (context: Context, player: State<Player>): HTMLElement => {
    // Hide the narrative when not connected
    const visibilityStyle = van.derive(() => player.val.connected ? "display: block;" : "display: none;");

    const narrativeElement = div(
        {
            class: "narrative",
            id: "narrative",
            style: visibilityStyle,
            "aria-label": "Game interface",
        },
        // History viewing indicator
        HistoryIndicator(context),
        // Output display area
        OutputWindow(context, player),
        // Command input area
        InputArea(context, player),
    );

    // Add scroll event handler for infinite history loading
    let scrollTimeout: number | null = null;
    narrativeElement.addEventListener("scroll", () => {
        // Throttle scroll events
        if (scrollTimeout !== null) {
            clearTimeout(scrollTimeout);
        }

        scrollTimeout = setTimeout(() => {
            handleScrollForHistoryLoading(context, narrativeElement);
            scrollTimeout = null;
        }, 100);
    });

    return narrativeElement;
};
