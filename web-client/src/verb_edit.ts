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

import van from "vanjs-core";
import { FloatingWindow } from "vanjs-ui";

import { createEditor, updateEditor } from "./editor";
import { Context } from "./model";
import { MoorRemoteObject } from "./rpc";
import { ObjectRef } from "./var";

const { button, div, pre } = van.tags;

enum CompileErrorKind {
    ParseError,
    Other,
}

interface ParseError {
    kind: CompileErrorKind.ParseError;
    line: number;
    column: number;
    context: string;
    end_line_col: [number, number] | null;
    message: string;
}

interface OtherError {
    kind: CompileErrorKind.Other;
    message: string;
}
type CompileError = ParseError | OtherError;

function transformErrors(error: CompileError | null) {
    if (error == null) {
        return "";
    }

    if (error.kind === CompileErrorKind.ParseError) {
        return "At line " + error.line + ", column " + error.column + ": " + error.message;
    } else {
        return error.message;
    }
}

async function compileVerb(context: Context, object: ObjectRef, verb: string, code): Promise<CompileError> {
    const mrpc_object = new MoorRemoteObject(object, context.authToken);
    const result = await mrpc_object.compileVerb(verb, code);
    if (!result) {
        return null;
    }
    if (result["ParseError"]) {
        const pe = result["ParseError"];
        pe.kind = CompileErrorKind.ParseError;
        return pe;
    }
    if (result["message"] == undefined) {
        return null;
    }
    return { kind: CompileErrorKind.Other, message: "Unknown error" };
}

export function showVerbEditor(
    context: Context,
    title: string,
    object: ObjectRef,
    verb: string,
    content: Array<string>,
) {
    const editor_state = van.state({ model: null });
    const compile_error_state = van.state(null);

    // Where the monaco editor itself lives.
    const editor_div = div(
        {
            style: "width: 100%; height: 100%;",
        },
    );

    const hidden_style = van.derive(() => {
            return compile_error_state.val != null && compile_error_state.val["message"] != undefined;
        })
        ? "width: 100%; height: 64px; display: block;"
        : "width: 100%; height: 0px; display: none;";
    const compile_errors = div(
        {
            style: hidden_style,
            class: "verb_compile_errors",
        },
        () => pre(transformErrors(compile_error_state.val)),
    );

    // Surrounding container with compile button and whatever else we might need
    const container_div = div(
        {
            class: "editor_container",
        },
        button(
            {
                onclick: async () => {
                    compile_error_state.val = await compileVerb(
                        context,
                        object,
                        verb,
                        editor_state.val.model.getValue(),
                    );
                },
            },
            "Compile",
        ),
        () => compile_errors,
        editor_div,
    );

    const editor = div(
        FloatingWindow(
            {
                title: title,
                width: 600,
                height: 800,
            },
            container_div,
        ),
    );
    document.body.appendChild(editor);

    // Now hang the editor off it.
    const model = createEditor(editor_div);
    editor_state.val = { model: model };
    updateEditor(model, content);
}

export function launchVerbEditor(context: Context, title: string, object: ObjectRef, verb: string) {
    // First things first, retrieve the verb.
    // Decode the 'object' as a reference to an object, in curie form.
    if (!context.authToken) {
        console.error("No auth token available for verb editor");
        return;
    }
    const mrpc_object = new MoorRemoteObject(object, context.authToken);
    mrpc_object.getVerbCode(verb).then((result) => {
        console.log("Got verb code: " + result);
        showVerbEditor(context, title, object, verb, result);
    });
}
