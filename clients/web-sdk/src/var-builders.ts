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
import { VarList } from "@moor/schema/generated/moor-var/var-list";
import { VarObj } from "@moor/schema/generated/moor-var/var-obj";
import { VarUnion } from "@moor/schema/generated/moor-var/var-union";
import * as flatbuffers from "flatbuffers";

import { parseObjectCurie, parseUuObjIdString } from "./curie";

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
