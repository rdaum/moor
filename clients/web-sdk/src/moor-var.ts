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

import { ErrorCode } from "@moor/schema/generated/moor-common/error-code";
import { ObjId } from "@moor/schema/generated/moor-common/obj-id";
import { ObjUnion } from "@moor/schema/generated/moor-common/obj-union";
import { UuObjId } from "@moor/schema/generated/moor-common/uu-obj-id";
import { Var as FbVar } from "@moor/schema/generated/moor-var/var";
import { VarAnonymous } from "@moor/schema/generated/moor-var/var-anonymous";
import { VarBinary } from "@moor/schema/generated/moor-var/var-binary";
import { VarBool } from "@moor/schema/generated/moor-var/var-bool";
import { VarErr } from "@moor/schema/generated/moor-var/var-err";
import { VarFloat } from "@moor/schema/generated/moor-var/var-float";
import { VarFlyweight } from "@moor/schema/generated/moor-var/var-flyweight";
import { VarInt } from "@moor/schema/generated/moor-var/var-int";
import { VarList } from "@moor/schema/generated/moor-var/var-list";
import { VarMap } from "@moor/schema/generated/moor-var/var-map";
import { VarObj } from "@moor/schema/generated/moor-var/var-obj";
import { VarStr } from "@moor/schema/generated/moor-var/var-str";
import { VarSym } from "@moor/schema/generated/moor-var/var-sym";
import { VarUnion } from "@moor/schema/generated/moor-var/var-union";
import * as flatbuffers from "flatbuffers";

import { objToString } from "./curie";
import { buildObjRefList as buildObjRefListShared } from "./var-builders";

export class MoorVar {
    private fb: FbVar;

    constructor(fb: FbVar) {
        this.fb = fb;
    }

    typeCode(): VarUnion {
        return this.fb.variantType();
    }

    isNone(): boolean {
        return this.fb.variantType() === VarUnion.VarNone;
    }

    asInteger(): number | null {
        if (this.fb.variantType() !== VarUnion.VarInt) {
            return null;
        }
        const varInt = this.fb.variant(new VarInt()) as VarInt | null;
        const value = varInt?.value();
        return value !== null && value !== undefined ? Number(value) : null;
    }

    asFloat(): number | null {
        if (this.fb.variantType() !== VarUnion.VarFloat) {
            return null;
        }
        const varFloat = this.fb.variant(new VarFloat()) as VarFloat | null;
        return varFloat?.value() ?? null;
    }

    asString(): string | null {
        if (this.fb.variantType() !== VarUnion.VarStr) {
            return null;
        }
        const varStr = this.fb.variant(new VarStr()) as VarStr | null;
        return varStr?.value() ?? null;
    }

    asBool(): boolean | null {
        if (this.fb.variantType() !== VarUnion.VarBool) {
            return null;
        }
        const varBool = this.fb.variant(new VarBool()) as VarBool | null;
        return varBool?.value() ?? null;
    }

    asObject(): { oid?: number; uuid?: string } | null {
        if (this.fb.variantType() !== VarUnion.VarObj) {
            return null;
        }
        const varObj = this.fb.variant(new VarObj()) as VarObj | null;
        if (!varObj) {
            return null;
        }

        const obj = varObj.obj();
        if (!obj) {
            return null;
        }

        const objType = obj.objType();
        switch (objType) {
            case ObjUnion.ObjId: {
                const objId = obj.obj(new ObjId()) as ObjId | null;
                return objId ? { oid: objId.id() } : null;
            }
            case ObjUnion.UuObjId: {
                const uuObjId = obj.obj(new UuObjId()) as UuObjId | null;
                if (!uuObjId) {
                    return null;
                }
                return { uuid: uuObjId.packedValue().toString() };
            }
            default:
                return null;
        }
    }

    asAnonymous(): { oid?: number; uuid?: string; anonymous?: true } | null {
        if (this.fb.variantType() !== VarUnion.VarAnonymous) {
            return null;
        }
        const varAnon = this.fb.variant(new VarAnonymous()) as VarAnonymous | null;
        if (!varAnon) {
            return null;
        }

        const obj = varAnon.obj();
        if (!obj) {
            return null;
        }

        const objType = obj.objType();
        switch (objType) {
            case ObjUnion.ObjId: {
                const objId = obj.obj(new ObjId()) as ObjId | null;
                return objId ? { oid: objId.id(), anonymous: true } : null;
            }
            case ObjUnion.UuObjId: {
                const uuObjId = obj.obj(new UuObjId()) as UuObjId | null;
                if (!uuObjId) {
                    return null;
                }
                return { uuid: uuObjId.packedValue().toString(), anonymous: true };
            }
            default:
                return null;
        }
    }

    asSymbol(): string | null {
        if (this.fb.variantType() !== VarUnion.VarSym) {
            return null;
        }
        const varSym = this.fb.variant(new VarSym()) as VarSym | null;
        return varSym?.symbol()?.value() ?? null;
    }

