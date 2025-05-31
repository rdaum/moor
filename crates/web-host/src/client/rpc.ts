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

/**
 * Remote Procedure Call (RPC) Module
 *
 * This module provides utilities for communicating with the Moor server,
 * including object reference handling, remote method invocation, and
 * data transformation between JSON and MOO formats.
 */
import { context } from "./moor";
import { matchRef, ObjectRef, oidRef, ORefKind, sysobjRef } from "./var";

/**
 * Converts a JavaScript value to its MOO string representation
 *
 * @param json - The JavaScript value to convert
 * @returns String representation in MOO format
 * @throws Error if the value cannot be converted
 */
function translateJsonToMOO(json: any): string {
    // Handle primitive types
    if (typeof json === "number") {
        return json.toString();
    } else if (typeof json === "string") {
        return "\"" + json + "\"";
    } else if (json === null) {
        return "NULL";
    } else if (json === undefined) {
        return "E_NONE";
    } else if (typeof json === "boolean") {
        return json ? "1" : "0";
    } // Handle object types
    else if (typeof json === "object") {
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
            throw new Error(`Cannot convert object to MOO format: ${JSON.stringify(json)}`);
        }
    } else {
        throw new Error(`Cannot convert ${typeof json} to MOO format`);
    }
}

/**
 * Converts an array of JavaScript values to a MOO argument list string
 *
 * @param args - Array of arguments to convert to MOO format
 * @returns Comma-separated string of MOO-formatted arguments
 */
function transformArgs(args: any[]): string {
    return args.map(arg => translateJsonToMOO(arg)).join(", ");
}

/**
 * Recursively transforms a JSON result from server eval into JavaScript objects
 *
 * Converts MOO object references into MoorRemoteObject instances, and recursively
 * processes arrays and objects to transform their contents as well.
 *
 * @param json - The JSON value to transform
 * @returns Transformed value with object references converted to MoorRemoteObject instances
 */
function transformEval(json: any): any {
    // Handle null/undefined
    if (json == null) {
        return null;
    }

    // Pass through primitive values
    if (typeof json !== "object") {
        return json;
    }

    // Convert object references to MoorRemoteObject instances
    if (json["oid"] != null) {
        const oref = oidRef(json["oid"]);
        return new MoorRemoteObject(oref, context.authToken);
    } // Process arrays recursively
    else if (Array.isArray(json)) {
        return json.map(item => transformEval(item));
    } // Process objects recursively
    else {
        const result: Record<string, any> = {};
        for (const key in json) {
            if (Object.prototype.hasOwnProperty.call(json, key)) {
                result[key] = transformEval(json[key]);
            }
        }
        return result;
    }
}

/**
 * Remote proxy for a MOO object that provides RPC-like access to methods and properties
 *
 * This class encapsulates operations that can be performed on a remote MOO object,
 * including verb invocation, property access, and code management.
 */
export class MoorRemoteObject {
    /** Reference to the MOO object */
    private readonly oref: ObjectRef;

    /** Authentication token for server requests */
    private readonly authToken: string;

    /**
     * Creates a new remote object proxy
     *
     * @param oref - Object reference identifying the MOO object
     * @param authToken - Authentication token for server requests
     */
    constructor(oref: ObjectRef, authToken: string) {
        this.oref = oref;
        this.authToken = authToken;
    }

    /**
     * Invokes a verb/method on the remote MOO object
     *
     * @param verbName - Name of the verb to call
     * @param args - Arguments to pass to the verb
     * @returns Promise resolving to the result of the verb invocation
     *
     * @todo Replace with RESTful API instead of using eval
     */
    async callVerb(verbName: string, args: any[] = []): Promise<any> {
        const self = "#" + this.oref;
        const argsStr = transformArgs(args);
        const expr = `return ${self}:${verbName}(${argsStr});`;
        return performEval(this.authToken, expr);
    }

    /**
     * Retrieves the source code for a verb/method
     *
     * @param verbName - Name of the verb to fetch
     * @returns Promise resolving to an array of code lines
     * @throws Error if the fetch operation fails
     */
    async getVerbCode(verbName: string): Promise<string[]> {
        const endpoint = `/verbs/${orefCurie(this.oref)}/${verbName}`;

        const response = await fetch(endpoint, {
            method: "GET",
            headers: {
                "X-Moor-Auth-Token": this.authToken,
            },
        });

        if (response.ok) {
            const data = await response.json();
            return data["code"];
        } else {
            console.error(`Failed to fetch verb code for ${verbName}:`, response.statusText);
            throw new Error(`Failed to fetch verb code: ${response.status} ${response.statusText}`);
        }
    }

    /**
     * Retrieves all verbs/methods defined on this object
     *
     * @returns Promise resolving to the list of verbs
     * @throws Error if the fetch operation fails
     */
    async getVerbs(): Promise<any> {
        const endpoint = `/verbs/${orefCurie(this.oref)}`;

        const response = await fetch(endpoint, {
            method: "GET",
            headers: {
                "X-Moor-Auth-Token": this.authToken,
            },
        });

        if (response.ok) {
            return await response.json();
        } else {
            console.error("Failed to fetch verbs:", response.statusText);
            throw new Error(`Failed to fetch verbs: ${response.status} ${response.statusText}`);
        }
    }

