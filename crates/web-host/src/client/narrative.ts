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

import van, {State} from "vanjs-core";

import {FloatingWindow} from "van-ui";
import {curieORef, MoorRemoteObject} from "./rpc";

import {
    Context,
    NarrativeEvent,
    Player,
    Presentation,
    PresentationModel,
    Spool,
    SpoolType,
    SystemEvent,
    Traceback
} from "./model";
import {matchRef} from "./var";
import {launchVerbEditor, showVerbEditor} from "./verb_edit";

// import sanitize html
import DOMPurify from "dompurify";

const { div, span, textarea } = van.tags;

// Utility function to build DOM elements from HTML.
function generateElements(html) {
    const template = document.createElement("template");
    template.innerHTML = html.trim();
    return template.content.children;
}

export const displayDjot = ({ djot_text }) => {
    let ast = djot.parse(djot_text.val);
    let html = djot.renderHTML(ast);
    let elements = generateElements(html);
    let d = div();
    for (let element of elements) {
        d.appendChild(element);
    }
    return d;
};

export function htmlPurifySetup() {
    // Add a hook to make all links open a new window
    DOMPurify.addHook("afterSanitizeAttributes", function(node) {
        // set all elements owning target to target=_blank
        if ("target" in node) {
            node.setAttribute("target", "_blank");
        }
        // set non-HTML/MathML links to xlink:show=new
        if (
            !node.hasAttribute("target")
            && (node.hasAttribute("xlink:href") || node.hasAttribute("href"))
        ) {
            node.setAttribute("xlink:show", "new");
        }
    });

    // Add a hook to enforce URI scheme allow-list
    const allowlist = ["http", "https"];
    const regex = RegExp("^(" + allowlist.join("|") + "):", "gim");
    DOMPurify.addHook("afterSanitizeAttributes", function(node) {
        // build an anchor to map URLs to
        const anchor = document.createElement("a");

        // check all href attributes for validity
        if (node.hasAttribute("href")) {
            anchor.href = node.getAttribute("href");
            if (anchor.protocol && !anchor.protocol.match(regex)) {
                node.removeAttribute("href");
            }
        }
        // check all action attributes for validity
        if (node.hasAttribute("action")) {
            anchor.href = node.getAttribute("action");
            if (anchor.protocol && !anchor.protocol.match(regex)) {
                node.removeAttribute("action");
            }
        }
        // check all xlink:href attributes for validity
        if (node.hasAttribute("xlink:href")) {
            anchor.href = node.getAttribute("xlink:href");
            if (anchor.protocol && !anchor.protocol.match(regex)) {
                node.removeAttribute("xlink:href");
            }
        }
    });
}

function htmlSanitize(author, html) {
    // TODO: there should be some signing on this to prevent non-wizards from doing bad things.
    //   basically only wizard-perm'd things should be able to send HTML, and it should be signed
    //   by the server as such.
    return DOMPurify.sanitize(html);
}

export function action_invoke(author, verb, argument) {
    console.log("Invoking " + verb + " on " + author);
    let mrpc_object = new MoorRemoteObject(author, module.context.authToken);
    let verb_arguments = [];
    if (argument) {
        verb_arguments.push(argument);
    }
    mrpc_object.callVerb(verb, verb_arguments).then((result) => {
        console.log("Result: " + result);
    });
}

// Override link behaviour for djot to only permit inline links that refer to object verbs.
// These get turned into requests to invoke the verb with the player's permissions.
export function djotRender(author, ast) {
    return djot.renderHTML(ast, {
        overrides: {
            link: (node, renderer) => {
                console.log("Link node: ", node);
                let destination = node.destination;

                // Destination structures:
                //   invoke an action verb on the author of the message with an optional argument
                //      invoke/<verb>[/arg]
                let spec = destination.split("/");
                if (spec.length > 3) {
                    console.log("Invalid destination: " + destination);
                    return "";
                }
                // Handle invoke:
                // TODO: these should all be constructed with a PASETO token that is produced by the server, signed etc.
                //   and then validated here on the client side.
                var function_invoke;
                if (spec[0] === "invoke") {
                    let verb = spec[1];
                    // If there's an argument, it's the second element.
                    let arg = JSON.stringify(spec[2]) || "null";

                    // Turns into a javascript: link that will invoke the verb on the object.
                    function_invoke = "module.action_invoke(\"" + author + "\", \"" + verb + "\", " + arg + ")";
                }
                return "<a href='javascript:" + function_invoke + "'>" + renderer.render(node.children[0]) + "</a>";
            },
            url: (node, renderer) => {
                // Autolinks have to open in another tab.
                // Hover should say something about external link
                let destination = node.text;
                return "link: <a href='" + destination + "' target='_blank'>" + destination + "</a>";
            },
        },
    });
}

function narrativeAppend(content_node: HTMLElement) {
    let output = document.getElementById("output_window");
    output.appendChild(content_node);
    let narrative = document.getElementById("narrative");
    // scroll to bottom
    narrative.scrollTop = narrative.scrollHeight;
    document.body.scrollTop = document.body.scrollHeight;
}

