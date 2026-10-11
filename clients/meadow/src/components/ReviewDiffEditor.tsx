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

import { DiffEditor, type DiffEditorProps } from "@monaco-editor/react";
import type { editor } from "monaco-editor";
import { useLayoutEffect, useRef } from "react";

/** Detach the comparison before releasing its models when a review row changes. */
export function ReviewDiffEditor(props: DiffEditorProps) {
    const instance = useRef<editor.IStandaloneDiffEditor | null>(null);
    useLayoutEffect(() => () => {
        const view = instance.current;
        instance.current = null;
        const model = view?.getModel();
        view?.setModel(null);
        model?.original.dispose();
        model?.modified.dispose();
    }, []);
    return (
        <DiffEditor
            {...props}
            keepCurrentOriginalModel
            keepCurrentModifiedModel
            onMount={(view, monaco) => {
                instance.current = view;
                props.onMount?.(view, monaco);
            }}
        />
    );
}