    asList(): MoorVar[] | null {
        if (this.fb.variantType() !== VarUnion.VarList) {
            return null;
        }
        const varList = this.fb.variant(new VarList()) as VarList | null;
        if (!varList) {
            return null;
        }

        const result: MoorVar[] = [];
        for (let i = 0; i < varList.elementsLength(); i++) {
            const element = varList.elements(i);
            if (element) {
                result.push(new MoorVar(element));
            }
        }
        return result;
    }

    asMap(): Map<any, any> | null {
        if (this.fb.variantType() !== VarUnion.VarMap) {
            return null;
        }
        const varMap = this.fb.variant(new VarMap()) as VarMap | null;
        if (!varMap) {
            return null;
        }

        const result = new Map();
        for (let i = 0; i < varMap.pairsLength(); i++) {
            const pair = varMap.pairs(i);
            if (!pair) {
                continue;
            }
            const key = pair.key();
            const value = pair.value();
            if (!key || !value) {
                continue;
            }
            result.set(new MoorVar(key).toJS(), new MoorVar(value).toJS());
        }
        return result;
    }

    asError(): { code: number; msg: string } | null {
        if (this.fb.variantType() !== VarUnion.VarErr) {
            return null;
        }
        const varErr = this.fb.variant(new VarErr()) as VarErr | null;
        if (!varErr) {
            return null;
        }

        const err = varErr.error();
        if (!err) {
            return null;
        }

        return {
            code: err.errType() ?? 0,
            msg: err.msg() ?? "",
        };
    }

    asBinary(): Uint8Array | null {
        if (this.fb.variantType() !== VarUnion.VarBinary) {
            return null;
        }
        const varBinary = this.fb.variant(new VarBinary()) as VarBinary | null;
        return varBinary?.dataArray() ?? null;
    }

    toJS(): any {
        const varType = this.fb.variantType();
        switch (varType) {
            case VarUnion.VarNone:
                return null;
            case VarUnion.VarInt:
                return this.asInteger();
            case VarUnion.VarFloat:
                return this.asFloat();
            case VarUnion.VarStr:
                return this.asString();
            case VarUnion.VarBool:
                return this.asBool();
            case VarUnion.VarObj:
                return this.asObject();
            case VarUnion.VarAnonymous:
                return this.asAnonymous();
            case VarUnion.VarSym:
                return this.asSymbol();
            case VarUnion.VarList: {
                const list = this.asList();
                return list ? list.map((value) => value.toJS()) : null;
            }
            case VarUnion.VarMap: {
                const map = this.asMap();
                if (!map) {
                    return null;
                }
                const obj: Record<string, any> = {};
                for (const [key, value] of map.entries()) {
                    obj[String(key)] = value;
                }
                return obj;
            }
            case VarUnion.VarErr:
                return { error: this.asError() };
            case VarUnion.VarBinary:
                return this.asBinary();
            case VarUnion.VarFlyweight: {
                const varFlyweight = this.fb.variant(new VarFlyweight()) as VarFlyweight | null;
                if (!varFlyweight) {
                    return null;
                }

                const result: Record<string, unknown> = {};
                const delegate = varFlyweight.delegate();
                if (delegate) {
                    const delegateStr = objToString(delegate);
                    result._delegate = delegateStr ? `#${delegateStr}` : "#-1";
                }

                for (let i = 0; i < varFlyweight.slotsLength(); i++) {
                    const slot = varFlyweight.slots(i);
                    if (!slot) {
                        continue;
                    }
                    const slotName = slot.name()?.value();
                    const slotValue = slot.value();
                    if (slotName && slotValue) {
                        result[slotName] = new MoorVar(slotValue).toJS();
                    }
                }

                const contents = varFlyweight.contents();
                if (contents && contents.elementsLength() > 0) {
                    const contentsArray: unknown[] = [];
                    for (let i = 0; i < contents.elementsLength(); i++) {
                        const item = contents.elements(i);
                        if (item) {
                            contentsArray.push(new MoorVar(item).toJS());
                        }
                    }
                    result._contents = contentsArray;
                }

                return result;
            }
            default:
                console.warn(`Unsupported Var type: ${VarUnion[varType]}`);
                return null;
        }
    }

