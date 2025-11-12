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

import { ObjId } from "../generated/moor-common/obj-id.js";
import { UuObjId } from "../generated/moor-common/uu-obj-id.js";

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
        throw new Error(`Unknown CURIE format: ${curie}`, "invalid_curie");
    }
}

export function sysobjRef(sysobj: string[]): SysObj {
    return { kind: ORefKind.SysObj, sysobj: sysobj };
}

export function matchRef(match: string): ObjMatch {
    return { kind: ORefKind.Match, match: match };
}

/**
 * Convert UuObjId packed value to UUID string representation
 * Matches Rust implementation in crates/var/src/obj.rs
 *
 * Format: FFFFFF-FFFFFFFFFF
 * - First group (6 hex): autoincrement (16 bits) << 6 | rng (6 bits)
 * - Second group (10 hex): epoch_ms (40 bits)
 */
export function uuObjIdToString(packedValue: bigint): string {
    // Extract components from packed 62-bit value
    // autoincrement: top 16 bits
    // rng: next 6 bits
    // epoch_ms: bottom 40 bits
    const autoincrement = Number((packedValue >> 46n) & 0xFFFFn);
    const rng = Number((packedValue >> 40n) & 0x3Fn);
    const epochMs = Number(packedValue & 0xFFFFFFFFFFn);

    // Format: FFFFFF-FFFFFFFFFF
    const firstGroup = ((autoincrement << 6) | rng).toString(16).toUpperCase().padStart(6, "0");
    const secondGroup = epochMs.toString(16).toUpperCase().padStart(10, "0");
    return `${firstGroup}-${secondGroup}`;
}

/**
 * Extract object ID string from FlatBuffer Obj union
 * Returns numeric ID for ObjId, UUID string for UuObjId, null for other types
 */
export function objToString(obj: any): string | null {
    if (!obj) return null;

    // ObjUnion enum values
    const ObjUnion = {
        NONE: 0,
        ObjId: 1,
        UuObjId: 2,
        AnonymousObjId: 3,
    };

    const objType = obj.objType();

    // ObjId type - numeric ID (format: 123)
    if (objType === ObjUnion.ObjId) {
        const objId = obj.obj(new ObjId());
        return objId ? objId.id().toString() : null;
    }

    // UuObjId type - UUID-based ID (format: FFFFFF-FFFFFFFFFF)
    if (objType === ObjUnion.UuObjId) {
        const uuObjId = obj.obj(new UuObjId());
        if (uuObjId) {
            const packedValue = uuObjId.packedValue();
            return uuObjIdToString(packedValue);
        }
    }

    // AnonymousObjId can't come over RPC
    return null;
}

/**
 * Convert FlatBuffer Obj union to CURIE format (oid:N or uuid:xxx)
 * Returns CURIE string for ObjId/UuObjId, null for other types
 */
export function objToCurie(obj: any): string | null {
    if (!obj) return null;

    // ObjUnion enum values
    const ObjUnion = {
        NONE: 0,
        ObjId: 1,
        UuObjId: 2,
        AnonymousObjId: 3,
    };

    const objType = obj.objType();

    // ObjId type - return oid:N
    if (objType === ObjUnion.ObjId) {
        const objId = obj.obj(new ObjId());
        return objId ? `oid:${objId.id()}` : null;
    }

    // UuObjId type - return uuid:FFFFFF-FFFFFFFFFF
    if (objType === ObjUnion.UuObjId) {
        const uuObjId = obj.obj(new UuObjId());
        if (uuObjId) {
            const packedValue = uuObjId.packedValue();
            return `uuid:${uuObjIdToString(packedValue)}`;
        }
    }

    // AnonymousObjId can't come over RPC
    return null;
}

/**
 * Convert string object ID to CURIE format
 * Takes output from objToString() and converts to proper CURIE
 *
 * @param objStr - Object ID string (e.g., "123" or "000991-9A750B6A58" or "#123" or "oid:123")
 * @returns CURIE string (e.g., "oid:123" or "uuid:000991-9A750B6A58")
 */
export function stringToCurie(objStr: string): string {
    if (!objStr) return "oid:-1";

    // Strip leading # if present
    let stripped = objStr.startsWith("#") ? objStr.substring(1) : objStr;

    // If it has a colon, extract the ID part and revalidate
    // (handles incorrectly-prefixed CURIEs like "oid:0000A9-9A755A4762")
    if (stripped.includes(":")) {
        const parts = stripped.split(":");
        if (parts.length === 2) {
            stripped = parts[1]; // Get the ID part after the prefix
        }
    }

    // UUID format: XXXXXX-XXXXXXXXXX (6 hex chars, dash, 10 hex chars)
    if (stripped.length === 17 && stripped[6] === "-" && /^[0-9A-Fa-f]{6}-[0-9A-Fa-f]{10}$/.test(stripped)) {
        return `uuid:${stripped}`;
    }

    // Numeric ID
    return `oid:${stripped}`;
}

export class Error {
    code: string;
    message: string | null;
    constructor(code: string, message: string | null) {
        this.code = code;
        this.message = message;
    }
}
