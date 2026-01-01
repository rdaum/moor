// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

// ! Font size control for narrative text

import React from "react";

interface FontSizeControlProps {
    fontSize: number;
    onDecrease: () => void;
    onIncrease: () => void;
    minSize?: number;
    maxSize?: number;
}

export const FontSizeControl: React.FC<FontSizeControlProps> = ({
    fontSize,
    onDecrease,
    onIncrease,
    minSize = 10,
    maxSize = 24,
}) => {
    return (
        <div className="font-size-control">
            <button
                onClick={onDecrease}
                className="font-size-button"
                disabled={fontSize <= minSize}
                aria-label="Decrease font size"
            >
                –
            </button>
            <span className="font-size-display" aria-live="polite">
                {fontSize}px
            </span>
            <button
                onClick={onIncrease}
                className="font-size-button"
                disabled={fontSize >= maxSize}
                aria-label="Increase font size"
            >
                +
            </button>
        </div>
    );
};
