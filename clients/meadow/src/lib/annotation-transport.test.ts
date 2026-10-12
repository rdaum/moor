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

// Copyright (C) 2026 The mooR Authors
// SPDX-License-Identifier: GPL-3.0-or-later
import { Event } from "@moor/schema/generated/moor-common/event";
import { EventMetadata } from "@moor/schema/generated/moor-common/event-metadata";
import { EventUnion } from "@moor/schema/generated/moor-common/event-union";
import { NarrativeEvent } from "@moor/schema/generated/moor-common/narrative-event";
import { NotifyEvent } from "@moor/schema/generated/moor-common/notify-event";
import { Obj } from "@moor/schema/generated/moor-common/obj";
import { ObjId } from "@moor/schema/generated/moor-common/obj-id";
import { ObjUnion } from "@moor/schema/generated/moor-common/obj-union";
import { Symbol as FbSymbol } from "@moor/schema/generated/moor-common/symbol";
import { Uuid } from "@moor/schema/generated/moor-common/uuid";
import { NarrativeEventMessage } from "@moor/schema/generated/moor-rpc/narrative-event-message";
import { Var } from "@moor/schema/generated/moor-var/var";
import { VarInt } from "@moor/schema/generated/moor-var/var-int";
import { VarList } from "@moor/schema/generated/moor-var/var-list";
import { VarMap } from "@moor/schema/generated/moor-var/var-map";
import { VarMapPair } from "@moor/schema/generated/moor-var/var-map-pair";
import { VarStr } from "@moor/schema/generated/moor-var/var-str";
import { VarUnion } from "@moor/schema/generated/moor-var/var-union";
import {
    MoorVar,
    parseHistoricalNarrativeEvent,
    parseNarrativeEventEnvelope,
    parseWsNarrativeEventMessage,
} from "@moor/web-sdk";
import { Encrypter, generateIdentity, identityToRecipient } from "age-encryption";
import { Builder, ByteBuffer } from "flatbuffers";
import { expect, it } from "vitest";
import { decryptEventBlob } from "./age-decrypt";

function notification() {
    const b = new Builder(1024);
    const string = (value: string) => Var.createVar(b, VarUnion.VarStr, VarStr.createVarStr(b, b.createString(value)));
    const map = (entries: [string, number][]) => {
        const pairs = entries.map(([key, value]) => {
            const k = string(key);
            VarMapPair.startVarMapPair(b);
            VarMapPair.addKey(b, k);
            VarMapPair.addValue(b, value);
            return VarMapPair.endVarMapPair(b);
        });
        return Var.createVar(b, VarUnion.VarMap, VarMap.createVarMap(b, VarMap.createPairsVector(b, pairs)));
    };
    const list = (values: number[]) =>
        Var.createVar(b, VarUnion.VarList, VarList.createVarList(b, VarList.createElementsVector(b, values)));
    const document = map([
        ["version", Var.createVar(b, VarUnion.VarInt, VarInt.createVarInt(b, 1n))],
        [
            "blocks",
            list([map([
                ["kind", string("paragraph")],
                [
                    "children",
                    list([
                        map([["kind", string("text")], ["text", string("You picked up ")]]),
                        map([["kind", string("reference")], [
                            "descriptor",
                            map([["kind", string("object")], ["ref", string("oid:47")]]),
                        ], ["children", list([map([["kind", string("text")], ["text", string("Compass")]])])]]),
                        map([["kind", string("text")], ["text", string(".")]]),
                    ]),
                ],
            ])]),
        ],
    ]);
    const documentKey = FbSymbol.createSymbol(b, b.createString("cowbell_document"));
    EventMetadata.startEventMetadata(b);
    EventMetadata.addKey(b, documentKey);
    EventMetadata.addValue(b, document);
    const documentMetadata = EventMetadata.endEventMetadata(b);
    const table = map([["a1", map([["kind", string("object")], ["ref", string("oid:47")]])]]);
    const key = FbSymbol.createSymbol(b, b.createString("annotations"));
    EventMetadata.startEventMetadata(b);
    EventMetadata.addKey(b, key);
    EventMetadata.addValue(b, table);
    const annotations = EventMetadata.endEventMetadata(b);
    const titleKey = FbSymbol.createSymbol(b, b.createString("collapse_title"));
    const title = string("Help");
    EventMetadata.startEventMetadata(b);
    EventMetadata.addKey(b, titleKey);
    EventMetadata.addValue(b, title);
    const metadata = NotifyEvent.createMetadataVector(b, [
        annotations,
        EventMetadata.endEventMetadata(b),
        documentMetadata,
    ]);
    const value = string("You picked up [Compass]{annotation=a1}.");
    const contentType = FbSymbol.createSymbol(b, b.createString("text_djot"));
    NotifyEvent.startNotifyEvent(b);
    NotifyEvent.addValue(b, value);
    NotifyEvent.addContentType(b, contentType);
    NotifyEvent.addMetadata(b, metadata);
    const event = Event.createEvent(b, EventUnion.NotifyEvent, NotifyEvent.endNotifyEvent(b));
    const id = Uuid.createUuid(b, Uuid.createDataVector(b, new Uint8Array(16).fill(1)));
    NarrativeEvent.startNarrativeEvent(b);
    NarrativeEvent.addEventId(b, id);
    NarrativeEvent.addAuthor(b, value);
    NarrativeEvent.addEvent(b, event);
    const narrative = NarrativeEvent.endNarrativeEvent(b);
    b.finish(narrative);
    const historyBytes = b.asUint8Array().slice();
    const player = Obj.createObj(b, ObjUnion.ObjId, ObjId.createObjId(b, 2));
    NarrativeEventMessage.startNarrativeEventMessage(b);
    NarrativeEventMessage.addPlayer(b, player);
    NarrativeEventMessage.addEvent(b, narrative);
    b.finish(NarrativeEventMessage.endNarrativeEventMessage(b));
    return { liveBytes: b.asUint8Array(), historyBytes };
}
const decode = (value: Var) => new MoorVar(value).toJS();
const string = (value: Var) => new MoorVar(value).asString();

it("retains nested annotation metadata through live FlatBuffers and encrypted historical replay", async () => {
    const { liveBytes, historyBytes } = notification();
    const live = parseWsNarrativeEventMessage(
        NarrativeEventMessage.getRootAsNarrativeEventMessage(new ByteBuffer(liveBytes)),
        decode,
        string,
    );
    expect(live?.kind).toBe("notify");
    if (live?.kind !== "notify") throw new Error("notify missing");
    expect(live.eventMeta?.annotations).toEqual({ a1: { kind: "object", ref: "oid:47" } });
    expect(live.eventMeta?.collapseTitle).toBe("Help");
    const identity = await generateIdentity();
    const encrypter = new Encrypter();
    encrypter.addRecipient(await identityToRecipient(identity));
    const recovered = await decryptEventBlob(await encrypter.encrypt(historyBytes), identity);
    const envelope = parseNarrativeEventEnvelope(recovered);
    const history = parseHistoricalNarrativeEvent(envelope!.narrativeEvent, decode, string);
    expect(history?.kind).toBe("notify");
    if (history?.kind !== "notify") throw new Error("history missing");
    expect(history.eventMeta?.annotations).toEqual(live.eventMeta?.annotations);
    expect(history.content).toEqual(live.content);
    expect(live.eventMeta?.cowbellDocument).toMatchObject({ version: 1, blocks: [{ kind: "paragraph" }] });
    expect(history.eventMeta?.cowbellDocument).toEqual(live.eventMeta?.cowbellDocument);
    expect(history.eventMeta?.collapseTitle).toBe("Help");
});