function processNarrativeMessage(context: Context, msg: NarrativeEvent) {
    // Msg may have content_type attr, and if so, check it, or default to text/plain
    let content_type = msg.content_type || "text/plain";

    // If msg is text/plain and prefixed with #$#, it's an MCP-ish thing.
    // We don't implement full MCP by any means, but we support local editing of this type:
    // #$# edit name: Wizard:edittest upload: @program #2:edittest this none this
    if (content_type === "text/plain" && msg.message.startsWith("#$# ")) {
        let mcp_command = msg.message.substring(4);
        console.log("MCP command: " + mcp_command);
        let parts = mcp_command.split(" ");
        if (parts.length < 2) {
            console.log("Invalid MCP command: " + mcp_command);
            return;
        }
        if (parts[0] !== "edit") {
            console.log("Unknown MCP command: " + parts[0]);
            return;
        }

        // parts[1] is "name: ",
        // parts[2] is object:verb.
        if (parts[1] != "name:") {
            console.log("Invalid MCP command: " + mcp_command);
            return;
        }

        let name = parts[2];
        let name_parts = name.split(":");
        if (name_parts.length != 2) {
            console.log("Invalid object:verb: " + name);
            return;
        }

        let object = matchRef(name_parts[0]);
        let verb = name_parts[1];

        let uploadCommand = mcp_command.split("upload: ")[1];

        context.spool = new Spool(SpoolType.Verb, name, object, verb, uploadCommand);

        return;
    }

    if (context.spool != null && content_type == "text/plain") {
        if (msg.message == ".") {
            let spool = context.spool;
            let name = spool.name;
            let code = spool.take();
            showVerbEditor(context, name, spool.object, spool.entity, code);
        } else {
            context.spool.append(msg.message);
        }
        return;
    }

    let output = document.getElementById("output_window");

    let content = msg.message;
    // If the content is a list, join together into one string with linefeeds.
    if (Array.isArray(content)) {
        content = content.join("\n");
    }

    // If it's not text at all, we can't currently do anything with it.
    if (typeof content !== "string") {
        console.log("Unknown content type: " + content_type);
        return;
    }

    // We can handle text/djot by turning into HTML. Otherwise you're gonna get raw text.
    let content_node = span();
    if (content_type === "text/djot") {
        let ast = djot.parse(content);
        let html = djotRender(msg.author, ast);
        let elements = generateElements(html);

        for (let element of elements) {
            content_node.append(div({ class: "text_djot" }, element));
        }
    } else if (content_type == "text/html") {
        let html = htmlSanitize(msg.author, content);
        let elements = generateElements(html);
        for (let element of elements) {
            content_node.append(div({ class: "text_html" }, element));
        }
    } else {
        content_node.append(div({ class: "text_narrative" }, content));
    }
    narrativeAppend(content_node);
}

function handleSystemMessage(context: Context, msg: SystemEvent) {
    // pop up into the toast notification at the top
    let content = msg.system_message;
    context.systemMessage.show(content, 2);

    // Also append to the narrative window.
    let content_node = div({ class: "system_message_narrative" }, content);
    narrativeAppend(content_node);
}

