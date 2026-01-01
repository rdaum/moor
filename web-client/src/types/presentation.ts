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

export type SemanticTarget = "navigation" | "inventory" | "status" | "tools" | "communication" | "help";
export type PresentationTarget =
    | SemanticTarget
    | "window"
    | "verb-editor"
    | "property-editor"
    | "property-value-editor"
    | "object-browser"
    | "text-editor"
    | "profile-setup";

export const TARGET_TYPES = {
    WINDOW: "window" as const,
    NAVIGATION: "navigation" as const,
    INVENTORY: "inventory" as const,
    STATUS: "status" as const,
    TOOLS: "tools" as const,
    COMMUNICATION: "communication" as const,
    HELP: "help" as const,
    VERB_EDITOR: "verb-editor" as const,
    PROPERTY_EDITOR: "property-editor" as const,
    PROPERTY_VALUE_EDITOR: "property-value-editor" as const,
    OBJECT_BROWSER: "object-browser" as const,
    TEXT_EDITOR: "text-editor" as const,
    PROFILE_SETUP: "profile-setup" as const,
};
