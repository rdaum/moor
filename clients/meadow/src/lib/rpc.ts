// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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
import { MoorVar } from "./MoorVar";
import {
    compileVerbFlatBuffer,
    getPropertiesFlatBuffer,
    getPropertyFlatBuffer,
    getVerbCodeFlatBuffer,
    getVerbsFlatBuffer,
    invokeVerbFlatBuffer,
    performEvalFlatBuffer,
} from "./rpc-fb";
import { curieToObjectRef, matchRef, ObjectRef, ORefKind, sysobjRef } from "./var";

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
     * Checks if this object reference represents an anonymous object (sigil)
     * by examining the CURIE format
     */
    private isAnonymous(): boolean {
        // Anonymous objects in the CURIE format would have specific markers
        // For now, we check if it's not a regular oid: or uuid: or sysobj:
        // In practice, anonymous objects will come through eval results marked with the anonymous flag
        return false; // Anonymous objects are only in eval results, not in object refs
    }

    /**
     * Invokes a verb/method on the remote MOO object
     *
     * @param verbName - Name of the verb to call
     * @param args - Optional FlatBuffer-encoded arguments as Uint8Array
     * @returns Promise resolving to the result of the verb invocation
     * @throws Error if the object is anonymous (cannot perform operations on anonymous objects)
     */
    async callVerb(verbName: string, args?: Uint8Array): Promise<unknown> {
        if (this.isAnonymous()) {
            throw new Error("Cannot invoke verbs on anonymous objects");
        }

        const { result } = await invokeVerbFlatBuffer(
            this.authToken,
            orefCurie(this.oref),
            verbName,
            args,
        );

        return result;
    }

    /**
     * Retrieves the source code for a verb/method
     *
     * @param verbName - Name of the verb to fetch
     * @returns Promise resolving to an array of code lines
     * @throws Error if the object is anonymous or fetch operation fails
     */
    async getVerbCode(verbName: string): Promise<string[]> {
        if (this.isAnonymous()) {
            throw new Error("Cannot retrieve code from anonymous objects");
        }

        const verbValue = await getVerbCodeFlatBuffer(this.authToken, orefCurie(this.oref), verbName);

        // Extract code from FlatBuffer VerbValue
        const codeLength = verbValue.codeLength();
        const code: string[] = [];
        for (let i = 0; i < codeLength; i++) {
            const line = verbValue.code(i);
            if (line) {
                code.push(line);
            }
        }

        return code;
    }

    /**
     * Retrieves all verbs/methods defined on this object
     *
     * @returns Promise resolving to VerbsReply FlatBuffer
     * @throws Error if the fetch operation fails
     */
    async getVerbs() {
        return await getVerbsFlatBuffer(this.authToken, orefCurie(this.oref), true);
    }

    /**
     * Compiles and updates a verb/method on the remote object
     *
     * @param verbName - Name of the verb to compile
     * @param code - Source code to compile
     * @returns Promise resolving to compilation results (empty object if successful, errors otherwise)
     * @throws Error if the object is anonymous (cannot modify anonymous objects)
     */
    async compileVerb(
        verbName: string,
        code: string,
    ): Promise<Awaited<ReturnType<typeof compileVerbFlatBuffer>> | { error: string }> {
        if (this.isAnonymous()) {
            return { "error": "Cannot compile code on anonymous objects" };
        }

        try {
            return await compileVerbFlatBuffer(
                this.authToken,
                orefCurie(this.oref),
                verbName,
                code,
            );
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
     * @throws Error if the object is anonymous or fetch operation fails
     */
    async getProperty(propertyName: string): Promise<unknown> {
        if (this.isAnonymous()) {
            throw new Error("Cannot retrieve properties from anonymous objects");
        }

        const propValue = await getPropertyFlatBuffer(this.authToken, orefCurie(this.oref), propertyName);

        // Extract property value from FlatBuffer PropertyValue
        const valueVar = propValue.value();
        if (!valueVar) {
            throw new Error(`No value found for property ${propertyName}`);
        }

        // Convert Var to JavaScript value
        const moorVar = new MoorVar(valueVar);
        return moorVar.toJS();
    }

    /**
     * Retrieves all properties from the remote object
     *
     * @returns Promise resolving to PropertiesReply FlatBuffer
     * @throws Error if the object is anonymous or fetch operation fails
     */
    async getProperties() {
        if (this.isAnonymous()) {
            throw new Error("Cannot retrieve properties from anonymous objects");
        }

        return await getPropertiesFlatBuffer(this.authToken, orefCurie(this.oref), true);
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
    const kind = oref.kind;
    switch (oref.kind) {
        case ORefKind.Oid:
            return oref.curie;

        case ORefKind.SysObj:
            return `sysobj:${encodeURIComponent(oref.sysobj.join("."))}`;

        case ORefKind.Match:
            return `match("${encodeURIComponent(oref.match)}")`;

        default:
            throw new Error(`Unknown ObjectRef kind: ${kind}`);
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
export async function performEval(authToken: string, expr: string): Promise<unknown> {
    return performEvalFlatBuffer(authToken, expr);
}
