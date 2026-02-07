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

import { readFileSync } from "node:fs";
import { resolve } from "node:path";

export interface CharacterConfig {
    id: string;
    username: string;
    password: string;
    isProgrammer?: boolean;
    isWizard?: boolean;
    notes?: string;
}

export interface MoorWebMcpConfig {
    baseUrl: string;
    wsBaseUrl?: string;
    defaultCharacter?: string;
    characters: CharacterConfig[];
}

function assertConfig(config: unknown, path: string): asserts config is MoorWebMcpConfig {
    if (!config || typeof config !== "object") {
        throw new Error(`Invalid config in ${path}: expected object`);
    }

    const candidate = config as Partial<MoorWebMcpConfig>;
    if (!candidate.baseUrl || typeof candidate.baseUrl !== "string") {
        throw new Error(`Invalid config in ${path}: missing baseUrl`);
    }
    if (!Array.isArray(candidate.characters) || candidate.characters.length === 0) {
        throw new Error(`Invalid config in ${path}: characters must be a non-empty array`);
    }

    const ids = new Set<string>();
    for (const character of candidate.characters) {
        if (!character || typeof character !== "object") {
            throw new Error(`Invalid config in ${path}: character must be object`);
        }
        const ch = character as Partial<CharacterConfig>;
        if (!ch.id || !ch.username || !ch.password) {
            throw new Error(`Invalid config in ${path}: each character needs id, username, password`);
        }
        if (ids.has(ch.id)) {
            throw new Error(`Invalid config in ${path}: duplicate character id ${ch.id}`);
        }
        ids.add(ch.id);
    }
}

export function resolveDefaultCharacter(config: MoorWebMcpConfig): string {
    if (config.defaultCharacter) {
        return config.defaultCharacter;
    }
    const programmer = config.characters.find((c) => c.isProgrammer);
    if (programmer) {
        return programmer.id;
    }
    return config.characters[0].id;
}

export function loadConfig(configPath: string): MoorWebMcpConfig {
    const absolutePath = resolve(configPath);
    const raw = readFileSync(absolutePath, "utf8");
    const parsed = JSON.parse(raw);
    assertConfig(parsed, absolutePath);
    return parsed;
}