    /**
     * Compiles and updates a verb/method on the remote object
     *
     * @param verbName - Name of the verb to compile
     * @param code - Source code to compile
     * @returns Promise resolving to compilation results (empty object if successful, errors otherwise)
     */
    async compileVerb(verbName: string, code: string): Promise<Record<string, any>> {
        const endpoint = `/verbs/${orefCurie(this.oref)}/${verbName}`;

        try {
            const response = await fetch(endpoint, {
                method: "POST",
                headers: {
                    "X-Moor-Auth-Token": this.authToken,
                },
                body: code,
            });

            if (response.ok) {
                // The server may return success with compilation errors
                const resultJson = await response.json();
                if (resultJson["errors"]) {
                    return resultJson["errors"];
                } else {
                    return {}; // Success with no errors
                }
            } else {
                console.error("Failed to compile verb:", response.statusText);
                return { "error": `Failed to compile verb: ${response.status} ${response.statusText}` };
            }
        } catch (err) {
            console.error("Exception during verb compilation:", err);
            return { "error": `Exception during compilation: ${err.message}` };
        }
    }

    /**
     * Retrieves the value of a property from the remote object
     *
     * @param propertyName - Name of the property to retrieve
     * @returns Promise resolving to the property value (transformed to JavaScript equivalents)
     * @throws Error if the fetch operation fails
     */
    async getProperty(propertyName: string): Promise<any> {
        const endpoint = `/properties/${orefCurie(this.oref)}/${propertyName}`;

        const response = await fetch(endpoint, {
            method: "GET",
            headers: {
                "X-Moor-Auth-Token": this.authToken,
            },
        });

        if (response.ok) {
            const value = await response.json();
            return transformEval(value);
        } else {
            console.error(`Failed to fetch property '${propertyName}':`, response.statusText);
            throw new Error(`Failed to fetch property: ${response.status} ${response.statusText}`);
        }
    }

    /**
     * Retrieves all properties from the remote object
     *
     * @returns Promise resolving to a map of property names to values
     * @throws Error if the fetch operation fails
     */
    async getProperties(): Promise<Record<string, any>> {
        const endpoint = `/properties/${orefCurie(this.oref)}`;

        const response = await fetch(endpoint, {
            method: "GET",
            headers: {
                "X-Moor-Auth-Token": this.authToken,
            },
        });

        if (response.ok) {
            const value = await response.json();
            return transformEval(value);
        } else {
            console.error("Failed to fetch properties:", response.statusText);
            throw new Error(`Failed to fetch properties: ${response.status} ${response.statusText}`);
        }
    }
}

/**
 * Converts an ObjectRef into a canonical URI identifier (CURIE)
 *
 * @param oref - The object reference to convert
 * @returns String representation as a CURIE
 * @throws Error if the object reference type is unknown
 */
export function orefCurie(oref: ObjectRef): string {
    switch (oref.kind) {
        case ORefKind.Oid:
            return `oid:${oref.oid}`;

        case ORefKind.SysObj:
            return `sysobj:${encodeURIComponent(oref.sysobj.join("."))}`;

        case ORefKind.Match:
            return `match("${encodeURIComponent(oref.match)}")`;

        default:
            throw new Error(`Unknown ObjectRef kind: ${(oref as any).kind}`);
    }
}

/**
 * Parses a CURIE string into an ObjectRef
 *
 * @param curie - The CURIE string to parse
 * @returns The corresponding ObjectRef
 * @throws Error if the CURIE is invalid or has an unknown type
 */
export function curieORef(curie: string): ObjectRef {
    const parts = curie.split(":");

    if (parts.length !== 2) {
        throw new Error(`Invalid OREF CURIE format: ${curie}`);
    }

    const type = parts[0];
    const value = parts[1];

    switch (type) {
        case "oid":
            return oidRef(parseInt(value, 10));

        case "sysobj":
            return sysobjRef(value.split("."));

        case "match_env":
            return matchRef(value);

        default:
            throw new Error(`Unknown CURIE type: ${type}`);
    }
}

/**
 * Evaluates a MOO expression on the server and returns the result
 *
 * @param authToken - Authentication token for the request
 * @param expr - MOO expression to evaluate
 * @returns Promise resolving to the evaluated result
 * @throws Error if the evaluation fails
 */
export async function performEval(authToken: string, expr: string): Promise<any> {
    try {
        const response = await fetch("/eval", {
            method: "POST",
            body: expr,
            headers: {
                "X-Moor-Auth-Token": authToken,
            },
        });

        if (response.ok) {
            const result = await response.json();
            return transformEval(result);
        } else {
            console.error("Failed to evaluate expression:", response.statusText);
            throw new Error(`Expression evaluation failed: ${response.status} ${response.statusText}`);
        }
    } catch (err) {
        console.error("Exception during expression evaluation:", err);
        throw new Error(`Exception during evaluation: ${err.message}`);
    }
}

/**
 * Retrieves the welcome message from the server
 *
 * The welcome message is returned as an array of strings that are joined
 * together to form a single djot document.
 *
 * @returns Promise resolving to the welcome message text
 */
export async function retrieveWelcome(): Promise<string> {
    try {
        const response = await fetch("/welcome");

        if (response.ok) {
            const welcomeText = await response.json() as string[];
            // Join the array of strings into a single document with newlines
            return welcomeText.join("\n");
        } else {
            const errorMsg = `Failed to retrieve welcome text: ${response.status} ${response.statusText}`;
            console.error(errorMsg);
            context.systemMessage.show("Failed to retrieve welcome text!", 3);
            return "";
        }
    } catch (err) {
        const errorMsg = `Exception retrieving welcome text: ${err.message}`;
        console.error(errorMsg);
        context.systemMessage.show("Failed to retrieve welcome text!", 3);
        return "";
    }
}
