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
    let self = "#" + this.object_id;
    let args_str = transform_args(args);
    let expr = "return #" + self + ":" + verb_name + "(" + args_str + ");";
    return perform_eval(this.auth_token, expr);
  }

  async get_property(property_name) {
    let self = "#" + this.object_id;
    let expr = "return #" + self + "." + property_name + ";";
    return perform_eval(this.auth_token, expr);
  }

  async get_verbs() {
    let self = "#" + this.object_id;
    let expr = ""
      + "r = {};"
      + "verbs = verbs(" + self + "); "
      + "for v in (verbs)"
      + "  r = {@r, {v, verb_args(" + self + ", v), verb_info(" + self + ", v)}};"
      + "endfor;"
      + "return r;";
    let verbs = perform_eval(this.auth_token, expr);
    return (await verbs).map((verb) => {
      return new MoorVerb(this.object_id, verb[0], verb[1], verb[2], this.auth_token);
    });
  }

  async get_properties() {
    let self = "#" + this.object_id;
    let expr = "return properties(" + self + ");";
    return perform_eval(this.auth_token, expr);
  }

  async get_verb_code(verb_name) {
    let self = "#" + this.object_id;
    let expr = "return verb_code(" + self + ", \"" + verb_name + "\");";
    return perform_eval(this.auth_token, expr);
  }
}

// Handle for a Verb.
class MoorVerb {
  constructor(object_id, verb_name, verb_args, verb_info, auth_token) {
    this.object_id = object_id;
    this.verb_name = verb_name;
    this.verb_args = verb_args;
    this.verb_info = verb_info;
    this.auth_token = auth_token;
  }

  async get_code() {
    let self = "#" + this.object_id;
    let expr = "return verb_code(" + self + ", \"" + this.verb_name + "\");";
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
      "X-Moor-Auth-Token": context.auth_token,
    },
  });
  if (result.ok) {
    let expr = await result.json();
    return transform_eval(expr);
  } else {
    console.log("Failed to evaluate expression!");
  }
}

// Utility function to build DOM elements from HTML.
function generateElements(html) {
  const template = document.createElement("template");
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
