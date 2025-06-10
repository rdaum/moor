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
import { MessageBoard } from "./components/ui-components";
import { Login } from "./login";
import { Context, Notice, PresentationManager, PresentationModel } from "./model";
import { handleUserClosePresentation, htmlPurifySetup, Narrative } from "./narrative";
import { retrieveWelcome } from "./rpc";

const { button, div, span, input, select, option, br, pre, form, a, p } = van.tags;

/**
 * RightDock Component
 *
 * Renders a collection of panels in the right dock area of the UI.
 * Each panel represents a presentation from the server that has been
 * targeted to display in the right dock area.
 *
 * Features:
 * - Dynamically shows/hides based on available presentations
 * - Reactively updates when presentations change
 * - Provides close buttons for each panel
 * - Uses presentation titles from attributes when available
 *
 * @param context - Application context containing presentations and other state
 * @returns DOM element containing the right dock and its panels
 */
const RightDock = (context: Context) => {
    const presentations = context.presentations;

    // Show dock only when presentations exist
    const hidden_style = van.derive(() => {
        const length = presentations.val.rightDockPresentations().length;
        return length > 0 ? "display: block;" : "display: none;";
    });

    const panels = div({
        class: "right_dock",
        style: hidden_style,
    });

    // Reactively update panels when presentations change
    van.derive(() => {
        // Clear existing content
        panels.innerHTML = "";

        // Create panels for each active presentation
        for (const presentationId of presentations.val.rightDockPresentations()) {
            const presentation = presentations.val.getPresentation(presentationId);
            if (!presentation || presentation.val.closed.val) {
                continue;
            }

            // Create panel with title bar and content
            panels.appendChild(div(
                {
                    id: presentationId,
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
                                console.log("Closing right-dock presentation:", presentationId);
                                // Use the proper dismiss handler
                                handleUserClosePresentation(context, presentationId);
                            },
                        },
                        "X",
                    ),
                    van.derive(() => presentation.val.attrs["title"] || presentationId),
                ),
                presentation.val.content,
            ));
        }
    });

    return panels;
};

/**
 * App Component
 *
 * Main application component that orchestrates the entire UI layout.
 * This is the top-level component that coordinates all other components
 * and manages application-wide state.
 *
 * Layout structure:
 * - System notification area (MessageBoard)
 * - Login interface (shown when disconnected)
 * - Main content grid with:
 *   - Narrative display (game output and input area)
 *   - Right dock panel (for auxiliary UI elements)
 *
 * @param context - Global application context with shared state
 * @returns The complete application UI structure
 */
const App = (context: Context) => {
    // Create reactive state for player information
    const player = van.state(context.player);

    // State for welcome message (loaded asynchronously)
    const welcomeMessage = van.state("");

    // Load welcome message on startup
    van.derive(() => {
        retrieveWelcome()
            .then(msg => {
                welcomeMessage.val = msg;
            })
            .catch(error => {
                console.error("Failed to load welcome message:", error);
                welcomeMessage.val = "Welcome to Moor";
            });
    });

    // Main application structure
    return div(
        // Main container (primarily for styling)
        div({ class: "main" }),
        // System message notifications area (toast-style)
        MessageBoard(van.state(context.systemMessage)),
        // Login component (shows/hides based on connection state)
        Login(context, player, welcomeMessage),
        // Main content grid (narrative and panels)
        div(
            { class: "columns_grid" },
            // Main narrative display with command input
            Narrative(context, player),
            // Right dock for auxiliary UI panels
            RightDock(context),
        ),
    );
};

// ============================================================================
// Application Initialization
// ============================================================================

// Configure HTML sanitization for security
htmlPurifySetup();

// Create the global application context
export const context = new Context();

// Render the application to the document body
van.add(document.body, App(context));
