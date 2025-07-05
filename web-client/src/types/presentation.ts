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

export interface PresentationData {
    readonly id: string;
    readonly content_type: string;
    readonly content: string;
    readonly target: string;
    readonly attributes: ReadonlyArray<readonly [string, string]>;
}

export interface Presentation {
    readonly id: string;
    readonly target: string;
    readonly title: string;
    readonly content: string | string[];
    readonly contentType: "text/plain" | "text/djot" | "text/html";
    readonly attrs: Readonly<{ [key: string]: string }>;
}

export type DockTarget = "left-dock" | "right-dock" | "top-dock" | "bottom-dock";
export type PresentationTarget = DockTarget | "window" | "verb-editor";

export const TARGET_TYPES = {
    WINDOW: "window" as const,
    RIGHT_DOCK: "right-dock" as const,
    LEFT_DOCK: "left-dock" as const,
    TOP_DOCK: "top-dock" as const,
    BOTTOM_DOCK: "bottom-dock" as const,
    VERB_EDITOR: "verb-editor" as const,
};
