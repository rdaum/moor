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

// Parse a JSON document representing a MOO 'Var'.
// Moor JSON common are a bit special because we have a number of types that are not a direct map.

// Represents a MOO 'map' which is a list of key-value pairs in sorted order and binary search for keys.
// (We cannot use a JavaScript object because the keys are potentially-not strings.)
// - Maps are not supported in JSON serialization, so we have to encode them as a list of pairs,
//   with a tag to indicate that it's a map.
// - Object references are encoded as a JSON object with a tag to indicate the type of reference.
//      { oid: 1234 }
// - Errors are encoded as a JSON object with a tag to indicate the type of error, and an optional description.
//      { error: "E_PROPNF", message: "Property not found" }
// - Lists are encoded as JSON arrays.
// - Strings are encoded as JSON strings.
// - Integers & floats are encoded as JSON numbers, but there's a caveat here that JSON's spec
//   can't permit a full 64-bit integer, so we have to be careful about that.
// - Future things like WAIFs, etc. will need to be encoded in a way that makes sense for JSON.

export function jsonToValue(json) {
  if (typeof json === "number") {
    return new Int(json);
  } else if (typeof json === "string") {
    return new Str(json);
  } else if (typeof json === "object") {
    if (json["error"]) {
      return new Error(json["error"], json["message"]);
    } else if (json["oid"] != null) {
      return new ObjectRef(json["oid"]);
    } else if (json["map_pairs"] != null) {
      let pairs = [];
      let jsonPairs = json["map_pairs"];
      if (!Array.isArray(jsonPairs)) {
        throw "Map pairs must be an array";
      }
      for (let i = 0; i < json.length; i++) {
        pairs.push(jsonToValue(jsonPairs[i]));
      }
      return new Map(pairs);
    } else {
      throw "Unknown object type: " + json;
    }
  } else {
    throw "Unknown JSON type: " + json;
  }
}

export function valueToJson(v) {
  if (v instanceof Int) {
    return v.value;
  } else if (v instanceof Str) {
    return v.value;
  } else if (v instanceof Error) {
    return { error: v.code, message: v.message };
  } else if (v instanceof ObjectRef) {
    return { oid: v.oid };
  } else if (v instanceof Map) {
    return { map_pairs: v.pairs.map(valueToJson) };
  } else {
    throw "Unknown object type: " + v;
  }
}

// An ObjectRef can be one of:
//      .oid: number - literal object id
//      .sysobj: array of strings - system object reference, a "path" starting from #0.  $login.welcome_message
//      .match: string - a string to match in the player's current environment (room)

export class ObjectRef {
  constructor(oid, sysobj = null, match = null) {
    this.oid = oid;
    this.sysobj = sysobj;
    this.match_env = match;
  }
}

export function oidRef(oid) {
  return new ObjectRef(oid);
}

export function sysobjRef(sysobj) {
  return new ObjectRef(null, sysobj);
}

export function matchRef(match) {
  return new ObjectRef(null, null, match);
}

export class Error {
  constructor(code, message) {
    this.code = code;
    this.message = message;
  }
}

export class Map {
  constructor(pairs = []) {
    this.pairs = pairs;
  }

  // Insert a key-value pair into the map, replacing the value if the key already exists, common are kept in sorted
  // order.
  // As in MOO, we are CoW friendly, so we return a new map with the new pair inserted.
  insert(key, value) {
    let pairs = this.pairs.slice();
    let i = pairs.findIndex(pair => pair[0] >= key);
    if (i < 0) {
      i = pairs.length;
    } else if (pairs[i][0] === key) {
      pairs[i] = [key, value];
      return new Map(pairs);
    }
    pairs.splice(i, 0, [key, value]);
    return new Map(pairs);
  }

  // Remove a key-value pair from the map, returning a new map with the pair removed.
  remove(key) {
    let pairs = this.pairs.slice();
    let i = pairs.findIndex(pair => pair[0] === key);
    if (i < 0) {
      return this;
    }
    pairs.splice(i, 1);
    return new Map(pairs);
  }

  // Get the value for a key, or undefined if the key is not in the map.
  get(key) {
    let i = this.pairs.findIndex(pair => pair[0] === key);
    if (i < 0) {
      return undefined;
    }
    return this.pairs[i][1];
  }

  // Return the set of pairs
  pairs() {
    return this.pairs;
  }

  // Return the keys in the map
  keys() {
    return this.pairs.map(pair => pair[0]);
  }

  // Return the common in the map
  values() {
    return this.pairs.map(pair => pair[1]);
  }

  // Return the number of pairs in the map
  size() {
    return this.pairs.length;
  }
}