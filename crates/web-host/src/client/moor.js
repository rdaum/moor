// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

import * as monaco from 'https://cdn.jsdelivr.net/npm/monaco-editor@0.50.0/+esm';

const {button, div, input, select, option, br, pre, form, a} = van.tags

import {createEditor, updateEditor} from "./editor.js";


export const context = {
    ws: null,
    // The history of commands entered by the user.
    history: [],
    // The current position in the history, relative from end. E.g. 0 is no history, 1 is the last command, etc.
    // Reset to 0 after each entry.
    history_offset: 0,
    auth_token: null,
}

// Utility function to build DOM elements from HTML.
function generateElements(html) {
    const template = document.createElement("template");
    template.innerHTML = html.trim();
    return template.content.children;
}


async function retrieveWelcome() {
    let result = await fetch("/welcome");
    if (result.ok) {
        let welcome_text = await result.json();
        // "welcome_text" is a json array of strings, but we want to treat it as one djot doc,
        // so we'll join them together with a newline.
        let welcome_joined = welcome_text.join("\n");
        return welcome_joined;
    } else {
        console.log("Failed to retrieve welcome text!");
        context.sys_msg.show({message: "Unavailable"});
        return "";
    }
}

const Displaydjot = ({djot_text}) => {
    let ast = djot.parse(djot_text.val);
    let html = djot.renderHTML(ast);
    let elements = generateElements(html);
    let d = div();
    for (let element of elements) {
        d.appendChild(element);
    }
    return d
}


class Player {
    constructor(name, auth_token, connected) {
        this.name = name;
        this.auth_token = auth_token;
        this.connected = connected
    }
}

// Handle the connect phase in response to a user clicking the "Connect" button.
async function connect(player, mode, username, password) {
    console.log("Connecting...");
    let url = "/auth/" + mode;
    let data = new URLSearchParams();
    data.set("player", username.value);
    data.set("password", password.value);
    let result = await fetch(url, {
        method: "POST",
        body: data
    });
    if (result.ok) {
        console.log("Connected!");
    } else {
        console.log("Failed to connect!");
        context.sys_msg.show({message: "Failed to connect!", durationSec: 3});
        return;
    }
    let login_result = await result.text();
    login_result = login_result.split(" ");
    let player_name = login_result[0];
    let auth_token = result.headers.get("X-Moor-Auth-Token");
    if (!auth_token) {
        console.log("No token; authorization denied");
        alert("Could not authenticate!");
        return;
    }

    // Authorized but not connected.
    player.val = new Player(player_name, auth_token, false);

    // Now initiate the websocket connection.
    let ws = new WebSocket("ws://localhost:8080/ws/attach/" + mode + "/" + auth_token);
    ws.onopen = () => {
        console.log("Connected to server!");
        player.val = new Player(player_name, auth_token, true);
    };
    ws.onmessage = (e) => {
        if (e.data) {
            handle_narrative_event(e.data);
        }
    }
    ws.onclose = () => {
        console.log("Connection closed!");
        player.val = new Player(player_name, auth_token, false)
    }

    context.ws = ws
    context.auth_token = auth_token

    // Move focus to input area.
    document.getElementById("input_area").focus();
}

// A login box that prompts the user for their player name and password, and then initiates login through
// /auth/connect (if connecting) or /auth/create (if creating).
const Login = (player, login_message) => {
    const mode_select = select({id: "mode_select"}, option("connect"), option("create"));
    let connect_callback = () => connect(player, mode_select.value, username, password);
    const welcome = van.derive(() => Displaydjot({djot_text: login_message}));

    const username = input({
        type: "text",
        onkeyup: (e) => {
            if (e.key === "Enter") {
                connect_callback();
            }
        }
    });
    const password = input({
        type: "password",
        onkeyup: (e) => {
            if (e.key === "Enter") {
                connect_callback();
            }
        }
    });
    const go_button = button({onclick: connect_callback}, "Connect");
    return FloatingWindow(
        {
            parentDom: document.body,
            title: "Login",
            closeCross: null,
            closed: van.derive(() => player.val.connected),
            width: "300px",
            height: "300px",
        },
        div(
            welcome, mode_select, username, password, go_button
        )
    );
}

export function action_invoke(author, verb, argument) {
    console.log("Invoking " + verb + " on " + author);
    let mrpc_object = new MoorRPCObject(author, module.context.auth_token);
    let verb_arguments = [];
    if (argument) {
        verb_arguments.push(argument);
    }
    mrpc_object.invoke_verb(verb, verb_arguments).then((result) => {
        console.log("Result: " + result);
    });
}

async function compile_verb(object, verb, code, compile_state) {
    console.log("do compile: " + object + ":" + verb);
    let mrpc_object = new MoorRPCObject(object, module.context.auth_token);
    let result = await mrpc_object.compile_verb(verb, code);
    console.log("Compile result: ", result);
    if (result) {
        let result_text = result.join("\n");
        console.log("Compile error: " + result_text);
        return result_text;
    } else {
        return null;
    }
}

