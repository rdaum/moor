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

// Ergonomic wrapper around FlatBuffer Var types
// Provides an API similar to the Rust moor_var::Var

import { ErrorCode } from "../generated/moor-common/error-code.js";
import { ObjId } from "../generated/moor-common/obj-id.js";
import { ObjUnion } from "../generated/moor-common/obj-union.js";
import { UuObjId } from "../generated/moor-common/uu-obj-id.js";
import { VarAnonymous } from "../generated/moor-var/var-anonymous.js";
import { VarBinary } from "../generated/moor-var/var-binary.js";
import { VarBool } from "../generated/moor-var/var-bool.js";
import { VarErr } from "../generated/moor-var/var-err.js";
import { VarFloat } from "../generated/moor-var/var-float.js";
import { VarFlyweight } from "../generated/moor-var/var-flyweight.js";
import { VarInt } from "../generated/moor-var/var-int.js";
import { VarList } from "../generated/moor-var/var-list.js";
import { VarMap } from "../generated/moor-var/var-map.js";
import { VarObj } from "../generated/moor-var/var-obj.js";
import { VarStr } from "../generated/moor-var/var-str.js";
import { VarSym } from "../generated/moor-var/var-sym.js";
import { VarUnion } from "../generated/moor-var/var-union.js";
import { Var as FbVar } from "../generated/moor-var/var.js";
import { objToString } from "./var.js";

/**
 * Ergonomic wrapper around FlatBuffer Var
 *
 * Provides a clean API for working with MOO values, abstracting away
 * the complexity of navigating FlatBuffer unions.
 */
export class MoorVar {
    private fb: FbVar;

    constructor(fb: FbVar) {
        this.fb = fb;
    }

    /**
     * Get the type of this variant
     */
    typeCode(): VarUnion {
        return this.fb.variantType();
    }

    /**
     * Check if this is a None/null value
     */
    isNone(): boolean {
        return this.fb.variantType() === VarUnion.VarNone;
    }

    /**
     * Extract integer value, or null if not an integer
     * Note: FlatBuffers returns bigint for 64-bit integers, we convert to number
     */
    asInteger(): number | null {
        if (this.fb.variantType() !== VarUnion.VarInt) return null;
        const varInt = this.fb.variant(new VarInt()) as VarInt | null;
        const val = varInt?.value();
        return val !== null && val !== undefined ? Number(val) : null;
    }

    /**
     * Extract float value, or null if not a float
     */
    asFloat(): number | null {
        if (this.fb.variantType() !== VarUnion.VarFloat) return null;
        const varFloat = this.fb.variant(new VarFloat()) as VarFloat | null;
        return varFloat?.value() ?? null;
    }

    /**
     * Extract string value, or null if not a string
     */
    asString(): string | null {
        if (this.fb.variantType() !== VarUnion.VarStr) return null;
        const varStr = this.fb.variant(new VarStr()) as VarStr | null;
        return varStr?.value() ?? null;
    }

    /**
     * Extract boolean value, or null if not a boolean
     */
    asBool(): boolean | null {
        if (this.fb.variantType() !== VarUnion.VarBool) return null;
        const varBool = this.fb.variant(new VarBool()) as VarBool | null;
        return varBool?.value() ?? null;
    }

    /**
     * Extract object reference, or null if not an object
     * Returns a simple object with either oid (number) or uuid (string)
     */
    asObject(): { oid?: number; uuid?: string } | null {
        if (this.fb.variantType() !== VarUnion.VarObj) return null;
        const varObj = this.fb.variant(new VarObj()) as VarObj | null;
        if (!varObj) return null;

        const obj = varObj.obj();
        if (!obj) return null;

        const objType = obj.objType();
        switch (objType) {
            case ObjUnion.ObjId: {
                const objId = obj.obj(new ObjId()) as ObjId | null;
                return objId ? { oid: objId.id() } : null;
            }
            case ObjUnion.UuObjId: {
                const uuObjId = obj.obj(new UuObjId()) as UuObjId | null;
                if (!uuObjId) return null;
                // Return the packed UUID value as a string
                return { uuid: uuObjId.packedValue().toString() };
            }
            default:
                return null;
        }
    }

