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

import React from "react";
import { Presentation } from "../../types/presentation";
import { Panel } from "../Panel";

interface TopDockProps {
    presentations: Presentation[];
    onClosePresentation: (id: string) => void;
    onLinkClick?: (url: string) => void;
}

export const TopDock: React.FC<TopDockProps> = ({ presentations, onClosePresentation, onLinkClick }) => {
    if (presentations.length === 0) {
        return null;
    }

    return (
        <div className="top_dock" style={{ display: "flex" }}>
            <h2 className="sr-only">Top Dock Panels</h2>
            {presentations.map((presentation) => (
                <Panel
                    key={presentation.id}
                    presentation={presentation}
                    onClose={onClosePresentation}
                    className="top_dock_panel"
                    titleClassName="top_dock_panel_title"
                    contentClassName="top_dock_panel_content"
                    closeButtonClassName="top_dock_panel_close"
                    onLinkClick={onLinkClick}
                />
            ))}
        </div>
    );
};
