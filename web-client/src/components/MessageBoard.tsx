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

import React, { useCallback, useState } from "react";

interface MessageBoardProps {
    className?: string;
}

interface SystemMessage {
    message: string;
    visible: boolean;
}

/**
 * Hook for managing system message state
 */
export const useSystemMessage = () => {
    const [systemMessage, setSystemMessage] = useState<SystemMessage>({
        message: "",
        visible: false,
    });

    const showMessage = useCallback((message: string, duration: number = 5) => {
        setSystemMessage({ message, visible: true });

        setTimeout(() => {
            setSystemMessage(prev => ({ ...prev, visible: false }));
        }, duration * 1000);
    }, []);

    const hideMessage = useCallback(() => {
        setSystemMessage(prev => ({ ...prev, visible: false }));
    }, []);

    return {
        systemMessage,
        showMessage,
        hideMessage,
    };
};

/**
 * Displays a temporary notification message that automatically disappears.
 *
 * @param props - Component properties
 * @returns A React component
 */
export const MessageBoard: React.FC<
    MessageBoardProps & {
        message: string;
        visible: boolean;
    }
> = ({ message, visible, className = "" }) => {
    const displayStyle = visible ? "block" : "none";

    return (
        <div
            className={`message_board ${className}`}
            style={{ display: displayStyle }}
            role="status"
            aria-live="polite"
            aria-atomic="true"
        >
            {message}
        </div>
    );
};
