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

import {retrieveWelcome} from "./rpc";

import {Context} from "./model";
import {Login} from "./login";
import {htmlPurifySetup, Narrative} from "./narrative";

const {button, div, span, input, select, option, br, pre, form, a, p} = van.tags;


export class Notice {
    message: State<string | null>;
    visible: State<boolean>;

    constructor() {
        this.message = van.state("");
        this.visible = van.state(false);
    }

    show(message, duration) {
        this.message.val = message;
        console.log("Showing notice: " + message);
        this.visible.val = true;
        setTimeout(() => {
            this.visible.val = false;
            console.log("Hiding notice");
        }, duration * 1000);
    }

}
// Displays notices from the system, such as "Connection lost" or "Connection established".
// Fade out after a few seconds.
const MessageBoard = (notice : State<Notice>) => {
    let hidden_style = van.derive(() => notice.val.visible.val ? "display: block;" : "display: none;");
    return div(
        {
            class: "message_board",
            style: hidden_style
        },
        notice.val.message
    );
}


const App = (context: Context) => {
    const player = van.state(context.player);
    const welcome_message = van.state("");

    van.derive(() => {
        retrieveWelcome().then((msg) => {
            welcome_message.val = msg;
        });
    });

    const dom = div({
        class: "main"
    });

    return div(
        dom,
        MessageBoard(van.state(context.systemMessage)),
        Login(context, player, welcome_message),
        Narrative(context, player)
    );
};

htmlPurifySetup();

console.log("Context: ", Context);
export const context = new Context();
van.add(document.body, App(new Context()));