    /**
     * Extract anonymous object reference, or null if not an anonymous object
     * Returns a simple object with either oid (number) or uuid (string), marked as anonymous
     * Anonymous objects are sigils that preserve identity but cannot be used for operations
     */
    asAnonymous(): { oid?: number; uuid?: string; anonymous?: true } | null {
        if (this.fb.variantType() !== VarUnion.VarAnonymous) return null;
        const varAnon = this.fb.variant(new VarAnonymous()) as VarAnonymous | null;
        if (!varAnon) return null;

        const obj = varAnon.obj();
        if (!obj) return null;

        const objType = obj.objType();
        switch (objType) {
            case ObjUnion.ObjId: {
                const objId = obj.obj(new ObjId()) as ObjId | null;
                return objId ? { oid: objId.id(), anonymous: true } : null;
            }
            case ObjUnion.UuObjId: {
                const uuObjId = obj.obj(new UuObjId()) as UuObjId | null;
                if (!uuObjId) return null;
                return { uuid: uuObjId.packedValue().toString(), anonymous: true };
            }
            default:
                return null;
        }
    }

    /**
     * Extract symbol value, or null if not a symbol
     */
    asSymbol(): string | null {
        if (this.fb.variantType() !== VarUnion.VarSym) return null;
        const varSym = this.fb.variant(new VarSym()) as VarSym | null;
        return varSym?.symbol()?.value() ?? null;
    }

    /**
     * Extract list value as an array of MoorVar, or null if not a list
     */
    asList(): MoorVar[] | null {
        if (this.fb.variantType() !== VarUnion.VarList) return null;
        const varList = this.fb.variant(new VarList()) as VarList | null;
        if (!varList) return null;

        const result: MoorVar[] = [];
        for (let i = 0; i < varList.elementsLength(); i++) {
            const element = varList.elements(i);
            if (element) {
                result.push(new MoorVar(element));
            }
        }
        return result;
    }

    /**
     * Extract map value as a JavaScript Map, or null if not a map
     */
    asMap(): Map<any, any> | null {
        if (this.fb.variantType() !== VarUnion.VarMap) return null;
        const varMap = this.fb.variant(new VarMap()) as VarMap | null;
        if (!varMap) return null;

        const result = new Map();
        for (let i = 0; i < varMap.pairsLength(); i++) {
            const pair = varMap.pairs(i);
            if (pair) {
                const key = pair.key();
                const value = pair.value();
                if (key && value) {
                    result.set(
                        new MoorVar(key).toJS(),
                        new MoorVar(value).toJS(),
                    );
                }
            }
        }
        return result;
    }

    /**
     * Extract error value, or null if not an error
     */
    asError(): { code: number; msg: string } | null {
        if (this.fb.variantType() !== VarUnion.VarErr) return null;
        const varErr = this.fb.variant(new VarErr()) as VarErr | null;
        if (!varErr) return null;

        const err = varErr.error();
        if (!err) return null;

        return {
            code: err.errType() ?? 0,
            msg: err.msg() ?? "",
        };
    }

    /**
     * Extract binary value as Uint8Array, or null if not binary
     */
    asBinary(): Uint8Array | null {
        if (this.fb.variantType() !== VarUnion.VarBinary) return null;
        const varBinary = this.fb.variant(new VarBinary()) as VarBinary | null;
        return varBinary?.dataArray() ?? null;
    }

