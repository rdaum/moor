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

import { useCallback, useState } from "react";

export interface EditorSession {
    id: string;
    title: string;
    objectCurie: string;
    verbName: string;
    content: string;
    uploadAction?: string; // For MCP-triggered editors
}

export const useVerbEditor = () => {
    const [editorSession, setEditorSession] = useState<EditorSession | null>(null);

    // Launch verb editor with specific content
    const launchVerbEditor = useCallback(async (
        title: string,
        objectCurie: string,
        verbName: string,
        authToken: string,
    ) => {
        try {
            // Fetch verb content from server
            const response = await fetch(`/verbs/${encodeURIComponent(objectCurie)}/${encodeURIComponent(verbName)}`, {
                method: "GET",
                headers: {
                    "X-Moor-Auth-Token": authToken,
                },
            });

            if (!response.ok) {
                throw new Error(`Failed to fetch verb: ${response.status} ${response.statusText}`);
            }

            const content = await response.text();

            // Create editor session
            setEditorSession({
                id: `${objectCurie}:${verbName}`,
                title,
                objectCurie,
                verbName,
                content,
            });
        } catch (error) {
            console.error("Error launching verb editor:", error);
        }
    }, []);

    // Show verb editor with provided content (for MCP workflow)
    const showVerbEditor = useCallback((
        title: string,
        objectCurie: string,
        verbName: string,
        content: string,
        uploadAction?: string,
    ) => {
        setEditorSession({
            id: `${objectCurie}:${verbName}`,
            title,
            objectCurie,
            verbName,
            content,
            uploadAction,
        });
    }, []);

    // Close the editor
    const closeEditor = useCallback(() => {
        setEditorSession(null);
    }, []);

    return {
        editorSession,
        launchVerbEditor,
        showVerbEditor,
        closeEditor,
    };
};
