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

import { parseObjectCurie } from "./curie";

export enum ORefKind {
    Oid,
    SysObj,
    Match,
}

export interface OidRef {
    kind: ORefKind.Oid;
    curie: string;
}

export interface SysObjRef {
    kind: ORefKind.SysObj;
    sysobj: string[];
}

export interface MatchRef {
    kind: ORefKind.Match;
    match: string;
}

export type ObjectRef = OidRef | SysObjRef | MatchRef;

export function oidRef(oid: number): OidRef {
    return { kind: ORefKind.Oid, curie: `oid:${oid}` };
}

export function uuidRef(uuid: string): OidRef {
    return { kind: ORefKind.Oid, curie: `uuid:${uuid}` };
}

export function curieToObjectRef(curie: string): ObjectRef {
    const parsed = parseObjectCurie(curie);
    if (parsed.kind === "oid") {
        return oidRef(parsed.oid);
    }
    return uuidRef(parsed.uuid);
}

export function sysobjRef(sysobj: string[]): SysObjRef {
    return { kind: ORefKind.SysObj, sysobj };
}

export function matchRef(match: string): MatchRef {
    return { kind: ORefKind.Match, match };
}