    /**
     * Convert this Var to a plain JavaScript value
     *
     * This recursively converts MOO types to their JavaScript equivalents:
     * - Int/Float -> number
     * - Str -> string
     * - Bool -> boolean
     * - List -> Array
     * - Map -> Object (for now, could be Map)
     * - Obj -> { oid: number } or { uuid: string }
     * - Err -> { error: { code, msg } }
     * - None -> null
     */
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
                return list ? list.map(v => v.toJS()) : null;
            }

            case VarUnion.VarMap: {
                const map = this.asMap();
                if (!map) return null;
                // Convert Map to plain object for easier consumption
                const obj: Record<string, any> = {};
                for (const [k, v] of map.entries()) {
                    obj[String(k)] = v;
                }
                return obj;
            }

            case VarUnion.VarErr:
                return { error: this.asError() };

            case VarUnion.VarBinary:
                return this.asBinary();

            default:
                console.warn(`Unsupported Var type: ${VarUnion[varType]}`);
                return null;
        }
    }

    /**
     * Convert to MOO literal string representation
     * Matches Rust implementation in crates/compiler/src/unparse.rs
     */
    toLiteral(): string {
        const varType = this.fb.variantType();

        switch (varType) {
            case VarUnion.VarNone:
                return "None";

            case VarUnion.VarInt: {
                const val = this.asInteger();
                return val !== null ? val.toString() : "0";
            }

            case VarUnion.VarFloat: {
                const val = this.asFloat();
                return val !== null ? val.toString() : "0.0";
            }

            case VarUnion.VarStr: {
                const val = this.asString();
                if (val === null) return "\"\"";
                // Simple quote escaping - proper implementation would handle all escapes
                return `"${val.replace(/\\/g, "\\\\").replace(/"/g, "\\\"")}"`;
            }

            case VarUnion.VarBool: {
                const val = this.asBool();
                return val !== null ? val.toString() : "false";
            }

            case VarUnion.VarObj: {
                const varObj = this.fb.variant(new VarObj()) as VarObj | null;
                const obj = varObj?.obj();
                if (!obj) return "#-1";
                const objStr = objToString(obj);
                return objStr ? `#${objStr}` : "#-1";
            }

            case VarUnion.VarAnonymous: {
                // Anonymous objects display as *anonymous* sigil
                return "*anonymous*";
            }

            case VarUnion.VarList: {
                const varList = this.fb.variant(new VarList()) as VarList | null;
                if (!varList) return "{}";

                const items: string[] = [];
                const len = varList.elementsLength();
                for (let i = 0; i < len; i++) {
                    const item = varList.elements(i);
                    if (item) {
                        items.push(new MoorVar(item).toLiteral());
                    }
                }
                return `{${items.join(", ")}}`;
            }

            case VarUnion.VarMap: {
                const varMap = this.fb.variant(new VarMap()) as VarMap | null;
                if (!varMap) return "[]";

                const pairStrs: string[] = [];
                const len = varMap.pairsLength();
                for (let i = 0; i < len; i++) {
                    const pair = varMap.pairs(i);
                    if (pair) {
                        const key = pair.key();
                        const val = pair.value();
                        if (key && val) {
                            const keyStr = new MoorVar(key).toLiteral();
                            const valStr = new MoorVar(val).toLiteral();
                            pairStrs.push(`${keyStr} -> ${valStr}`);
                        }
                    }
                }
                return `[${pairStrs.join(", ")}]`;
            }

            case VarUnion.VarErr: {
                const varErr = this.fb.variant(new VarErr()) as VarErr | null;
                const err = varErr?.error();
                if (!err) return "E_NONE";
                const errType = err.errType();
                let errName: string;

                // For custom errors, get the symbol name
                if (errType === 255) { // ErrCustom
                    const customSym = err.customSymbol();
                    errName = customSym?.value() || "ErrCustom";
                } else {
                    errName = ErrorCode[errType] || "E_NONE";
                }

                const msg = err.msg();
                if (msg) {
                    // Escape quotes in the message
                    const escaped = msg.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
                    return `${errName}("${escaped}")`;
                }
                return errName;
            }

            case VarUnion.VarSym: {
                const val = this.asSymbol();
                return val ? `'${val}` : "''";
            }

            case VarUnion.VarBinary: {
                const binary = this.asBinary();
                if (!binary) return "<binary:empty>";
                return `<binary:${binary.length} bytes>`;
            }

            case VarUnion.VarFlyweight: {
                const varFlyweight = this.fb.variant(new VarFlyweight()) as VarFlyweight | null;
                if (!varFlyweight) return "<flyweight:invalid>";

                const delegate = varFlyweight.delegate();
                if (!delegate) return "<flyweight:no-delegate>";

                // Format: < delegate, .slot = value, ..., { contents } >
                const delegateStr = objToString(delegate);
                const result: string[] = [`<${delegateStr ? `#${delegateStr}` : "#-1"}`];

                // Add slots
                const slotsLen = varFlyweight.slotsLength();
                for (let i = 0; i < slotsLen; i++) {
                    const slot = varFlyweight.slots(i);
                    if (slot) {
                        const slotName = slot.name()?.value();
                        const slotValue = slot.value();
                        if (slotName && slotValue) {
                            const valueStr = new MoorVar(slotValue).toLiteral();
                            result.push(`.${slotName} = ${valueStr}`);
                        }
                    }
                }

                // Add contents
                const contents = varFlyweight.contents();
                if (contents) {
                    const contentItems: string[] = [];
                    const contentsLen = contents.elementsLength();
                    for (let i = 0; i < contentsLen; i++) {
                        const item = contents.elements(i);
                        if (item) {
                            contentItems.push(new MoorVar(item).toLiteral());
                        }
                    }
                    if (contentItems.length > 0) {
                        result.push(`{${contentItems.join(", ")}}`);
                    }
                }

                return result.join(", ") + ">";
            }

            default:
                return `<unsupported:${VarUnion[varType]}>`;
        }
    }

    /**
     * Get a debug string representation
     */
    toString(): string {
        return `MoorVar(${VarUnion[this.typeCode()]}: ${JSON.stringify(this.toJS())})`;
    }
}
