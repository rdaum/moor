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

/**
 * Login Module
 *
 * This module handles the user authentication flow, including:
 * - Displaying the login form UI
 * - Processing login/create account requests
 * - Establishing WebSocket connections
 * - Managing authentication state
 *
 * The module provides both the UI components and the authentication logic
 * needed to establish a connection to the Moor server.
 */

import van, { State } from "vanjs-core";
import { Context, Player } from "./model";
import { displayDjot, handleEvent, fetchAndDisplayHistory } from "./narrative";

// Extract commonly used VanJS elements
const { button, div, input, select, option, br, label } = van.tags;

/**
 * Initiates authentication and connection to the Moor server
 *
 * This function handles the entire authentication flow including:
 * 1. Sending credentials to the server
 * 2. Processing the authentication response
 * 3. Establishing a WebSocket connection
 * 4. Setting up event handlers for the connection
 *
 * @param context - Application context to update with connection info
 * @param player - State object to update with player information
 * @param mode - Connection mode ('connect' for login or 'create' for new account)
 * @param username - Input element containing the username
 * @param password - Input element containing the password
 */
async function connect(
    context: Context,
    player: State<Player>,
    mode: string,
    username: HTMLInputElement,
    password: HTMLInputElement,
) {
    try {
        // Validate inputs
        if (!username.value || !username.value.trim()) {
            context.systemMessage.show("Please enter a username", 3);
            username.focus();
            return;
        }

        if (!password.value) {
            context.systemMessage.show("Please enter a password", 3);
            password.focus();
            return;
        }

        // Build authentication request
        const url = `/auth/${mode}`;
        const data = new URLSearchParams();
        data.set("player", username.value.trim());
        data.set("password", password.value);

        // Show connecting status
        context.systemMessage.show(`Connecting to server...`, 2);

        // Send authentication request
        const result = await fetch(url, {
            method: "POST",
            body: data,
        });

        // Handle HTTP errors
        if (!result.ok) {
            const errorMessage = result.status === 401
                ? "Invalid username or password"
                : `Failed to connect (${result.status}: ${result.statusText})`;

            console.error(`Authentication failed: ${result.status}`, result);
            context.systemMessage.show(errorMessage, 5);
            return;
        }

        // Parse authentication response
        const loginResult = await result.text();
        const loginComponents = loginResult.split(" ");
        const playerOid = loginComponents[0];
        const authToken = result.headers.get("X-Moor-Auth-Token");
        context.authToken = authToken;

        // Validate authentication token
        if (!authToken) {
            console.error("Authentication failed: No token received");
            context.systemMessage.show("Authentication failed: No token received", 5);
            return;
        }

        // Update player state (authorized but not yet connected)
        player.val = new Player(playerOid, authToken, false);
        context.systemMessage.show("Authenticated! Loading history...", 2);

        // Fetch and display historical events BEFORE opening WebSocket
        // This ensures a clean temporal boundary between historical and live events
        try {
            await fetchAndDisplayHistory(context, 1000);
            console.log("History loaded successfully, now establishing WebSocket connection");
        } catch (error) {
            console.error("Failed to load history:", error);
            // Continue with connection even if history fails
        }

        context.systemMessage.show("Establishing connection...", 2);

        // Establish WebSocket connection AFTER history is loaded
        const baseUrl = window.location.host;
        const isSecure = window.location.protocol === "https:";
        const wsUrl = `${isSecure ? "wss://" : "ws://"}${baseUrl}/ws/attach/${mode}/${authToken}`;

        const ws = new WebSocket(wsUrl);

        // Set up WebSocket event handlers
        ws.onopen = () => {
            // Update player state to connected
            player.val = new Player(playerOid, authToken, true);
            context.systemMessage.show("Connected!", 2);

            // Move focus to input area after UI updates
            setTimeout(() => {
                const inputArea = document.getElementById("input_area");
                if (inputArea) inputArea.focus();
            }, 100); // Use slightly longer timeout for reliability
        };

        ws.onmessage = (e) => {
            if (e.data) {
                handleEvent(context, e.data);
            }
        };

        ws.onerror = (error) => {
            console.error("WebSocket error:", error);
            context.systemMessage.show("Connection error", 5);
        };

        ws.onclose = (event) => {
            console.log(`Connection closed (${event.code}: ${event.reason})`);
            player.val = new Player(playerOid, authToken, false);

            if (event.code !== 1000) { // 1000 is normal closure
                context.systemMessage.show(
                    `Connection closed: ${event.reason || "Server disconnected"}`,
                    5,
                );
            }
        };

        // Update application context
        context.ws = ws;
    } catch (error) {
        // Handle any unexpected errors
        console.error("Connection error:", error);
        context.systemMessage.show(
            `Connection error: ${error instanceof Error ? error.message : "Unknown error"}`,
            5,
        );
    }
}

/**
 * Login Component
 *
 * Renders a login form that allows users to either connect to an existing
 * account or create a new one. The component automatically hides when
 * the user is connected and shows when disconnected.
 *
 * Features:
 * - Toggle between connect/create modes
 * - Welcome message display using Djot format
 * - Input validation
 * - Enter key support for submission
 * - Automatic visibility management based on connection state
 *
 * @param context - Application context
 * @param player - Player state information
 * @param loginMessage - Welcome message to display (in Djot format)
 * @returns Login form component
 */
export const Login = (context: Context, player: State<Player>, loginMessage: State<string>) => {
    // Create form elements
    const modeSelect = select(
        { id: "mode_select" },
        option({ value: "connect" }, "Connect"),
        option({ value: "create" }, "Create"),
    );

    const username = input({
        id: "login_username",
        type: "text",
        placeholder: "Username",
        autocomplete: "username",
        spellcheck: "false",
    });

    const password = input({
        id: "login_password",
        type: "password",
        placeholder: "Password",
        autocomplete: "current-password",
    });

    // Initialize connect function that will be called on submit
    const handleConnect = () => connect(context, player, modeSelect.value, username, password);

    // Add Enter key handler to both input fields
    username.onkeyup = password.onkeyup = (e) => {
        if (e.key === "Enter") {
            e.preventDefault();
            handleConnect();
        }
    };

    // Create submit button
    const goButton = button({
        onclick: handleConnect,
        class: "login_button",
    }, "Go");

    // Create welcome message component using Djot rendering
    const welcome = van.derive(() => div({ class: "welcome_box" }, displayDjot({ djot_text: loginMessage })));

    // Show login form only when not connected
    const visibilityStyle = van.derive(() => !player.val.connected ? "display: block;" : "display: none;");

    // Assemble the login form
    return div(
        {
            class: "login_window",
            style: visibilityStyle,
        },
        // Welcome message display
        welcome,
        br,
        // Login form
        div(
            { class: "login_prompt" },
            // Connect/Create selector
            modeSelect,
            " ",
            // Username field with label
            label(
                { for: "login_username", class: "login_label" },
                "Player: ",
                username,
            ),
            " ",
            // Password field with label
            label(
                { for: "login_password", class: "login_label" },
                "Password: ",
                password,
            ),
            " ",
            // Submit button
            goButton,
        ),
    );
};
