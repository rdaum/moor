// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

import type { Resource } from "@modelcontextprotocol/server";
import type { CharacterRef } from "./config.js";
import type { DynamicResource, DynamicTool, MoorWebClient } from "./moor-service.js";

function key(character: CharacterRef): string {
    return JSON.stringify([character.mooId, character.playerId]);
}

export function resourceUri(character: CharacterRef, uri: string): string {
    return `moor://world/${encodeURIComponent(character.mooId)}/${encodeURIComponent(character.playerId)}/${
        encodeURIComponent(uri)
    }`;
}

/** Registry values belong to the world and account that supplied them. */
export class ToolCatalog {
    private readonly toolCache = new Map<string, Promise<DynamicTool[]>>();
    private readonly resourceCache = new Map<string, Promise<DynamicResource[]>>();

    constructor(private readonly moor: MoorWebClient) {}

    private load<T>(cache: Map<string, Promise<T>>, character: CharacterRef, fetch: () => Promise<T>): Promise<T> {
        const id = key(character);
        let pending = cache.get(id);
        if (!pending) {
            pending = fetch();
            cache.set(id, pending);
            pending.catch(() => {
                if (cache.get(id) === pending) cache.delete(id);
            });
        }
        return pending;
    }

    tools(character: CharacterRef): Promise<DynamicTool[]> {
        return this.load(this.toolCache, character, () => this.moor.refreshDynamicTools(character));
    }

    resources(character: CharacterRef): Promise<DynamicResource[]> {
        return this.load(this.resourceCache, character, () => this.moor.refreshDynamicResources(character));
    }

    async refresh(character: CharacterRef): Promise<number> {
        this.toolCache.delete(key(character));
        this.resourceCache.delete(key(character));
        return (await this.tools(character)).length;
    }

    async listResources(characters: CharacterRef[], defaultCharacter: CharacterRef): Promise<Resource[]> {
        const result: Resource[] = [];
        for (const character of characters) {
            for (const { content: _content, ...resource } of await this.resources(character)) {
                result.push({
                    ...resource,
                    uri: resourceUri(character, resource.uri),
                    name: `${character.mooId}: ${resource.name}`,
                });
                if (key(character) === key(defaultCharacter)) result.push(resource);
            }
        }
        return result;
    }

    async readResource(uri: string, defaultCharacter: CharacterRef): Promise<DynamicResource | undefined> {
        const scoped = /^moor:\/\/world\/([^/]+)\/([^/]+)\/([^/]+)$/.exec(uri);
        const character = scoped
            ? { mooId: decodeURIComponent(scoped[1]), playerId: decodeURIComponent(scoped[2]) }
            : defaultCharacter;
        this.moor.findCharacter(character);
        const original = scoped ? decodeURIComponent(scoped[3]) : uri;
        return (await this.resources(character)).find(resource => resource.uri === original);
    }
}
