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

import {matchRef, ObjectRef, oidRef, ORefKind, sysobjRef} from "./var";
import {context} from "./moor";

function translateJsonToMOO(json : any): any {
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
        result += translateJsonToMOO(json[i]);
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

// Turn a list of arguments containing JSON common into a string which is a list of MOO
// common.
function transform_args(args) {
  let result = [];
  for (let i = 0; i < args.length; i++) {
    result.push(translateJsonToMOO(args[i]));
  }
  return result.join(", ");
}

// Recursively descend a JSON result from eval, and turns object references into MooRPCObjects.
function transformEval(json) {
  // Empty json is null, so return null.
  if (json == null) {
    return null;
  }
  if (typeof json != "object") {
    return json;
  }
  if (json["oid"] != null) {
    let oref = oidRef(json["oid"]);
    return new MoorRemoteObject(oref, context.authToken);
  } else if (Array.isArray(json)) {
    let result = [];
    for (let i = 0; i < json.length; i++) {
      result.push(transformEval(json[i]));
    }
    return result;
  } else {
    let result = {};
    for (let key in json) {
      result[key] = transformEval(json[key]);
    }
    return result;
  }
}

// Object handle for a MOO object to permit simple RPC type behaviours.
export class MoorRemoteObject {
  oref: ObjectRef;
  authToken: string;

  constructor(oref : ObjectRef, authToken: string) {
    this.oref = oref;
    this.authToken = authToken;
  }

  // Call a verb on the object by eval.
  // "return #<object_id>:<verb>(<args>)"
  // TODO: replace with use of RESTful API.
  async callVerb(verb_name, args) {
    let self = "#" + this.oref;
    let args_str = transform_args(args);
    let expr = "return " + self + ":" + verb_name + "(" + args_str + ");";
    return performEval(this.authToken, expr);
  }

  // Get the code and property value of a verb.
  async getVerbCode(verb_name) : Promise<string> {
    // REST resource /verbs/#object_id/verb_name
    let result = await fetch("/verbs/" + orefCurie(this.oref) + "/" + verb_name, {
      method: "GET",
      headers: {
        "X-Moor-Auth-Token": this.authToken,
      },
    });
    if (result.ok) {
      let code = await result.json();
      return code["code"];
    } else {
      console.log("Failed to fetch verb code!");
    }
  }

  async getVerbs() {
    // REST resource /verbs/#object_id
    let result = await fetch("/verbs/" + orefCurie(this.oref), {
      method: "GET",
      headers: {
        "X-Moor-Auth-Token": this.authToken,
      },
    });
    if (result.ok) {
      let verbs = await result.json();
      return verbs;
    } else {
      console.log("Failed to fetch verbs!");
    }
  }

  async compileVerb(verb_name, code) : Promise<object> {
    // REST post /verbs/#object_id/verb_name
    let result = await fetch("/verbs/" + orefCurie(this.oref) + "/" + verb_name, {
      method: "POST",
      headers: {
        "X-Moor-Auth-Token": this.authToken,
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
        return {};
      }
    } else {
      console.log("Failed to compile verb!");
      return { "error": "Failed to compile verb!" };
    }
  }

  async getProperty(property_name) {
    // /properties/#object_id/property_name
    let result = await fetch("/properties/" + orefCurie(this.oref) + "/" + property_name, {
      method: "GET",
      headers: {
        "X-Moor-Auth-Token": this.authToken,
      },
    });
    if (result.ok) {
      let value = await result.json();
      return transformEval(value);
    } else {
      console.log("Failed to fetch property value!");
    }
  }

  async getProperties() {
    // /properties/object_id
    let result = await fetch("/properties/" + orefCurie(this.oref), {
      method: "GET",
      headers: {
        "X-Moor-Auth-Token": this.authToken,
      },
    });
    if (result.ok) {
      let value = await result.json();
      return transformEval(value);
    } else {
      console.log("Failed to fetch property value!");
    }
  }
}

// Construct a CURI from an object ref
export function orefCurie(oref : ObjectRef) : string {
  if (oref.kind == ORefKind.Oid) {
    return "oid:" + oref.oid;
  }

  if (oref.kind == ORefKind.SysObj) {
    return "sysobj:" + encodeURIComponent(oref.sysobj.join("."));
  }

  if (oref.kind == ORefKind.Match) {
    return "match(\"" + encodeURIComponent(oref.match) + "\")";
  }
}

export function curieORef(curie) {
  let parts = curie.split(":");
  if (parts.length != 2) {
    throw "Invalid OREF CURI: " + curie;
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

// Evaluate a MOO expression on the server and return the result.
export async function performEval(auth_token, expr) {
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
    return transformEval(expr);
  } else {
    console.log("Failed to evaluate expression!");
  }
}

export async function retrieveWelcome() {
  let result = await fetch("/welcome");
  if (result.ok) {
    let welcome_text = await result.json();
    // "welcome_text" is a json array of strings, but we want to treat it as one djot doc,
    // so we'll join them together with a newline.
    return welcome_text.join("\n");
  } else {
    console.log("Failed to retrieve welcome text!");
    context.systemMessage.show("Failed to retrieve welcome text!", 3);
    return "";
  }
}