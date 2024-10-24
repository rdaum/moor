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

// import {ObjectRef} from "./var";

// Converts a JSON representation of a MOO value into a MOO expression string
// JSON values look like:
//     number -> number
//     "string" -> "string"
//     { error_code: number, error_name: string (e.g. E_PROPNF), error_message: string } -> E_<error_name>
//     { oid: number } -> #<oid>
//     [ ... ] -> { ... }
import {oidRef, matchRef, ObjectRef } from "./var.js";

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
    let oref = ObjectRef(json["oid"]);
    return new MoorRPCObject(oref, context.auth_token);
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
export class MoorRPCObject {
  constructor(oref, auth_token) {
    this.oref = oref;
    this.auth_token = auth_token;
  }

  // Call a verb on the object by eval.
  // "return #<object_id>:<verb>(<args>)"
  async invoke_verb(verb_name, args) {
    let self = "#" + this.oref;
    let args_str = transform_args(args);
    let expr = "return " + self + ":" + verb_name + "(" + args_str + ");";
    return perform_eval(this.auth_token, expr);
  }

  // Get the code and property value of a verb.
  async get_verb_code(verb_name) {
    // REST resource /verbs/#object_id/verb_name
    let result = await fetch("/verbs/" + oref_curie(this.oref) + "/" + verb_name, {
      method: "GET",
      headers: {
        "X-Moor-Auth-Token": this.auth_token,
      },
    });
    if (result.ok) {
      let code = await result.json();
      return code["code"];
    } else {
      console.log("Failed to fetch verb code!");
    }
  }

  async get_verbs() {
    // REST resource /verbs/#object_id
    let result = await fetch("/verbs/" + oref_curie(this.oref), {
      method: "GET",
      headers: {
        "X-Moor-Auth-Token": this.auth_token,
      },
    });
    if (result.ok) {
      let verbs = await result.json();
      return verbs;
    } else {
      console.log("Failed to fetch verbs!");
    }
  }

  async compile_verb(verb_name, code) {
    // REST post /verbs/#object_id/verb_name
    let result = await fetch("/verbs/" + oref_curie(this.oref) + "/" + verb_name, {
      method: "POST",
      headers: {
        "X-Moor-Auth-Token": this.auth_token,
      },
      body: code,
    });
    if (result.ok) {
      // ok can be either with or without compile errors.  if the json has "errors" then it failed, and
      // we return that, otherwise return empty array.
      let result_json = await result.json();
      if (result_json["errors"]) {
        return result_json["errors"];
      } else {
        return [];
      }
    } else {
      console.log("Failed to compile verb!");
      return false;
    }
  }

  async get_property(property_name) {
    // /properties/#object_id/property_name
    let result = await fetch("/properties/" + oref_curie(this.oref) + "/" + property_name, {
      method: "GET",
      headers: {
        "X-Moor-Auth-Token": this.auth_token,
      },
    });
    if (result.ok) {
      let value = await result.json();
      return transform_eval(value);
    } else {
      console.log("Failed to fetch property value!");
    }
  }

  async get_properties() {
    // /properties/object_id
    let result = await fetch("/properties/" + oref_curie(this.oref), {
      method: "GET",
      headers: {
        "X-Moor-Auth-Token": this.auth_token,
      },
    });
    if (result.ok) {
      let value = await result.json();
      return transform_eval(value);
    } else {
      console.log("Failed to fetch property value!");
    }
  }
}

// Construct a CURI from an object ref
export function oref_curie(oref) {
  if (oref.oid != null) {
    return "oid:" + oref.oid;
  }

  if (oref.sysobj != null) {
      return "sysobj:" + encodeURIComponent(oref.sysobj.join("."));
  }

  if (oref.match_env != null) {
      return "match_env:" + encodeURIComponent(oref.match);
  }
}

export function curie_oref(curie) {
    let parts = curie.split(":");
    if (parts.length != 2) {
        throw "Invalid OREF CURI: " + curie
    }

    if (parts[0] == "oid") {
        return oidRef(parseInt(parts[1]));
    }

    if (parts[0] == "sysobj") {
        return sysobjRef(parts[1].split("."));
    }

    if (parts[0] == "match_env") {
        return matchRef(parts[1]);
    }

    throw "Unknown CURI type: " + parts[0];
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
      "X-Moor-Auth-Token": auth_token,
    },
  });
  if (result.ok) {
    let expr = await result.json();
    return transform_eval(expr);
  } else {
    console.log("Failed to evaluate expression!");
  }
}