export function action_edit_verb(object, verb) {
    // First things first, retrieve the verb.
    let mrpc_object = new MoorRPCObject(object, module.context.auth_token);
    let vc = mrpc_object.get_verb_code(verb).then((result) => {
        console.log("Verb code: " + result);
        let title = "Verb: #" + object + ":" + verb;

        let editor_state =  van.state({model: null});
        let compile_error_state = van.state(null);

        // Where the monaco editor itself lives.
        let editor_div = div(
            {
                style: "width: 100%; height: 100%;"
            }
        );

        let hidden_style = van.derive(() =>  compile_error_state.val) ?
            "width: 100%; height: 64px; display: block;" :
            "width: 100%; height: 0px; display: none;";
        let compile_errors = div(
            {
                style: hidden_style
            },
            () => pre(compile_error_state.val)
        )

        // Surrounding container with compile button and whatever else we might need
        let container_div = div (
            button(
                {
                    onclick: async () => {
                        compile_error_state.val = await compile_verb(object, verb, editor_state.val.model.getValue());
                    }
                },
                "Compile",
            ),
            compile_errors,
            editor_div,
        )


        let editor = div(
            FloatingWindow(
                {
                    parentDom: document.body,
                    title: title,
                    id: "editor",
                    width: 500,
                    height: 300,
                }, container_div));
        document.body.appendChild(editor);

        // Now hang the editor off it.
        let model = createEditor(editor_div);
        editor_state.val = {model: model};
        updateEditor(model, result);
    })
}

// Override link behaviour for djot to only permit inline links that refer to object verbs.
// These get turned into requests to invoke the verb with the player's permissions.
function djotRender(author, ast) {
    return djot.renderHTML(ast, {
        overrides: {
            link: (node, renderer) => {
                console.log("Link node: ", node);
                let destination = node.destination;

                // Destination structures:
                //   invoke an action verb on the author of the message with an optional argument
                //      invoke:<verb>[:arg]
                //   retrieve verb contents and bring up editor with it.
                //   is invoked: <compile_command> object:verb on save
                //      edit_verb:<object>:<verb>
                //   retrieve property contents and bring up editor with it.
                //   is invoked: <set_command> object.prop on save
                //      edit_prop:<object>:<prop>:


                let spec = destination.split(":");

                if (spec.length > 3) {
                    console.log("Invalid destination: " + destination);
                    return "";
                }

                // Handle invoke:
                var function_invoke;
                if (spec[0] == "invoke") {
                    let verb = spec[1];
                    // If there's an argument, it's the second element.
                    let arg = JSON.stringify(spec[2]) || "null";

                    // Turns into a javascript: link that will invoke the verb on the object.
                    function_invoke = "module.action_invoke(\"" + author + "\", \"" + verb + "\", " + arg + ")";

                } else if (spec[0] == "edit_verb") {
                    // TODO validate object and verb names
                    let object = spec[1];
                    let verb = spec[2];

                    function_invoke = "module.action_edit_verb(\"" + object + "\", \"" + verb + "\")";
                } else if (spec[0] == "edit_prop") {
                    // TODO validate object and prop names
                    let object = spec[1];
                    let prop = spec[2];

                    function_invoke = "module.action_edit_prop(\"" + object + "\", \"" + prop + "\")";
                } else {
                    console.log("Unknown action: " + destination);
                    return "";
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
    })
}

function handle_narrative_msg(msg) {
    // Msg may have content_type attr, and if so, check it, or default to text/plain
    let content_type = msg["content_type"] || "text/plain";

    let output = document.getElementById("output_window");

    let author = msg["author"];
    let content = msg["message"];
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
        let html = djotRender(author, ast);
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

function handle_system_message(msg) {
    let content = msg["system_message"];
    context.sys_msg.show({message: content, durationSec: 3});
}

// Process an inbound (JSON) event from the websocket connection to the server.
function handle_narrative_event(msg) {
    // Parse event as JSON.
    console.log("Event: " + msg);
    let event = JSON.parse(msg);
    if (!event) {
        console.log("No event data in message: " + msg);
        return;
    }
    if (event["message"]) {
        handle_narrative_msg(event);
    } else if (event["system_message"]) {
        handle_system_message(event);
    } else {
        console.log("Unknown event type: " + event);
    }
}


// Text area where output will go
const OutputWindow = (player) => {
    console.log("OutputWindow called");
    return div({
        id: "output_window",
        class: "output_window",
    });
}

const InputArea = (player) => {
    let hidden_style = van.derive(() => player.val.connected ? "display: block;" : "display: none;");
    let total_style = van.derive(() => "width: 100%; " + hidden_style);
    const i = input({
        id: "input_area",
        style: total_style,
        disabled: van.derive(() => !player.val.connected),
        onkeyup: e => {
            // Arrow up means go back in history and fill the input area with that, if there is any.
            if (e.key === "ArrowUp") {
                if (context.history_offset < context.history.length) {
                    context.history_offset += 1;
                    if (context.history.length - context.history_offset >= 0) {
                        let value = context.history[context.history.length - context.history_offset];
                        if (value) {
                            i.value = value;
                        } else {
                            i.value = "";
                        }
                    }
                }
            } else if (e.key == "ArrowDown") {
                if (context.history_offset > 0) {
                    context.history_offset -= 1;
                    if (context.history.length - context.history_offset >= 0) {
                        let value = context.history[context.history.length - context.history_offset];
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
                context.history_offset = 0;
            }
        }
    });
    return div(i);
}

const Hello = () => {
    const player = van.state(new Player("", "", false))
    const welcome_message = van.state("")
    van.derive(() => {
        retrieveWelcome().then((msg) => {
            welcome_message.val = msg
        })
    })

    const playerName = van.derive(() => player.val.name)
    const connected = van.derive(() => player.val.connected)
    const dom = div()
    const sys_msg = new MessageBoard({
        top: "20px"
    })
    context.sys_msg = sys_msg;
    return div(
        dom,
        Login(player, welcome_message),
        div("Player: ", playerName),
        // TODO: indicator light for connection status, not text
        div("Connected: ", connected),
        OutputWindow(player),
        InputArea(player)
    )
}

van.add(document.body, Hello())


