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

// Handle the connect phase in response to a user clicking the "Connect" button.
import { Context, Player } from "./model";

import van, { State } from "vanjs-core";
import { displayDjot, handleEvent } from "./narrative";

const { button, div, input, select, option, br, label } = van.tags;

async function connect(context: Context, player: State<Player>, mode, username, password) {
    let url = "/auth/" + mode;
    let data = new URLSearchParams();
    data.set("player", username.value);
    data.set("password", password.value);
    let result = await fetch(url, {
        method: "POST",
        body: data,
    });
    if (result.ok) {
        console.log("Connected!");
    } else {
        console.log("Failed to connect!");
        context.systemMessage.show("Failed to connect!", 3);
        return;
    }
    let login_result = await result.text();
    let login_components = login_result.split(" ");
    let player_oid = login_components[0];
    let auth_token = result.headers.get("X-Moor-Auth-Token");
    if (!auth_token) {
        console.log("No token; authorization denied");
        alert("Could not authenticate!");
        return;
    }

    // Authorized but not connected.
    player.val = new Player(player_oid, auth_token, false);

    // Now initiate the websocket connection.
    const baseUrl = window.location.host;
    // Choose ws or wss depending on whether we're using HTTPS.
    let isSecure = window.location.protocol === "https:";
    const wsUrl = (isSecure ? "wss://" : "ws://") + baseUrl + "/ws/attach/" + mode + "/" + auth_token;
    let ws = new WebSocket(wsUrl);
    ws.onopen = () => {
        player.val = new Player(player_oid, auth_token, true);

        // Move focus to input area.
        // This is done in a timer because it can't actually work until after the dom style is updated.
        setTimeout(() => document.getElementById("input_area").focus(), 0.5);
    };
    ws.onmessage = (e) => {
        if (e.data) {
            handleEvent(context, e.data);
        }
    };
    ws.onclose = () => {
        console.log("Connection closed!");
        player.val = new Player(player_oid, auth_token, false);
    };

    context.ws = ws;
    context.authToken = auth_token;
}

// A login box that prompts the user for their player name and password, and then initiates login through
// /auth/connect (if connecting) or /auth/create (if creating).
export const Login = (context: Context, player: State<Player>, login_message: State<string>) => {
    const mode_select = select(
        { id: "mode_select" },
        option({ value: "connect" }, "Connect"),
        option({ value: "create" }, "Create"),
    );

    let connect_callback = () => connect(context, player, mode_select.value, username, password);
    const welcome = van.derive(() => div({ class: "welcome_box" }, displayDjot({ djot_text: login_message })));

    const username = input({
        id: "login_username",
        type: "text",
        onkeyup: (e) => {
            if (e.key === "Enter") {
                connect_callback();
            }
        },
    });
    const password = input({
        id: "login_password",
        type: "password",
        onkeyup: (e) => {
            if (e.key === "Enter") {
                connect_callback();
            }
        },
    });
    const go_button = button({ onclick: connect_callback }, "Go");
    let hidden_style = van.derive(() => !player.val.connected ? "display: block;" : "display: none;");

    return div(
        {
            class: "login_window",
            style: hidden_style,
        },
        welcome,
        br,
        div(
            {
                class: "login_prompt",
            },
            mode_select,
            " ",
            label(
                { for: "login_username" },
                "Player: ",
                username,
            ),
            " ",
            label(
                { for: "login_password" },
                "Password: ",
                password,
            ),
            " ",
            go_button,
        ),
    );
};
