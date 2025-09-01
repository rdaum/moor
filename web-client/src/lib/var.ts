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

export enum ORefKind {
    Oid,
    SysObj,
    Match,
}

export interface Oid {
    kind: ORefKind.Oid;
    curie: string;
}

export interface SysObj {
    kind: ORefKind.SysObj;
    sysobj: string[];
}

export interface ObjMatch {
    kind: ORefKind.Match;
    match: string;
}

export type ObjectRef = Oid | SysObj | ObjMatch;

// Parse a JSON document representing a MOO 'Var'.
// Moor JSON common are a bit special because we have a number of types that are not a direct map.
export function jsonToValue(json: any): any {
    if (typeof json === "number") {
        return json;
    } else if (typeof json === "string") {
        return json;
    } else if (typeof json === "object") {
        if ((json as any)["error"]) {
            return new Error((json as any)["error"], (json as any)["message"]);
        } else if ((json as any)["obj"] != null) {
            return curieToObjectRef((json as any)["obj"]);
        } else if ((json as any)["map_pairs"] != null) {
            const pairs: any[] = [];
            const jsonPairs = (json as any)["map_pairs"];
            if (!Array.isArray(jsonPairs)) {
                throw "Map pairs must be an array";
            }
            for (let i = 0; i < jsonPairs.length; i++) {
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

export function valueToJson(v: any): any {
    if (typeof v === "number" || typeof v === "string") {
        return v;
    } else if (v instanceof Error) {
        return { error: v.code, message: v.message };
    } else if (v["kind"] === ORefKind.Oid) {
        return { obj: v.curie };
    } else if (v instanceof Map) {
        return { map_pairs: v.pairs.map(valueToJson) };
    } else {
        throw "Unknown object type: " + v;
    }
}

export function oidRef(oid: number): Oid {
    return { kind: ORefKind.Oid, curie: `oid:${oid}` };
}

export function uuidRef(uuid: string): Oid {
    return { kind: ORefKind.Oid, curie: `uuid:${uuid}` };
}

export function curieToObjectRef(curie: string): ObjectRef {
    if (curie.startsWith("oid:")) {
        return oidRef(parseInt(curie.substring(4), 10));
    } else if (curie.startsWith("uuid:")) {
        return uuidRef(curie.substring(5));
    } else {
        throw new Error(`Unknown CURIE format: ${curie}`);
    }
}

export function sysobjRef(sysobj: string[]): SysObj {
    return { kind: ORefKind.SysObj, sysobj: sysobj };
}

export function matchRef(match: string): ObjMatch {
    return { kind: ORefKind.Match, match: match };
}

export class Error {
    code: string;
    message: string | null;
    constructor(code: string, message: string | null) {
        this.code = code;
        this.message = message;
    }
}

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
export class Map {
    pairs: Array<[any, any]>;

    constructor(pairs: Array<[any, any]> = []) {
        this.pairs = pairs;
    }

    // Insert a key-value pair into the map, replacing the value if the key already exists, common are kept in sorted
    // order.
    // As in MOO, we are CoW friendly, so we return a new map with the new pair inserted.
    insert(key: any, value: any): Map {
        const pairs = this.pairs.slice();
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
    remove(key: any): Map {
        const pairs = this.pairs.slice();
        const i = pairs.findIndex(pair => pair[0] === key);
        if (i < 0) {
            return this;
        }
        pairs.splice(i, 1);
        return new Map(pairs);
    }

    // Get the value for a key, or undefined if the key is not in the map.
    get(key: any): any {
        const i = this.pairs.findIndex(pair => pair[0] === key);
        if (i < 0) {
            return undefined;
        }
        return this.pairs[i][1];
    }

    // Return the set of pairs
    // Return the keys in the map
    keys() {
        return this.pairs.map(pair => pair[0]);
    }

    // Return the common in the map
    values() {
        return this.pairs.map(pair => pair[1]);
    }

    // Return the number of pairs in the map
    size(): number {
        return this.pairs.length;
    }
}
