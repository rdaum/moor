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

// Hook for managing text editor state and operations
// Handles editing freeform text content with verb-based save callbacks

import { useCallback, useState } from "react";

export interface TextEditorSession {
    id: string;
    title: string;
    description: string; // Explanatory blurb shown to user
    objectCurie: string;
    verbName: string;
    sessionId?: string; // Optional session ID passed as first arg on save
    content: string;
    contentType: "text/plain" | "text/djot";
    textMode: "string" | "list"; // How to send content: single string or list of strings
    presentationId: string;
}

export const useTextEditor = () => {
    const [textEditorSession, setTextEditorSession] = useState<TextEditorSession | null>(null);

    // Show text editor with provided content and save callback info
    const showTextEditor = useCallback((
        id: string,
        title: string,
        description: string,
        objectCurie: string,
        verbName: string,
        sessionId: string | undefined,
        content: string,
        contentType: "text/plain" | "text/djot",
        textMode: "string" | "list",
        presentationId: string,
    ) => {
        setTextEditorSession({
            id,
            title,
            description,
            objectCurie,
            verbName,
            sessionId,
            content,
            contentType,
            textMode,
            presentationId,
        });
    }, []);

    // Close the text editor
    const closeTextEditor = useCallback(() => {
        setTextEditorSession(null);
    }, []);

    return {
        textEditorSession,
        showTextEditor,
        closeTextEditor,
    };
};
