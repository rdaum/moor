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

interface BottomDockProps {
    presentations: Presentation[];
    onClosePresentation: (id: string) => void;
    onLinkClick?: (url: string) => void;
}

export const BottomDock: React.FC<BottomDockProps> = ({ presentations, onClosePresentation, onLinkClick }) => {
    if (presentations.length === 0) {
        return null;
    }

    return (
        <div className="bottom_dock" style={{ display: "flex" }}>
            <h2 className="sr-only">Bottom Dock Panels</h2>
            {presentations.map((presentation) => (
                <Panel
                    key={presentation.id}
                    presentation={presentation}
                    onClose={onClosePresentation}
                    className="bottom_dock_panel"
                    titleClassName="bottom_dock_panel_title"
                    contentClassName="bottom_dock_panel_content"
                    closeButtonClassName="bottom_dock_panel_close"
                    onLinkClick={onLinkClick}
                />
            ))}
        </div>
    );
};
