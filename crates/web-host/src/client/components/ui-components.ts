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
 * UI Components Module
 *
 * This module contains reusable UI components that can be used across the application.
 * These components are built using the VanJS framework and follow a functional approach
 * to UI development.
 */

import van, { State } from "vanjs-core";
import { Notice } from "../model";

const { div, span, button } = van.tags;

/**
 * Displays a temporary notification message that automatically disappears.
 *
 * @param notice - The notice state to display
 * @returns A VanJS component
 */
export const MessageBoard = (notice: State<Notice>) => {
    const hidden_style = van.derive(() => notice.val.visible.val ? "display: block;" : "display: none;");

    return div(
        {
            class: "message_board",
            style: hidden_style,
        },
        notice.val.message,
    );
};

/**
 * Creates a panel with a header that includes a title and close button
 *
 * @param props - Component properties
 * @param props.id - Unique identifier for the panel
 * @param props.title - Title to display in the panel header
 * @param props.onClose - Callback function when the panel is closed
 * @param props.className - Additional CSS class names
 * @param content - The content to display in the panel body
 * @returns A VanJS component
 */
export const Panel = ({ id, title, onClose, className = "" }, ...content) => {
    return div(
        {
            id,
            class: `panel ${className}`,
        },
        // Panel header with title and close button
        div(
            {
                class: "panel-header",
            },
            span(
                {
                    class: "panel-title",
                },
                title,
            ),
            button(
                {
                    class: "panel-close",
                    onclick: onClose,
                },
                "×",
            ),
        ),
        // Panel content container
        div(
            {
                class: "panel-content",
            },
            ...content,
        ),
    );
};
