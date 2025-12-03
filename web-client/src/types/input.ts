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

export type InputType =
    | "yes_no"
    | "choice"
    | "number"
    | "text"
    | "text_area"
    | "confirmation"
    | "yes_no_alternative"
    | "yes_no_alternative_all";

export interface InputMetadata {
    input_type?: InputType;
    prompt?: string;
    tts_prompt?: string;
    choices?: string[];
    min?: number;
    max?: number;
    default?: string | number | boolean;
    placeholder?: string;
    rows?: number;
    alternative_label?: string;
    alternative_placeholder?: string;
}
