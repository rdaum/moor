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
import { curieToObjectRef, matchRef, ObjectRef, oidRef, ORefKind, sysobjRef } from "./var";

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
        // Note: authToken will need to be passed separately when using this function
        return new MoorRemoteObject(oref, "");
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
     */
    async callVerb(verbName: string, args: any[] = []): Promise<any> {
        const endpoint = `/verbs/${orefCurie(this.oref)}/${verbName}/invoke`;

        try {
            const response = await fetch(endpoint, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    "X-Moor-Auth-Token": this.authToken,
                },
                body: JSON.stringify(args),
            });

            if (response.ok) {
                const result = await response.json();
                return transformEval(result);
            } else {
                console.error(`Failed to invoke verb ${verbName}:`, response.statusText);
                throw new Error(`Verb invocation failed: ${response.status} ${response.statusText}`);
            }
        } catch (err) {
            console.error(`Exception during verb invocation for ${verbName}:`, err);
            throw new Error(`Exception during verb invocation: ${err instanceof Error ? err.message : String(err)}`);
        }
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
        const endpoint = `/verbs/${orefCurie(this.oref)}?inherited=true`;

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
            return { "error": `Exception during compilation: ${err instanceof Error ? err.message : String(err)}` };
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
        const endpoint = `/properties/${orefCurie(this.oref)}?inherited=true`;

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
            return oref.curie;

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
    if (curie.startsWith("oid:") || curie.startsWith("uuid:")) {
        return curieToObjectRef(curie);
    } else if (curie.startsWith("sysobj:")) {
        const value = curie.substring(7);
        return sysobjRef(value.split("."));
    } else if (curie.startsWith("match(\"") && curie.endsWith("\")")) {
        const value = curie.substring(7, curie.length - 2);
        return matchRef(decodeURIComponent(value));
    } else {
        throw new Error(`Unknown CURIE format: ${curie}`);
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
        throw new Error(`Exception during evaluation: ${err instanceof Error ? err.message : String(err)}`);
    }
}

/**
 * Retrieves the welcome message and content type from the server
 *
 * The welcome message is returned as an array of strings that are joined
 * together to form a single document.
 *
 * @returns Promise resolving to an object with welcomeMessage and contentType
 */
export async function retrieveWelcome(): Promise<{
    welcomeMessage: string;
    contentType: "text/plain" | "text/djot" | "text/html" | "text/traceback";
}> {
    try {
        // Fetch welcome message
        const messageResponse = await fetch("/system_property/login/welcome_message");
        let welcomeMessage = "";

        if (messageResponse.ok) {
            const welcomeText = await messageResponse.json() as string[];
            welcomeMessage = welcomeText.join("\n");
        } else {
            const errorMsg = `Failed to retrieve welcome text: ${messageResponse.status} ${messageResponse.statusText}`;
            console.error(errorMsg);
            welcomeMessage = "Welcome to mooR";
        }

        // Fetch content type
        let contentType: "text/plain" | "text/djot" | "text/html" | "text/traceback" = "text/plain";
        try {
            const typeResponse = await fetch("/system_property/login/welcome_message_content_type");
            if (typeResponse.ok) {
                const typeValue = await typeResponse.json() as string;
                // Validate the content type
                if (
                    typeValue === "text/html" || typeValue === "text/djot" || typeValue === "text/plain"
                    || typeValue === "text/traceback"
                ) {
                    contentType = typeValue;
                }
            }
            // If 404 or invalid value, default to text/plain (already set)
        } catch (error) {
            console.log("Content type not available, defaulting to text/plain:", error);
        }

        return { welcomeMessage, contentType };
    } catch (err) {
        const errorMsg = `Exception retrieving welcome text: ${err instanceof Error ? err.message : String(err)}`;
        console.error(errorMsg);
        return { welcomeMessage: "Welcome to mooR", contentType: "text/plain" };
    }
}
