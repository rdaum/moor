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
//

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

export interface PlayerConfig {
    id: string;
    username: string;
    password: string;
    isProgrammer?: boolean;
    isWizard?: boolean;
    notes?: string;
}

export interface MooConfig {
    id: string;
    description: string;
    connectAddress: string;
    wsConnectAddress?: string;
    defaultPlayer?: string;
    players: PlayerConfig[];
}

export interface CharacterRef {
    mooId: string;
    playerId: string;
}

export interface MoorWebMcpConfig {
    defaultMoo?: string;
    defaultCharacter?: string;
    moos: MooConfig[];
}

function assertConfig(config: unknown, path: string): asserts config is MoorWebMcpConfig {
    if (!config || typeof config !== "object") {
        throw new Error(`Invalid config in ${path}: expected object`);
    }

    const candidate = config as Partial<MoorWebMcpConfig>;
    if (!Array.isArray(candidate.moos) || candidate.moos.length === 0) {
        throw new Error(`Invalid config in ${path}: moos must be a non-empty array`);
    }

    const mooIds = new Set<string>();
    for (const moo of candidate.moos) {
        if (!moo || typeof moo !== "object") {
            throw new Error(`Invalid config in ${path}: each moo must be object`);
        }
        const asMoo = moo as Partial<MooConfig>;
        if (!asMoo.id || typeof asMoo.id !== "string") {
            throw new Error(`Invalid config in ${path}: each moo needs id`);
        }
        if (mooIds.has(asMoo.id)) {
            throw new Error(`Invalid config in ${path}: duplicate moo id ${asMoo.id}`);
        }
        mooIds.add(asMoo.id);

        if (!asMoo.description || typeof asMoo.description !== "string") {
            throw new Error(`Invalid config in ${path}: moo ${asMoo.id} needs description`);
        }
        if (!asMoo.connectAddress || typeof asMoo.connectAddress !== "string") {
            throw new Error(`Invalid config in ${path}: moo ${asMoo.id} needs connectAddress`);
        }
        if (!Array.isArray(asMoo.players) || asMoo.players.length === 0) {
            throw new Error(`Invalid config in ${path}: moo ${asMoo.id} players must be a non-empty array`);
        }

        const playerIds = new Set<string>();
        for (const player of asMoo.players) {
            if (!player || typeof player !== "object") {
                throw new Error(`Invalid config in ${path}: moo ${asMoo.id} player must be object`);
            }
            const asPlayer = player as Partial<PlayerConfig>;
            if (!asPlayer.id || !asPlayer.username || !asPlayer.password) {
                throw new Error(`Invalid config in ${path}: moo ${asMoo.id} each player needs id, username, password`);
            }
            if (playerIds.has(asPlayer.id)) {
                throw new Error(`Invalid config in ${path}: moo ${asMoo.id} duplicate player id ${asPlayer.id}`);
            }
            playerIds.add(asPlayer.id);
        }
    }
}

function resolveDefaultMoo(config: MoorWebMcpConfig): MooConfig {
    if (config.defaultMoo) {
        const byId = config.moos.find((moo) => moo.id === config.defaultMoo);
        if (!byId) {
            throw new Error(`Invalid config: defaultMoo ${config.defaultMoo} not found`);
        }
        return byId;
    }
    return config.moos[0];
}

export function resolveDefaultCharacter(config: MoorWebMcpConfig): CharacterRef {
    const moo = resolveDefaultMoo(config);
    if (config.defaultCharacter) {
        const byId = moo.players.find((player) => player.id === config.defaultCharacter);
        if (!byId) {
            throw new Error(`Invalid config: defaultCharacter ${config.defaultCharacter} not found in moo ${moo.id}`);
        }
        return {
            mooId: moo.id,
            playerId: byId.id,
        };
    }
    if (moo.defaultPlayer) {
        const byId = moo.players.find((player) => player.id === moo.defaultPlayer);
        if (!byId) {
            throw new Error(`Invalid config: moo ${moo.id} defaultPlayer ${moo.defaultPlayer} not found`);
        }
        return {
            mooId: moo.id,
            playerId: byId.id,
        };
    }

    const programmer = moo.players.find((c) => c.isProgrammer);
    if (programmer) {
        return {
            mooId: moo.id,
            playerId: programmer.id,
        };
    }
    return {
        mooId: moo.id,
        playerId: moo.players[0].id,
    };
}

export function loadConfig(configPath: string): MoorWebMcpConfig {
    const absolutePath = resolve(configPath);
    const raw = readFileSync(absolutePath, "utf8");
    const parsed = JSON.parse(raw);
    assertConfig(parsed, absolutePath);
    return parsed;
}
