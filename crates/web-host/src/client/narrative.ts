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

import {MoorRemoteObject, retrieveWelcome} from "./rpc";

import {matchRef} from "./var";
import {Player, Spool, SpoolType, Context, NarrativeEvent, SystemEvent} from "./model";
import {showVerbEditor} from "./verb_edit";

const {button, div, span, input, select, option, br, pre, form, a, p} = van.tags;

// Utility function to build DOM elements from HTML.
function generateElements(html) {
    const template = document.createElement("template");
    template.innerHTML = html.trim();
    return template.content.children;
}

export const displayDjot = ({djot_text}) => {
    let ast = djot.parse(djot_text.val);
    let html = djot.renderHTML(ast);
    let elements = generateElements(html);
    let d = div();
    for (let element of elements) {
        d.appendChild(element);
    }
    return d;
};


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
            }
        }
    });
}




function processNarrativeMessage(context :  Context, msg : NarrativeEvent) {
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
    if (content_type === "text/djot") {
        let ast = djot.parse(content);
        let html = djotRender(msg.author, ast);
        console.log("DJOT HTML: " + html);
        let elements = generateElements(html);
        for (let element of elements) {
            output.appendChild(div({class: "text_djot"}, element));
        }
    } else {
        let content_node = div({class: "text_narrative"}, content);
        output.appendChild(content_node);
    }
    // scroll to bottom
    output.scrollTop = output.scrollHeight;
    document.body.scrollTop = document.body.scrollHeight;
}

function handleSystemMessage(context : Context, msg: SystemEvent) {
    let content = msg.system_message;
    context.systemMessage.show(content, 2);
}

// Process an inbound (JSON) event from the websocket connection to the server.
export function handleEvent(context : Context, msg) {
    let event = JSON.parse(msg);
    if (!event) {
        console.log("No event data in message: " + msg);
        return;
    }
    if (event["message"]) {
        processNarrativeMessage(context, event);
    } else if (event["system_message"]) {
        handleSystemMessage(context, event);
    } else {
        console.log("Unknown event type: " + event);
    }
}

// Text area where output will go
const OutputWindow = (player : State<Player>) => {
    return div({
        id: "output_window",
        class: "output_window"
    });
};

const InputArea = (context: Context, player : State<Player>) => {
    let hidden_style = van.derive(() => player.val.connected ? "display: block;" : "display: none;");
    const i = input({
        id: "input_area",
        style: hidden_style,
        disabled: van.derive(() => !player.val.connected),
        class: "input_area",
        onkeyup: e => {
            // Arrow up means go back in history and fill the input area with that, if there is any.
            if (e.key === "ArrowUp") {
                if (context.historyOffset < context.history.length) {
                    context.historyOffset += 1;
                    if (context.history.length - context.historyOffset >= 0) {
                        let value = context.history[context.history.length - context.historyOffset];
                        if (value) {
                            i.value = value;
                        } else {
                            i.value = "";
                        }
                    }
                }
            } else if (e.key === "ArrowDown") {
                if (context.historyOffset > 0) {
                    context.historyOffset -= 1;
                    if (context.history.length - context.historyOffset >= 0) {
                        let value = context.history[context.history.length - context.historyOffset];
                        if (value) {
                            i.value = value;
                        } else {
                            i.value = "";
                        }
                    }
                }
            } else if (e.key === "Enter") {
                // Put a copy into the narrative window, send it over websocket, and clear
                let input = i.value;
                let output = document.getElementById("output_window");
                let outine = div("> " + input);
                output.appendChild(outine);
                context.ws.send(i.value);
                i.value = "";
                // Append to history
                context.history.push(input);
                context.historyOffset = 0;
            }
        }
    });
    return div(i);
};

export const Narrative = (context: Context, player : State<Player>) => {
    let hidden_style = van.derive(() => player.val.connected ? "display: block;" : "display: none;");

    return div(
        {
            class: "narrative",
            style: hidden_style
        },
        OutputWindow(player),
        InputArea(context, player)
    );
};

