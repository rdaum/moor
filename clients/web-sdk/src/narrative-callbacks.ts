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

import type { WsEventMetadata, WsLinkPreview, WsRewritable } from "./ws-narrative";

export type NarrativeMessageHandler = (
    content: string | string[],
    timestamp?: string,
    contentType?: string,
    isHistorical?: boolean,
    noNewline?: boolean,
    presentationHint?: string,
    groupId?: string,
    ttsText?: string,
    thumbnail?: { contentType: string; data: string },
    linkPreview?: WsLinkPreview,
    eventMetadata?: WsEventMetadata,
    rewritable?: WsRewritable,
    rewriteTarget?: string,
) => void;
