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
import { Presentation } from "../types/presentation";
import { ContentRenderer } from "./ContentRenderer";

interface PanelProps {
    presentation: Presentation;
    onClose: (id: string) => void;
    className: string;
    titleClassName: string;
    contentClassName: string;
    closeButtonClassName: string;
    onLinkClick?: (url: string) => void;
}

export const Panel: React.FC<PanelProps> = ({
    presentation,
    onClose,
    className,
    titleClassName,
    contentClassName,
    closeButtonClassName,
    onLinkClick,
}) => {
    const handleClose = () => {
        onClose(presentation.id);
    };

    return (
        <div className={className}>
            <div className={titleClassName}>
                <span>{presentation.title}</span>
                <button
                    className={closeButtonClassName}
                    onClick={handleClose}
                    aria-label={`Close ${presentation.title}`}
                >
                    ×
                </button>
            </div>
            <div className={contentClassName}>
                <ContentRenderer
                    content={presentation.content}
                    contentType={presentation.contentType}
                    onLinkClick={onLinkClick}
                />
            </div>
        </div>
    );
};
