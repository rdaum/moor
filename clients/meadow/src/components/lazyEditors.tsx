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

import { lazySurface } from "./lazySurface";

export const ChangeWorkspace = lazySurface(
    "change workspace",
    async () => ({ default: (await import("./ChangeWorkspace")).ChangeWorkspace }),
);

export const ChangeReview = lazySurface(
    "change review",
    async () => ({ default: (await import("./ChangeReview")).ChangeReview }),
);

export const VerbEditor = lazySurface(
    "verb editor",
    async () => ({ default: (await import("./VerbEditor")).VerbEditor }),
);
export const TextEditor = lazySurface(
    "text editor",
    async () => ({ default: (await import("./TextEditor")).TextEditor }),
);
export const PropertyEditor = lazySurface(
    "property editor",
    async () => ({ default: (await import("./PropertyEditor")).PropertyEditor }),
);
export const PropertyValueEditorWindow = lazySurface(
    "property value editor",
    async () => ({ default: (await import("./PropertyValueEditorWindow")).PropertyValueEditorWindow }),
);
export const ObjectBrowser = lazySurface(
    "object browser",
    async () => ({ default: (await import("./ObjectBrowser")).ObjectBrowser }),
);
export const EvalPanel = lazySurface(
    "evaluation panel",
    async () => ({ default: (await import("./EvalPanel")).EvalPanel }),
);