function handlePresent(context: Context, msg: Presentation) {
    // Turn the attributes into a dictionary.
    let attrs = {};
    for (let attr of msg.attributes) {
        attrs[attr[0]] = attr[1];
    }

    // Transform the content, based on content-type.
    // We support three types: text/html, text/plain, and text/djot.
    var content;
    if (msg.content_type == "text/html") {
        let html = htmlSanitize(msg.id, msg.content);
        let tag = div();
        let elements = generateElements(html);
        for (let element of elements) {
            tag.appendChild(element);
        }
        content = tag;
    } else if (msg.content_type == "text/plain") {
        content = div(msg.content);
    } else if (msg.content_type = "text/djot") {
        let ast = djot.parse(msg.content);
        let html = djotRender(msg.id, ast);
        let elements = generateElements(html);
        let tag = div();
        for (let element of elements) {
            tag.appendChild(element);
        }
        content = tag;
    } else {
        console.log("Unknown content type in presentation: " + msg.content_type);
        return;
    }

    let model: State<PresentationModel> = van.state({
        id: msg.id,
        closed: van.state(false),
        target: msg.target,
        content: content,
        attrs: attrs,
    });
    context.presentations.val = context.presentations.val.withAdded(msg.id, model);

    // types of targets:
    //      window: build a FloatingWindow
    //      etc
    if (msg.target == "window") {
        let title = attrs["title"] || msg.id;
        let width = attrs["width"] || 500;
        let height = attrs["height"] || 300;

        let present = div(
            FloatingWindow(
                {
                    parentDom: document.body,
                    title: title,
                    closed: model.closed,
                    id: "window-present-" + msg.id,
                    width: width,
                    height: height,
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
        van.add(document.body, present);
    }

    if (msg.target == "right-dock") {
        // TODO: anything special beyond just adding to the state?
    }

    if (msg.target == "verb-editor") {
        // attributes: object (curie), verb
        let object = attrs["object"];
        let verb = attrs["verb"];

        let oref = curieORef(object);
        launchVerbEditor(context, "Edit: " + object + ":" + verb, oref, verb);
    }
}

function handleUnpresent(context: Context, id: string) {
    let model = context.presentations.get(id);
    if (!model) {
        console.log("No such presentation: " + id);
        return;
    }
    console.log("Closing presentation: " + id);
    context.presentations.val.getPresentation(id).closed.val = true;
    context.presentations.val = context.presentations.val.withRemoved(id);
}

function handleTraceback(context: Context, traceback: Traceback) {
    let content = traceback.traceback.join("\n");
    let content_node = div({ class: "traceback_narrative" }, content);
    narrativeAppend(content_node);
}

// Process an inbound (JSON) event from the websocket connection to the server.
export function handleEvent(context: Context, msg) {
    let event = JSON.parse(msg);
    if (!event) {
        return;
    }
    if (event["message"]) {
        processNarrativeMessage(context, event);
    } else if (event["system_message"]) {
        handleSystemMessage(context, event);
    } else if (event["present"]) {
        handlePresent(context, event["present"]);
    } else if (event["unpresent"]) {
        handleUnpresent(context, event["unpresent"]);
    } else if (event["traceback"]) {
        handleTraceback(context, event["traceback"]);
    } else {
        console.log("Unknown event type: " + event);
    }
}

// Text area where output will go
const OutputWindow = (player: State<Player>) => {
    return div({
        id: "output_window",
        class: "output_window",
        role: "log",
        "aria-live": "polite",
        "aria-atomic": "false",
    });
};

const InputArea = (context: Context, player: State<Player>) => {
    let hidden_style = van.derive(() => player.val.connected ? "display: block;" : "display: none;");
    const i = textarea({
        id: "input_area",
        style: hidden_style,
        disabled: van.derive(() => !player.val.connected),
        class: "input_area",
    });
    i.addEventListener("paste", e => {
        // Directly process the pasted content as-is and put it in the input box as-is
        e.stopPropagation();
        e.preventDefault();
        const pastedData = e.clipboardData?.getData("text") || "";
        if (pastedData) {
            // Insert the pasted data at the current cursor position.
            i.value = i.value.substring(0, i.selectionStart) + pastedData + i.value.substring(i.selectionEnd);

            // Jump the selection to the end of the pasted part
            i.selectionStart = i.selectionEnd = i.selectionStart + pastedData.length;
        }
    });
    i.addEventListener("keydown", e => {
        // Arrow up means go back in history and fill the input area with that, if there is any.
        if (e.key === "ArrowUp") {
            // If the field is multiple lines and the cursor is not at the beginning of the line or end of line,
            // then just do regular arrow-key stuff (don't go back in history).
            if (
                i.value.includes("\n")
                && (i.selectionStart != 0 && (i.selectionStart != i.selectionEnd || i.selectionStart != i.value.length))
            ) {
                return;
            }

            e.preventDefault();

            if (context.historyOffset < context.history.length) {
                context.historyOffset += 1;
                if (context.history.length - context.historyOffset >= 0) {
                    let value = context.history[context.history.length - context.historyOffset];
                    if (value) {
                        i.value = value.trimEnd();
                    } else {
                        i.value = "";
                    }
                }
            }
        } else if (e.key === "ArrowDown") {
            if (
                i.value.includes("\n")
                && (i.selectionStart != 0 && (i.selectionStart != i.selectionEnd || i.selectionStart != i.value.length))
            ) {
                return;
            }

            e.preventDefault();

            if (context.historyOffset > 0) {
                context.historyOffset -= 1;
                if (context.history.length - context.historyOffset >= 0) {
                    let value = context.history[context.history.length - context.historyOffset];
                    if (value) {
                        i.value = value.trimEnd();
                    } else {
                        i.value = "";
                    }
                }
            }
        } else if (e.keyCode == 13 && e.shiftKey) {
            // Shift-Enter means force a newline, like in other chat systems.
            e.preventDefault();

            // Put a newline in the input area at the current cursor position.
            let cursor = i.selectionStart;
            let text = i.value;
            i.value = text.substring(0, cursor) + "\n" + text.substring(cursor);
            i.selectionStart = cursor + 1;
            i.selectionEnd = cursor + 1;
        } else if (e.key === "Enter") {
            e.preventDefault();

            // Put a copy into the narrative window, send it over websocket, and clear
            let input = i.value;
            let output = document.getElementById("output_window");

            // For actual sent-content we split linefeeds out to avoid sending multiline content, at
            // least for now.
            const lines = i.value.split("\n");
            for (let line of lines) {
                let echo = div(
                    {
                        class: "input_echo",
                    },
                    "> " + line,
                );
                output.appendChild(echo);
                context.ws.send(line);
            }
            i.value = "";

            // Append (unmolested) to history
            context.history.push(input);
            context.historyOffset = 0;
        }
    });
    return div(i);
};

export const Narrative = (context: Context, player: State<Player>) => {
    let hidden_style = van.derive(() => player.val.connected ? "display: block;" : "display: none;");

    return div(
        {
            class: "narrative",
            id: "narrative",
            style: hidden_style,
        },
        OutputWindow(player),
        InputArea(context, player),
    );
};