    toLiteral(): string {
        const varType = this.fb.variantType();
        switch (varType) {
            case VarUnion.VarNone:
                return "None";
            case VarUnion.VarInt: {
                const value = this.asInteger();
                return value !== null ? value.toString() : "0";
            }
            case VarUnion.VarFloat: {
                const value = this.asFloat();
                return value !== null ? value.toString() : "0.0";
            }
            case VarUnion.VarStr: {
                const value = this.asString();
                if (value === null) {
                    return "\"\"";
                }
                return `"${value.replace(/\\/g, "\\\\").replace(/"/g, "\\\"")}"`;
            }
            case VarUnion.VarBool: {
                const value = this.asBool();
                return value !== null ? value.toString() : "false";
            }
            case VarUnion.VarObj: {
                const varObj = this.fb.variant(new VarObj()) as VarObj | null;
                const obj = varObj?.obj();
                if (!obj) {
                    return "#-1";
                }
                const objStr = objToString(obj);
                return objStr ? `#${objStr}` : "#-1";
            }
            case VarUnion.VarAnonymous:
                return "*anonymous*";
            case VarUnion.VarList: {
                const varList = this.fb.variant(new VarList()) as VarList | null;
                if (!varList) {
                    return "{}";
                }
                const items: string[] = [];
                for (let i = 0; i < varList.elementsLength(); i++) {
                    const item = varList.elements(i);
                    if (item) {
                        items.push(new MoorVar(item).toLiteral());
                    }
                }
                return `{${items.join(", ")}}`;
            }
            case VarUnion.VarMap: {
                const varMap = this.fb.variant(new VarMap()) as VarMap | null;
                if (!varMap) {
                    return "[]";
                }
                const pairStrs: string[] = [];
                for (let i = 0; i < varMap.pairsLength(); i++) {
                    const pair = varMap.pairs(i);
                    if (!pair) {
                        continue;
                    }
                    const key = pair.key();
                    const value = pair.value();
                    if (key && value) {
                        pairStrs.push(`${new MoorVar(key).toLiteral()} -> ${new MoorVar(value).toLiteral()}`);
                    }
                }
                return `[${pairStrs.join(", ")}]`;
            }
            case VarUnion.VarErr: {
                const varErr = this.fb.variant(new VarErr()) as VarErr | null;
                const err = varErr?.error();
                if (!err) {
                    return "E_NONE";
                }
                const errType = err.errType();
                let errName: string;
                if (errType === 255) {
                    errName = err.customSymbol()?.value() || "ErrCustom";
                } else {
                    errName = ErrorCode[errType] || "E_NONE";
                }
                const msg = err.msg();
                if (msg) {
                    const escaped = msg.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
                    return `${errName}("${escaped}")`;
                }
                return errName;
            }
            case VarUnion.VarSym: {
                const value = this.asSymbol();
                return value ? `'${value}` : "''";
            }
            case VarUnion.VarBinary: {
                const binary = this.asBinary();
                if (!binary) {
                    return "<binary:empty>";
                }
                return `<binary:${binary.length} bytes>`;
            }
            case VarUnion.VarFlyweight: {
                const varFlyweight = this.fb.variant(new VarFlyweight()) as VarFlyweight | null;
                if (!varFlyweight) {
                    return "<flyweight:invalid>";
                }
                const delegate = varFlyweight.delegate();
                if (!delegate) {
                    return "<flyweight:no-delegate>";
                }
                const delegateStr = objToString(delegate);
                const result: string[] = [`<${delegateStr ? `#${delegateStr}` : "#-1"}`];
                for (let i = 0; i < varFlyweight.slotsLength(); i++) {
                    const slot = varFlyweight.slots(i);
                    if (!slot) {
                        continue;
                    }
                    const slotName = slot.name()?.value();
                    const slotValue = slot.value();
                    if (slotName && slotValue) {
                        result.push(`.${slotName} = ${new MoorVar(slotValue).toLiteral()}`);
                    }
                }
                const contents = varFlyweight.contents();
                if (contents) {
                    const contentItems: string[] = [];
                    for (let i = 0; i < contents.elementsLength(); i++) {
                        const item = contents.elements(i);
                        if (item) {
                            contentItems.push(new MoorVar(item).toLiteral());
                        }
                    }
                    if (contentItems.length > 0) {
                        result.push(`{${contentItems.join(", ")}}`);
                    }
                }
                return `${result.join(", ")}>`;
            }
            default:
                return `<unsupported:${VarUnion[varType]}>`;
        }
    }

    toString(): string {
        return `MoorVar(${VarUnion[this.typeCode()]}: ${JSON.stringify(this.toJS())})`;
    }

    static buildEmptyList(): Uint8Array {
        const builder = new flatbuffers.Builder(256);
        const emptyListOffset = VarList.createVarList(builder, VarList.createElementsVector(builder, []));
        const listVarOffset = FbVar.createVar(builder, VarUnion.VarList, emptyListOffset);
        builder.finish(listVarOffset);
        return builder.asUint8Array();
    }

    static buildStringList(strings: string[]): Uint8Array {
        const estimatedSize = 256 + strings.reduce((sum, value) => sum + value.length * 2, 0);
        const builder = new flatbuffers.Builder(estimatedSize);

        const varOffsets: number[] = [];
        for (const str of strings) {
            const strOffset = builder.createString(str);
            const varStrOffset = VarStr.createVarStr(builder, strOffset);
            const varOffset = FbVar.createVar(builder, VarUnion.VarStr, varStrOffset);
            varOffsets.push(varOffset);
        }

        const elementsVectorOffset = VarList.createElementsVector(builder, varOffsets);
        const varListOffset = VarList.createVarList(builder, elementsVectorOffset);
        const listVarOffset = FbVar.createVar(builder, VarUnion.VarList, varListOffset);
        builder.finish(listVarOffset);
        return builder.asUint8Array();
    }

    static buildObjRefList(curies: string[]): Uint8Array {
        return buildObjRefListShared(curies);
    }
}
