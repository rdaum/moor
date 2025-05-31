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
const { div, span, textarea } = van.tags;

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
 * Appends content to the narrative window and scrolls to the bottom
 *
 * @param contentNode - The HTML element to append to the narrative
 * @throws Error if output_window or narrative elements don't exist
 */
function narrativeAppend(contentNode: HTMLElement): void {
    const output = document.getElementById("output_window");
    if (!output) {
        console.error("Cannot find output window element");
        return;
    }

    // Add the content to the output window
    output.appendChild(contentNode);

    // Find the narrative container
    const narrative = document.getElementById("narrative");
    if (!narrative) {
        console.error("Cannot find narrative element");
        return;
    }

    // Scroll both the narrative container and body to the bottom
    // Using setTimeout to ensure this happens after rendering
    setTimeout(() => {
        narrative.scrollTop = narrative.scrollHeight;
        document.body.scrollTop = document.body.scrollHeight;
    }, 0);
}

/**
 * Processes a narrative message from the server
 *
 * @param context - Application context
 * @param msg - The narrative event to process
 */
function processNarrativeMessage(context: Context, msg: NarrativeEvent): void {
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
    processMessageContent(context, msg, contentType);
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
 */
function processMessageContent(context: Context, msg: NarrativeEvent, contentType: string): void {
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

    // Render content based on its type
    if (contentType === CONTENT_TYPES.DJOT) {
        // Parse and render Djot content
        const ast = djot.parse(content);
        const html = djotRender(msg.author, ast);
        const elements = generateElements(html);

        // Add all elements to the content node with appropriate styling
        for (let i = 0; i < elements.length; i++) {
            contentNode.append(div({ class: "text_djot" }, elements[i]));
        }
    } else if (contentType === CONTENT_TYPES.HTML) {
        // Sanitize and render HTML content
        const html = htmlSanitize(msg.author, content);
        const elements = generateElements(html);

        // Add all elements to the content node with appropriate styling
        for (let i = 0; i < elements.length; i++) {
            contentNode.append(div({ class: "text_html" }, elements[i]));
        }
    } else {
        // Default case: plain text
        contentNode.append(div({ class: "text_narrative" }, content));
    }

    // Add the content to the narrative display
    narrativeAppend(contentNode);
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
            createFloatingWindow(model, attrs, content);
            break;

        case TARGET_TYPES.VERB_EDITOR:
            launchVerbEditorFromPresentation(context, attrs);
            break;

        case TARGET_TYPES.RIGHT_DOCK:
            // Right dock presentations are handled by the RightDock component
            // They're automatically displayed when added to the presentation manager
            break;

        default:
            console.warn(`Unknown presentation target: ${msg.target}`);
    }
}

/**
 * Creates a floating window for a window-target presentation
 *
 * @param model - The presentation model
 * @param attrs - Presentation attributes
 * @param content - The content element to display
 */
function createFloatingWindow(
    model: State<PresentationModel>,
    attrs: Record<string, string>,
    content: HTMLElement,
): void {
    // Get window parameters with defaults
    const title = attrs["title"] || model.val.id;
    const width = parseInt(attrs["width"] || `${DEFAULT_PRESENTATION.WIDTH}`, 10);
    const height = parseInt(attrs["height"] || `${DEFAULT_PRESENTATION.HEIGHT}`, 10);

    // Create the floating window
    const windowElement = div(
        FloatingWindow(
            {
                parentDom: document.body,
                title: title,
                closed: model.val.closed,
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
 * @param player - Reactive player state
 * @returns A VanJS component
 */
const OutputWindow = (player: State<Player>): HTMLElement => {
    return div({
        id: "output_window",
        class: "output_window",
        role: "log",
        "aria-live": "polite",
        "aria-atomic": "false",
        // Add ARIA attributes for accessibility
        "aria-label": "Game narrative content",
        "aria-relevant": "additions",
    });
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

    return div(
        {
            class: "narrative",
            id: "narrative",
            style: visibilityStyle,
            "aria-label": "Game interface",
        },
        // Output display area
        OutputWindow(player),
        // Command input area
        InputArea(context, player),
    );
};
