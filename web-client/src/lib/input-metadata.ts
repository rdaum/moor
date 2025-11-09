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

import { MetadataPair } from "../generated/moor-rpc/metadata-pair";
import type { InputMetadata } from "../types/input";
import { MoorVar } from "./MoorVar";

/**
 * Parse FlatBuffer metadata pairs into InputMetadata structure
 */
export function parseInputMetadata(metadataPairs: MetadataPair[] | null): InputMetadata | null {
    if (!metadataPairs || metadataPairs.length === 0) {
        return null;
    }

    const metadata: InputMetadata = {};

    for (const pair of metadataPairs) {
        const key = pair.key()?.value();
        const valueVar = pair.value();

        if (!key || !valueVar) {
            continue;
        }

        const moorVar = new MoorVar(valueVar);

        switch (key) {
            case "input_type": {
                const strValue = moorVar.asString();
                if (strValue) {
                    metadata.input_type = strValue as InputMetadata["input_type"];
                }
                break;
            }

            case "prompt": {
                const strValue = moorVar.asString();
                if (strValue) {
                    metadata.prompt = strValue;
                }
                break;
            }

            case "choices": {
                const listValue = moorVar.asList();
                if (listValue) {
                    metadata.choices = listValue
                        .map(v => v.asString())
                        .filter((v): v is string => v !== null);
                }
                break;
            }

            case "min": {
                const intValue = moorVar.asInteger();
                const floatValue = moorVar.asFloat();
                const numValue = intValue !== null ? intValue : floatValue;
                if (numValue !== null) {
                    metadata.min = numValue;
                }
                break;
            }

            case "max": {
                const intValue = moorVar.asInteger();
                const floatValue = moorVar.asFloat();
                const numValue = intValue !== null ? intValue : floatValue;
                if (numValue !== null) {
                    metadata.max = numValue;
                }
                break;
            }

            case "default": {
                const strValue = moorVar.asString();
                const intValue = moorVar.asInteger();
                const floatValue = moorVar.asFloat();
                const boolValue = moorVar.asBool();
                if (strValue !== null) {
                    metadata.default = strValue;
                } else if (intValue !== null) {
                    metadata.default = intValue;
                } else if (floatValue !== null) {
                    metadata.default = floatValue;
                } else if (boolValue !== null) {
                    metadata.default = boolValue;
                }
                break;
            }

            case "placeholder": {
                const strValue = moorVar.asString();
                if (strValue) {
                    metadata.placeholder = strValue;
                }
                break;
            }

            case "alternative_label": {
                const strValue = moorVar.asString();
                if (strValue) {
                    metadata.alternative_label = strValue;
                }
                break;
            }

            case "alternative_placeholder": {
                const strValue = moorVar.asString();
                if (strValue) {
                    metadata.alternative_placeholder = strValue;
                }
                break;
            }
        }
    }

    return Object.keys(metadata).length > 0 ? metadata : null;
}
