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

import Editor, { type EditorProps, type OnMount } from "@monaco-editor/react";
import { useCallback, useEffect, useRef } from "react";
import type { CompilerDiagnostic } from "../lib/change-review";

/** Compiler positions refer to the exact saved result; callers clear them when its text changes. */
export function ReviewProgramEditor(
    { diagnostics, initialValue, ...props }: Omit<EditorProps, "value" | "defaultValue"> & {
        initialValue: string;
        diagnostics: CompilerDiagnostic[];
    },
) {
    const mounted = useRef<Parameters<OnMount> | null>(null);
    const current = useRef(diagnostics);
    current.current = diagnostics;
    const update = useCallback(() => {
        if (!mounted.current) return;
        const [view, monaco] = mounted.current;
        const model = view.getModel();
        if (!model || model.isDisposed()) return;
        monaco.editor.setModelMarkers(
            model,
            "change-review",
            current.current.map(item => ({ ...item, severity: monaco.MarkerSeverity.Error })),
        );
    }, []);
    useEffect(update, [diagnostics, update]);
    return (
        <Editor
            {...props}
            // The model owns edits until this editing session ends. Replaying React's
            // controlled value can replace newer keystrokes before its effects catch up.
            defaultValue={initialValue}
            onMount={(view, monaco) => {
                mounted.current = [view, monaco];
                update();
                props.onMount?.(view, monaco);
            }}
        />
    );
}
