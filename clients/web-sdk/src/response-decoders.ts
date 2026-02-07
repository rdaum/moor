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

import { ServerFeatures } from "@moor/schema/generated/moor-rpc/server-features";
import { SysPropValue } from "@moor/schema/generated/moor-rpc/sys-prop-value";

import { parseClientReplyUnion, parseHostReplyUnion } from "./reply";

function replyTypeName(value: unknown): string {
    return (value as any)?.constructor?.name ?? typeof value;
}

export interface DecodedServerFeatures {
    persistentTasks: boolean;
    richNotify: boolean;
    lexicalScopes: boolean;
    typeDispatch: boolean;
    flyweightType: boolean;
    listComprehensions: boolean;
    boolType: boolean;
    useBooleanReturns: boolean;
    symbolType: boolean;
    useSymbolsInBuiltins: boolean;
    customErrors: boolean;
    useUuobjids: boolean;
    enableEventlog: boolean;
    anonymousObjects: boolean;
}

export function decodeServerFeatures(bytes: Uint8Array): DecodedServerFeatures {
    const replyUnion = parseHostReplyUnion(bytes, "Server features request");
    if (!(replyUnion instanceof ServerFeatures)) {
        throw new Error(`Unexpected server feature reply type: ${replyTypeName(replyUnion)}`);
    }

    return {
        persistentTasks: replyUnion.persistentTasks(),
        richNotify: replyUnion.richNotify(),
        lexicalScopes: replyUnion.lexicalScopes(),
        typeDispatch: replyUnion.typeDispatch(),
        flyweightType: replyUnion.flyweightType(),
        listComprehensions: replyUnion.listComprehensions(),
        boolType: replyUnion.boolType(),
        useBooleanReturns: replyUnion.useBooleanReturns(),
        symbolType: replyUnion.symbolType(),
        useSymbolsInBuiltins: replyUnion.useSymbolsInBuiltins(),
        customErrors: replyUnion.customErrors(),
        useUuobjids: replyUnion.useUuobjids(),
        enableEventlog: replyUnion.enableEventlog(),
        anonymousObjects: replyUnion.anonymousObjects(),
    };
}

export function decodeSysPropValue<T>(
    bytes: Uint8Array,
    decodeVarToJs: (value: unknown) => T,
): T | null {
    const replyUnion = parseClientReplyUnion(bytes, "System property fetch");
    if (!(replyUnion instanceof SysPropValue)) {
        throw new Error(`Unexpected system property reply type: ${replyTypeName(replyUnion)}`);
    }

    const value = replyUnion.value();
    if (!value) {
        return null;
    }

    return decodeVarToJs(value);
}
