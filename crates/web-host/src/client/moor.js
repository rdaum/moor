// Converts a JSON representation of a MOO value into a MOO expression string
// JSON values look like:
//     number -> number
//     "string" -> "string"
//     { error_code: number, error_name: string (e.g. E_PROPNF), error_message: string } -> E_<error_name>
//     { oid: number } -> #<oid>
//     [ ... ] -> { ... }
function json_to_moo(json) {
    if (typeof json === "number") {
        return json.toString();
    } else if (typeof json === "string") {
        return "\"" + json + "\"";
    } else if (typeof json === "object") {
        if (json["error_code"]) {
            return json["error_name"];
        } else if (json["oid"] != null) {
            return "#" + json["oid"];
        } else if (Array.isArray(json)) {
            let result = "{";
            for (let i = 0; i < json.length; i++) {
                result += json_to_moo(json[i]);
                if (i < json.length - 1) {
                    result += ", ";
                }
            }
            result += "}";
            return result;
        } else {
            throw "Unknown object type: " + json;
        }
    } else {
        throw "Unknown JSON type: " + json;
    }
}

// Turn a list of arguments containing JSON values into a string which is a list of MOO
// values.
function transform_args(args) {
    let result = [];
    for (let i = 0; i < args.length; i++) {
        result.push(json_to_moo(args[i]));
    }
    return result.join(", ");
}

// Recursively descend a JSON result from eval, and turns object references into MooRPCObjects.
function transform_eval(json) {
    // Empty json is null, so return null.
    if (json == null) {
        return null;
    }
    if (typeof json != "object") {
        return json;
    }
    if (json["oid"] != null) {
        return new MoorRPCObject(json["oid"], context.auth_token);
    } else if (Array.isArray(json)) {
        let result = [];
        for (let i = 0; i < json.length; i++) {
            result.push(transform_eval(json[i]));
        }
        return result;
    } else {
        let result = {};
        for (let key in json) {
            result[key] = transform_eval(json[key]);
        }
        return result;
    }
}

// Object handle for a MOO object to permit simple RPC type behaviours.
class MoorRPCObject {
    constructor(object_id, auth_token) {
        this.object_id = object_id;
        this.auth_token = auth_token;
    }

    // Call a verb on the object by eval.
    // "return #<object_id>:<verb>(<args>)"
    async invoke_verb(verb_name, args) {
        let self = json_to_moo(this.object_id)
        let args_str = transform_args(args);
        let expr = "return #" + self + ":" + verb_name + "(" + args_str + ");";
        return perform_eval(this.auth_token, expr);
    }

    async get_property(property_name) {
        let self = json_to_moo(this.object_id);
        let expr = "return #" + self + "." + property_name + ";";
        return perform_eval(this.auth_token, expr);
    }
}

// Call a builtin function on the server and return the result.
async function call_builtin(auth_token, builtin, args) {
    let args_str = transform_args(args);
    let expr = "return " + builtin + "(" + args_str + ");";
    return perform_eval(auth_token, expr);
}

// Evaluate a MOO expression on the server and return the result.
async function perform_eval(auth_token, expr) {
    // HTTP POST with the body being the expression. And add in the X-Moor-Auth-Token header.
    let result = await fetch("/eval", {
        method: "POST",
        body: expr,
        headers: {
            "X-Moor-Auth-Token": context.auth_token
        }
    });
    if (result.ok) {
        let expr = await result.json();
        return transform_eval(expr);
    } else {
        console.log("Failed to evaluate expression!");
    }
}

// Global state.
let context = {
    showdown: null,
    player: null,
    auth_token: null,
    ws: null
};

// Utility function to build DOM elements from HTML.
function generateElements(html) {
    const template = document.createElement('template');
    template.innerHTML = html.trim();
    return template.content.children;
}

function write_markdown(markdown, destination, style) {
    let html = context.showdown.makeHtml(markdown);
    let elements = generateElements(html);
    while (elements.length > 0) {
        if (style) {
            elements[0].classList.add(style);
        }
        destination.appendChild(elements[0]);
    }
}

// Retrieve and display welcome message.
async function retrieve_welcome(welcome_panel) {
    let result = await fetch("/welcome");
    if (result.ok) {
        let welcome_text = await result.json();
        // "welcome_text" is a json array of strings, but we want to treat it as one markdown doc,
        // so we'll join them together with a newline.
        let welcome_markdown = welcome_text.join("\n");
        write_markdown(welcome_markdown, welcome_panel);
    } else {
        console.log("Failed to retrieve welcome text!");
    }
}

