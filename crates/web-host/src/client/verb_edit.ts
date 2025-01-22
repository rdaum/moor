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

import van, {State} from "vanjs-core";
import {FloatingWindow} from "van-ui";

import {Context} from "./model";
import {ObjectRef} from "./var";
import {MoorRemoteObject} from "./rpc";
import {createEditor, updateEditor} from "./editor";

const {button, div, span, input, select, option, br, pre, form, a, p} = van.tags;

enum CompileErrorKind {
    ParseError,
    Other
}

interface ParseError {
    kind: CompileErrorKind.ParseError
    line: number,
    column: number,
    context: string,
    end_line_col: [number, number] | null,
    message : string
}

interface OtherError {
    kind: CompileErrorKind.Other,
    message: string
}
type CompileError = ParseError | OtherError;


function transformErrors(error : CompileError | null) {
    if (error == null) {
        return "";
    }

    if (error.kind === CompileErrorKind.ParseError) {
        return "At line " + error.line + ", column " + error.column + ": " + error.message;
    } else {
        return error.message;
    }
}

async function compileVerb(context: Context, object : ObjectRef, verb : string, code) : Promise<CompileError> {
    let mrpc_object = new MoorRemoteObject(object, context.authToken);
    let result = await mrpc_object.compileVerb(verb, code);
    if (!result) {
        return null;
    }
    if (result["ParseError"]) {
        let pe = result["ParseError"];
        pe.kind = CompileErrorKind.ParseError;
        return pe;
    }
    if (result["message"] == undefined) {
        return null;
    }
    return {kind: CompileErrorKind.Other, message: "Unknown error"};
}

export function showVerbEditor(context : Context, title: string, object: ObjectRef, verb: string, content: Array<string>) {
    let editor_state = van.state({model: null});
    let compile_error_state = van.state(null);

    // Where the monaco editor itself lives.
    let editor_div = div(
        {
            style: "width: 100%; height: 100%;"
        }
    );

    let hidden_style = van.derive(() => compile_error_state.val["message"] != undefined)
        ? "width: 100%; height: 64px; display: block;"
        : "width: 100%; height: 0px; display: none;";
    let compile_errors = div(
        {
            style: hidden_style,
            class: "verb_compile_errors"
        },
        () => pre(transformErrors(compile_error_state.val))
    );

    // Surrounding container with compile button and whatever else we might need
    let container_div = div(
        {
            class: "editor_container"
        },
        button(
            {
                onclick: async () => {
                    compile_error_state.val = await compileVerb(context, object, verb, editor_state.val.model.getValue());
                }
            },
            "Compile"
        ),
        () => compile_errors,
        editor_div
    );

    let editor = div(
        FloatingWindow(
            {
                parentDom: document.body,
                title: title,
                id: "editor",
                width: 500,
                height: 300,
            },
            container_div
        )
    );
    document.body.appendChild(editor);

    // Now hang the editor off it.
    let model = createEditor(editor_div);
    editor_state.val = {model: model};
    updateEditor(model, content);
}

export function launchVerbEditor(context: Context, title: string, object : ObjectRef, verb : string) {
    // First things first, retrieve the verb.
    // Decode the 'object' as a reference to an object, in curie form.
    let mrpc_object = new MoorRemoteObject(object, context.authToken);
    mrpc_object.getVerbCode(verb).then((result) => {
        showVerbEditor(context, title, object, verb, result.split("\n"));
    });
}

