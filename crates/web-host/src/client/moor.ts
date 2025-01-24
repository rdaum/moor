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

import van, { State } from "vanjs-core";

import { Tabs } from "van-ui";
import * as vanX from "vanjs-ext";
import { Login } from "./login";
import {
    addPresentation,
    Context,
    Player,
    Presentation,
    PresentationModel,
    Presentations,
    rightDockPresentations,
} from "./model";
import { htmlPurifySetup, Narrative } from "./narrative";
import { retrieveWelcome } from "./rpc";

const { button, div, span, input, select, option, br, pre, form, a, p } = van.tags;

export class Notice {
    message: State<string | null>;
    visible: State<boolean>;

    constructor() {
        this.message = van.state("");
        this.visible = van.state(false);
    }

    show(message, duration) {
        this.message.val = message;
        this.visible.val = true;
        setTimeout(() => {
            this.visible.val = false;
        }, duration * 1000);
    }
}

// Displays notices from the system, such as "Connection lost" or "Connection established".
// Fade out after a few seconds.
const MessageBoard = (notice: State<Notice>) => {
    let hidden_style = van.derive(() => notice.val.visible.val ? "display: block;" : "display: none;");
    return div(
        {
            class: "message_board",
            style: hidden_style,
        },
        notice.val.message,
    );
};

const RightDock = (presentations: State<Presentations>) => {
    // Whether this is hidden or not depends on the presence of right-dock presentations in the context
    let hidden_style = van.derive(() => {
        let length = presentations.val.rightDockPresentations().length;
        return length > 0 ? "display: block;" : "display: none;";
    });

    let panels = div({
        class: "right_dock",
        style: hidden_style,
    });
    van.derive(() => {
        // clear existing children
        panels.innerHTML = "";
        for (const presentation_id of presentations.val.rightDockPresentations()) {
            let presentation: State<PresentationModel> = presentations.val.presentations[presentation_id];
            if (presentation.val.closed.val) {
                continue;
            }
            console.log("Presentation: ", presentation.val, " @ ", presentation_id);
            panels.appendChild(div(
                {
                    id: presentation_id,
                    class: "right_dock_panel",
                },
                span(
                    {
                        class: "right_dock_panel_title",
                    },
                    span(
                        {
                            class: "right_dock_panel_close",
                            onclick: () => {
                                console.log("Closing presentation: ", presentation_id);
                                presentation.val.closed.val = true;
                            },
                        },
                        "X",
                    ),
                    van.derive(() => presentation.val.attrs["title"]),
                ),
                presentation.val.content,
            ));
        }
    });
    return panels;
};

const App = (context: Context) => {
    const player = van.state(context.player);
    const welcome_message = van.state("");

    van.derive(() => {
        retrieveWelcome().then((msg) => {
            welcome_message.val = msg;
        });
    });

    const dom = div({
        class: "main",
    });

    return div(
        dom,
        MessageBoard(van.state(context.systemMessage)),
        Login(context, player, welcome_message),
        div(
            {
                class: "columns_grid",
            },
            Narrative(context, player),
            RightDock(context.presentations),
        ),
    );
};

htmlPurifySetup();

export const context = new Context();

van.add(document.body, App(context));
