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

import { Obj } from "@moor/schema/generated/moor-common/obj";
import { ObjId } from "@moor/schema/generated/moor-common/obj-id";
import { ObjUnion } from "@moor/schema/generated/moor-common/obj-union";
import { UuObjId } from "@moor/schema/generated/moor-common/uu-obj-id";
import { Var as FbVar } from "@moor/schema/generated/moor-var/var";
import { VarInt } from "@moor/schema/generated/moor-var/var-int";
import { VarList } from "@moor/schema/generated/moor-var/var-list";
import { VarMap } from "@moor/schema/generated/moor-var/var-map";
import { VarMapPair } from "@moor/schema/generated/moor-var/var-map-pair";
import { VarObj } from "@moor/schema/generated/moor-var/var-obj";
import { VarStr } from "@moor/schema/generated/moor-var/var-str";
import { VarUnion } from "@moor/schema/generated/moor-var/var-union";
import * as flatbuffers from "flatbuffers";

import { parseObjectCurie, parseUuObjIdString } from "./curie.js";

export function buildObjRefOffset(builder: flatbuffers.Builder, curie: string): number {
    const parsed = parseObjectCurie(curie);
    if (parsed.kind === "oid") {
        const objIdOffset = ObjId.createObjId(builder, parsed.oid);
        const objOffset = Obj.createObj(builder, ObjUnion.ObjId, objIdOffset);
        const varObjOffset = VarObj.createVarObj(builder, objOffset);
        return FbVar.createVar(builder, VarUnion.VarObj, varObjOffset);
    }

    const packedValue = parseUuObjIdString(parsed.uuid);
    const uuObjIdOffset = UuObjId.createUuObjId(builder, packedValue);
    const objOffset = Obj.createObj(builder, ObjUnion.UuObjId, uuObjIdOffset);
    const varObjOffset = VarObj.createVarObj(builder, objOffset);
    return FbVar.createVar(builder, VarUnion.VarObj, varObjOffset);
}

export function buildObjRefList(curies: string[]): Uint8Array {
    const builder = new flatbuffers.Builder(256 + curies.length * 32);
    const varOffsets = curies.map((curie) => buildObjRefOffset(builder, curie));
    const elementsVectorOffset = VarList.createElementsVector(builder, varOffsets);
    const varListOffset = VarList.createVarList(builder, elementsVectorOffset);
    const listVarOffset = FbVar.createVar(builder, VarUnion.VarList, varListOffset);
    builder.finish(listVarOffset);
    return builder.asUint8Array();
}

/** An explicit object argument, distinct from strings and ordinary maps. */
export class ObjectArgument {
    constructor(readonly curie: string) {
        const parsed = parseObjectCurie(curie);
        if (
            parsed.kind === "oid" && (!/^oid:-?\d+$/.test(curie) || parsed.oid < -2147483648 || parsed.oid > 2147483647)
        ) {
            throw new Error("Object argument must contain a signed 32-bit object ID");
        }
    }
}

/** Structured arguments preserve literal strings, including strings that resemble object references. */
export type StructuredArgument =
    | string
    | number
    | ObjectArgument
    | StructuredArgument[]
    | { [key: string]: StructuredArgument };

export function buildStructuredArgs(values: StructuredArgument[]): Uint8Array {
    const builder = new flatbuffers.Builder(1024);
    const encode = (value: StructuredArgument, depth = 0): number => {
        if (depth > 16) throw new Error("Argument nesting is too deep");
        if (value instanceof ObjectArgument) return buildObjRefOffset(builder, value.curie);
        if (typeof value === "number") {
            if (!Number.isSafeInteger(value)) throw new Error("Integer argument is outside the safe range");
            return FbVar.createVar(builder, VarUnion.VarInt, VarInt.createVarInt(builder, BigInt(value)));
        }
        if (typeof value === "string") {
            return FbVar.createVar(builder, VarUnion.VarStr, VarStr.createVarStr(builder, builder.createString(value)));
        }
        if (Array.isArray(value)) {
            const elements = value.map(item => encode(item, depth + 1));
            return FbVar.createVar(
                builder,
                VarUnion.VarList,
                VarList.createVarList(builder, VarList.createElementsVector(builder, elements)),
            );
        }
        const pairs = Object.entries(value).map(([key, item]) => {
            const keyOffset = encode(key, depth + 1);
            const valueOffset = encode(item, depth + 1);
            VarMapPair.startVarMapPair(builder);
            VarMapPair.addKey(builder, keyOffset);
            VarMapPair.addValue(builder, valueOffset);
            return VarMapPair.endVarMapPair(builder);
        });
        return FbVar.createVar(
            builder,
            VarUnion.VarMap,
            VarMap.createVarMap(builder, VarMap.createPairsVector(builder, pairs)),
        );
    };
    builder.finish(encode(values));
    return builder.asUint8Array();
}