// Output a system message to the narrative panel.
function output_system_text(text) {
    let narrative = document.getElementById("narrative");
    write_markdown(text, narrative, "system_message");
    narrative.scrollTop = narrative.scrollHeight;
}

// Output a typical narrative message to the narrative panel.
function output_narrative_text(text) {
    let narrative = document.getElementById("narrative");
    write_markdown(text, narrative, "message");
    // scroll to bottom
    narrative.scrollTop = narrative.scrollHeight;
}

// Output a gap in the narrative panel, to make it easier to read.
function output_narrative_gap() {
    let anchor = document.getElementById("anchor");
    let narrative = document.getElementById("narrative");
    let msg = generateElements("<li class='gap'>&nbsp;</li>")[0];
    narrative.insertBefore(msg, anchor);
    // scroll to bottom
    narrative.scrollTop = narrative.scrollHeight;
}

// Process an inbound (JSON) event from the websocket connection to the server.
function handle_narrative_event(e) {
    // Parse event as JSON.
    let event = JSON.parse(e.data);
    if (event["message"]) {
        output_narrative_text(event["message"]);
    } else if (event["system_message"]) {
        output_system_text(event["system_message"]);
    } else {
        console.log("Unknown event type: " + event);
    }
}

// Handle the connect phase in response to a user clicking the "Connect" button.
async function connect(e) {
    e.preventDefault();
    // Pick action (/auth/connect or /auth/create) depending on the action selected.
    let action = document.getElementById("action").value;
    let url = "/auth/" + action;
    let form = e.target;
    let data = new URLSearchParams();
    data.set("player", form.player.value);
    data.set("password", form.password.value);
    let result = await fetch(url, {
        method: "POST",
        body: data
    });
    if (result.ok) {
        console.log("Connected!");
    } else {
        // TODO: we will need to capture the rejection message from the server somehow, and display it.
        //   Right now this is not possible through the existing LoginRequest/LoginResponse mechanism.
        //   Also this alert() thing is ugly, we'll want to replace it with something more UI consistent.
        console.log("Failed to connect!");
        alert("Failed to authenticate!")
        return;
    }
    let connect_form = document.getElementById("connect_form");
    let entry_form = document.getElementById("entry");
    connect_form.style.display = "none";
    entry_form.style.display = "";

    let login_result = await result.text();
    login_result = login_result.split(" ");
    let connect_type = login_result[1].toLowerCase();
    let player = login_result[0];
    let auth_token = result.headers.get("X-Moor-Auth-Token");
    if (!auth_token) {
        console.log("No token; authorization denied");
        alert("Could not authenticate!");
        return;
    }

    // Now initiate the websocket connection, attaching the header
    // with the token.
    let ws = new WebSocket("ws://localhost:8080/ws/attach/" + connect_type + "/" + auth_token);
    context.player = player;
    context.auth_token = auth_token;
    context.ws = ws;
    ws.onmessage = handle_narrative_event

    // Show the narrative panel, and hide the welcome panel.
    let welcome_span = document.getElementById("login_area");
    welcome_span.style.display = "none";
    let welcome_panel = document.getElementById("welcome");
    welcome_panel.style.display = "none";
    let narrative = document.getElementById("narrative");
    narrative.style.display = "inherit";
}

// Handle a user inputting a command.
async function handle_input(e) {
    e.preventDefault();
    context.ws.send(e.target.command.value);
    e.target.command.value = "";
    // Put a small gap in the narrative output, to make it easier to read the results.
    output_narrative_gap();
}

// Attach event handlers to our elements in the DOM.
document.addEventListener("DOMContentLoaded", function () {
    context.showdown = new showdown.Converter({
        // Permit markdown tables, strikethrough, and emoji.
        tables: true,
        strikethrough: true,
        emoji: true,
        // Require a space after the # in a heading, to prevent non-heading-intended things from being
        // interpreted as such.
        requireSpaceBeforeHeadingText: true,
        // Open links in new window to prevent this one from being clobbered
        openLinksInNewWindow: true,
        // Put a <br> after each line break, as this makes existing MOO content format correctly.
        simpleLineBreaks: true
    });

    let connect_form = document.getElementById("connect_form");
    let entry_form = document.getElementById("entry");

    connect_form.addEventListener("submit", connect);
    entry_form.addEventListener("submit", handle_input);

    let welcome_panel = document.getElementById("welcome");
    retrieve_welcome(welcome_panel);
});


