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
import { useCarouselOverflow } from "../../hooks/useCarouselOverflow";
import { Presentation } from "../../types/presentation";
import { Panel } from "../Panel";

interface TopDockProps {
    presentations: Presentation[];
    onClosePresentation: (id: string) => void;
    onLinkClick?: (url: string, position?: { x: number; y: number }) => void;
    onLinkHoldStart?: (url: string, position: { x: number; y: number }) => void;
    onLinkHoldEnd?: () => void;
}

export const TopDock: React.FC<TopDockProps> = (
    { presentations, onClosePresentation, onLinkClick, onLinkHoldStart, onLinkHoldEnd },
) => {
    const { containerRef, hasOverflow, hasScroll } = useCarouselOverflow();

    if (presentations.length === 0) {
        return null;
    }

    // Debug logging for React state
    // console.log('TopDock render:', { hasOverflow, hasScroll });

    const className = [
        "top_dock",
        hasOverflow && "has-overflow",
        hasScroll && "has-scroll",
    ].filter(Boolean).join(" ");

    return (
        <>
            <div ref={containerRef} className={className} style={{ display: "flex" }}>
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
                        onLinkHoldStart={onLinkHoldStart}
                        onLinkHoldEnd={onLinkHoldEnd}
                    />
                ))}
            </div>
            {hasOverflow && (
                <div
                    style={{
                        position: "absolute",
                        top: "50%",
                        right: "8px",
                        transform: "translateY(-50%)",
                        fontSize: "24px",
                        color: "white",
                        background: "rgba(0, 0, 0, 0.8)",
                        borderRadius: "50%",
                        width: "36px",
                        height: "36px",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        fontWeight: "bold",
                        pointerEvents: "none",
                        zIndex: 1000,
                    }}
                >
                    ›
                </div>
            )}
            {hasScroll && (
                <div
                    style={{
                        position: "absolute",
                        top: "50%",
                        left: "8px",
                        transform: "translateY(-50%)",
                        fontSize: "24px",
                        color: "white",
                        background: "rgba(0, 0, 0, 0.8)",
                        borderRadius: "50%",
                        width: "36px",
                        height: "36px",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        fontWeight: "bold",
                        pointerEvents: "none",
                        zIndex: 1000,
                    }}
                >
                    ‹
                </div>
            )}
        </>
    );
};
