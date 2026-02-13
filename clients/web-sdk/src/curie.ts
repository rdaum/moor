// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Lesser General Public License as published by the Free Software Foundation,
// version 3 or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Lesser General Public License for more
// details.
//
// You should have received a copy of the GNU Lesser General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

import { ObjId } from "@moor/schema/generated/moor-common/obj-id";
import { UuObjId } from "@moor/schema/generated/moor-common/uu-obj-id";

const UUID_RE = /^[0-9A-Fa-f]{6}-[0-9A-Fa-f]{10}$/;

export type ParsedCurie = { kind: "oid"; oid: number } | { kind: "uuid"; uuid: string };

export function parseObjectCurie(curie: string): ParsedCurie {
    if (curie.startsWith("oid:")) {
        const oid = parseInt(curie.slice(4), 10);
        if (Number.isNaN(oid)) {
            throw new Error(`Invalid oid in CURIE: ${curie}`);
        }
        return { kind: "oid", oid };
    }

    if (curie.startsWith("uuid:")) {
        const uuid = curie.slice(5);
        if (!UUID_RE.test(uuid)) {
            throw new Error(`Invalid uuid in CURIE: ${curie}`);
        }
        return { kind: "uuid", uuid: uuid.toUpperCase() };
    }

    throw new Error(`Unsupported CURIE format: ${curie}`);
}

/**
 * Convert UuObjId packed value to UUID string representation.
 * Format: FFFFFF-FFFFFFFFFF
 */
export function uuObjIdToString(packedValue: bigint): string {
    const autoincrement = Number((packedValue >> 46n) & 0xFFFFn);
    const rng = Number((packedValue >> 40n) & 0x3Fn);
    const epochMs = Number(packedValue & 0xFFFFFFFFFFn);

    const firstGroup = ((autoincrement << 6) | rng).toString(16).toUpperCase().padStart(6, "0");
    const secondGroup = epochMs.toString(16).toUpperCase().padStart(10, "0");
    return `${firstGroup}-${secondGroup}`;
}

/**
 * Parse UUID string representation (FFFFFF-FFFFFFFFFF) into packed UuObjId bigint.
 */
export function parseUuObjIdString(uuidStr: string): bigint {
    if (!UUID_RE.test(uuidStr)) {
        throw new Error(`Invalid UUID format: ${uuidStr}`);
    }

    const [first, second] = uuidStr.split("-");
    const firstGroup = parseInt(first, 16);
    const epochMs = BigInt(`0x${second}`);

    const autoincrement = BigInt(firstGroup >> 6);
    const rng = BigInt(firstGroup & 0x3F);
    return (autoincrement << 46n) | (rng << 40n) | epochMs;
}

export function objToString(obj: any): string | null {
    if (!obj) {
        return null;
    }

    const objType = obj.objType();
    if (objType === 1) { // ObjUnion.ObjId
        const objId = obj.obj(new ObjId());
        return objId ? objId.id().toString() : null;
    }

    if (objType === 2) { // ObjUnion.UuObjId
        const uuObjId = obj.obj(new UuObjId());
        if (!uuObjId) {
            return null;
        }
        return uuObjIdToString(uuObjId.packedValue());
    }

    return null;
}

export function objToCurie(obj: any): string | null {
    if (!obj) {
        return null;
    }

    const objType = obj.objType();
    if (objType === 1) { // ObjUnion.ObjId
        const objId = obj.obj(new ObjId());
        return objId ? `oid:${objId.id()}` : null;
    }

    if (objType === 2) { // ObjUnion.UuObjId
        const uuObjId = obj.obj(new UuObjId());
        if (!uuObjId) {
            return null;
        }
        return `uuid:${uuObjIdToString(uuObjId.packedValue())}`;
    }

    return null;
}

export function stringToCurie(objStr: string): string {
    if (!objStr) {
        return "oid:-1";
    }

    let stripped = objStr.startsWith("#") ? objStr.slice(1) : objStr;
    if (stripped.includes(":")) {
        const parts = stripped.split(":");
        if (parts.length === 2) {
            stripped = parts[1];
        }
    }

    if (UUID_RE.test(stripped)) {
        return `uuid:${stripped.toUpperCase()}`;
    }
    return `oid:${stripped}`;
}

/**
 * Convert a JS object-ref shape (typically decoded from MoorVar) into a CURIE.
 * Accepted forms:
 * - number/integer oid
 * - "#123", "oid:123"
 * - { oid: 123 }
 * - canonical uuid strings with or without "uuid:" prefix
 * - packed uuobjid bigint/decimal string in { uuid: ... }
 */
export function jsObjectRefToCurie(value: unknown): string | null {
    if (value === null || value === undefined) {
        return null;
    }

    if (typeof value === "number" && Number.isInteger(value)) {
        return `oid:${value}`;
    }

    if (typeof value === "string") {
        const raw = value.trim();
        if (!raw) {
            return null;
        }
        if (raw.startsWith("oid:")) {
            return parseObjectCurie(raw).kind === "oid" ? raw : null;
        }
        if (raw.startsWith("uuid:")) {
            try {
                const parsed = parseObjectCurie(raw);
                return parsed.kind === "uuid" ? `uuid:${parsed.uuid}` : null;
            } catch {
                return null;
            }
        }
        if (/^#\d+$/.test(raw)) {
            return `oid:${raw.slice(1)}`;
        }
        if (/^\d+$/.test(raw)) {
            return `oid:${raw}`;
        }
        if (UUID_RE.test(raw)) {
            return `uuid:${raw.toUpperCase()}`;
        }
        return null;
    }

    if (typeof value !== "object" || Array.isArray(value)) {
        return null;
    }

    const candidate = value as { oid?: unknown; uuid?: unknown };
    if (candidate.oid !== undefined && candidate.oid !== null) {
        return jsObjectRefToCurie(candidate.oid);
    }
    if (candidate.uuid !== undefined && candidate.uuid !== null) {
        const uuid = candidate.uuid;
        if (typeof uuid === "bigint") {
            return `uuid:${uuObjIdToString(uuid)}`;
        }
        if (typeof uuid === "number" && Number.isFinite(uuid) && uuid >= 0) {
            return `uuid:${uuObjIdToString(BigInt(Math.trunc(uuid)))}`;
        }
        if (typeof uuid === "string") {
            const rawUuid = uuid.trim();
            if (!rawUuid) {
                return null;
            }
            if (/^\d+$/.test(rawUuid)) {
                try {
                    return `uuid:${uuObjIdToString(BigInt(rawUuid))}`;
                } catch {
                    return null;
                }
            }
            if (rawUuid.startsWith("uuid:")) {
                try {
                    const parsed = parseObjectCurie(rawUuid);
                    return parsed.kind === "uuid" ? `uuid:${parsed.uuid}` : null;
                } catch {
                    return null;
                }
            }
            if (UUID_RE.test(rawUuid)) {
                return `uuid:${rawUuid.toUpperCase()}`;
            }
        }
    }

    return null;
}
