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

interface DialogSheetProps {
    title: string;
    titleId: string;
    onCancel: () => void;
    children: React.ReactNode;
    maxWidth?: string;
    role?: "dialog" | "alertdialog";
    ariaDescribedBy?: string;
}

export const DialogSheet: React.FC<DialogSheetProps> = ({
    title,
    titleId,
    onCancel,
    children,
    maxWidth = "520px",
    role = "dialog",
    ariaDescribedBy,
}) => {
    return (
        <>
            <div className="dialog-sheet-backdrop" onClick={onCancel} role="presentation" aria-hidden="true" />
            <div
                className="dialog-sheet"
                style={{ maxWidth }}
                role={role}
                aria-modal="true"
                aria-labelledby={titleId}
                aria-describedby={ariaDescribedBy}
            >
                <div className="dialog-sheet-header">
                    <h2 id={titleId}>{title}</h2>
                </div>
                {children}
            </div>
        </>
    );
};
