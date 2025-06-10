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
 * Theme toggle component for switching between light and dark modes
 *
 * @returns A button that toggles between light and dark themes, hidden until hover
 */
const ThemeToggle = () => {
    // Check if user has a saved theme preference
    const savedTheme = localStorage.getItem("theme");
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;

    // Initialize theme state (use saved preference, fallback to system preference, default to dark)
    const isDarkTheme = van.state(savedTheme ? savedTheme === "dark" : prefersDark);

    // Apply the theme class on initial load
    if (!isDarkTheme.val) {
        document.body.classList.add("light-theme");
    }

    // Toggle theme function
    const toggleTheme = () => {
        isDarkTheme.val = !isDarkTheme.val;
        if (isDarkTheme.val) {
            document.body.classList.remove("light-theme");
            localStorage.setItem("theme", "dark");
        } else {
            document.body.classList.add("light-theme");
            localStorage.setItem("theme", "light");
        }
    };

    // Return hover area with toggle button inside
    return div(
        { class: "theme-toggle-area" },
        button(
            {
                class: "theme-toggle",
                onclick: toggleTheme,
            },
            () => (isDarkTheme.val ? "Switch to Light Theme" : "Switch to Dark Theme"),
        ),
    );
};

/**
 * Creates a dock component for the specified dock type
 *
 * @param context - Application context containing presentations and other state
 * @param dockType - The type of dock ("left-dock", "right-dock", "top-dock", "bottom-dock")
 * @param dockClass - The CSS class for the dock container
 * @param panelClass - The CSS class for individual panels
 * @returns DOM element containing the dock and its panels
 */
const createDock = (context: Context, dockType: string, dockClass: string, panelClass: string) => {
    const presentations = context.presentations;

    // Show dock only when presentations exist
    // Use flex for top/bottom docks to enable horizontal layout
    const displayType = (dockType === "top-dock" || dockType === "bottom-dock") ? "flex" : "block";
    const hidden_style = van.derive(() => {
        const length = presentations.val.getPresentationsForTarget(dockType).length;
        return length > 0 ? `display: ${displayType};` : "display: none;";
    });

    const panels = div({
        class: dockClass,
        style: hidden_style,
    });

    // Reactively update panels when presentations change
    van.derive(() => {
        // Clear existing content
        panels.innerHTML = "";

        // Create panels for each active presentation
        for (const presentationId of presentations.val.getPresentationsForTarget(dockType)) {
            const presentation = presentations.val.getPresentation(presentationId);
            if (!presentation || presentation.val.closed.val) {
                continue;
            }

            // Create panel with title bar and content
            panels.appendChild(div(
                {
                    id: presentationId,
                    class: panelClass,
                },
                div(
                    {
                        class: `${panelClass}_title`,
                    },
                    button(
                        {
                            class: `${panelClass}_close`,
                            onclick: () => {
                                console.log(`Closing ${dockType} presentation:`, presentationId);
                                // Use the proper dismiss handler
                                handleUserClosePresentation(context, presentationId);
                            },
                        },
                        "×",
                    ),
                    van.derive(() => presentation.val.attrs["title"] || presentationId),
                ),
                div(
                    {
                        class: `${panelClass}_content`,
                    },
                    presentation.val.content,
                ),
            ));
        }
    });

    return panels;
};

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
    return createDock(context, "right-dock", "right_dock", "right_dock_panel");
};

/**
 * LeftDock Component - Similar to RightDock but positioned on the left
 */
const LeftDock = (context: Context) => {
    return createDock(context, "left-dock", "left_dock", "left_dock_panel");
};

/**
 * TopDock Component - Positioned at the top of the interface
 */
const TopDock = (context: Context) => {
    return createDock(context, "top-dock", "top_dock", "top_dock_panel");
};

/**
 * BottomDock Component - Positioned at the bottom of the interface
 */
const BottomDock = (context: Context) => {
    return createDock(context, "bottom-dock", "bottom_dock", "bottom_dock_panel");
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
        // Theme toggle button
        ThemeToggle(),
        // Login component (shows/hides based on connection state)
        Login(context, player, welcomeMessage),
        // Main application layout with all docks
        div(
            { class: "app_layout" },
            // Top dock for horizontal panels
            TopDock(context),
            // Middle section with left dock, narrative, and right dock
            div(
                { class: "middle_section" },
                // Left dock for auxiliary UI panels
                LeftDock(context),
                // Main narrative display with command input
                Narrative(context, player),
                // Right dock for auxiliary UI panels
                RightDock(context),
            ),
            // Bottom dock for horizontal panels
            BottomDock(context),
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
