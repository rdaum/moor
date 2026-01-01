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

import { useCallback, useState } from "react";

export interface VerbMetadata {
    location: number;
    owner: number;
    names: string[];
    r: boolean; // readable
    w: boolean; // writable
    x: boolean; // executable
    d: boolean; // ???
    arg_spec: string[];
}

export interface EditorSession {
    id: string;
    title: string;
    objectCurie: string;
    verbName: string;
    content: string;
    uploadAction?: string; // For MCP-triggered editors
    verbMetadata?: VerbMetadata; // Verb metadata from server
    presentationId?: string; // ID of the presentation that triggered this editor
}

export const useVerbEditor = () => {
    const [editorSessions, setEditorSessions] = useState<EditorSession[]>([]);
    const [activeSessionIndex, setActiveSessionIndex] = useState(0);

    // Launch verb editor with specific content
    const launchVerbEditor = useCallback(async (
        _title: string,
        objectCurie: string,
        verbName: string,
        authToken: string,
        presentationId?: string,
    ) => {
        try {
            // Fetch verb content from server using FlatBuffer API
            const { getVerbCodeFlatBuffer } = await import("../lib/rpc-fb");
            const { objToString } = await import("../lib/var");

            const verbValue = await getVerbCodeFlatBuffer(authToken, objectCurie, verbName);

            // Extract code from FlatBuffer VerbValue
            const codeLength = verbValue.codeLength();
            const codeLines: string[] = [];
            for (let i = 0; i < codeLength; i++) {
                const line = verbValue.code(i);
                if (line) {
                    codeLines.push(line);
                }
            }
            const content = codeLines.join("\n");

            // Extract verb metadata from VerbInfo
            const verbInfo = verbValue.verbInfo();
            if (!verbInfo) {
                throw new Error("No verb info in response");
            }

            const location = objToString(verbInfo.location());
            const owner = objToString(verbInfo.owner());

            const namesLength = verbInfo.namesLength();
            const names: string[] = [];
            for (let i = 0; i < namesLength; i++) {
                const nameSymbol = verbInfo.names(i);
                const name = nameSymbol?.value();
                if (name) {
                    names.push(name);
                }
            }

            const argSpecLength = verbInfo.argSpecLength();
            const argSpec: string[] = [];
            for (let i = 0; i < argSpecLength; i++) {
                const argSymbol = verbInfo.argSpec(i);
                const arg = argSymbol?.value();
                if (arg) {
                    argSpec.push(arg);
                }
            }

            const verbMetadata: VerbMetadata = {
                location: location ? parseInt(location) : 0,
                owner: owner ? parseInt(owner) : 0,
                names,
                r: verbInfo.r(),
                w: verbInfo.w(),
                x: verbInfo.x(),
                d: verbInfo.d(),
                arg_spec: argSpec,
            };

            // Create a more descriptive title using verb metadata
            const verbTitle = `${verbMetadata.names[0]} on #${verbMetadata.location}`;

            // Create editor session
            const newSession: EditorSession = {
                id: `${objectCurie}:${verbName}`,
                title: verbTitle,
                objectCurie,
                verbName,
                content,
                verbMetadata,
                presentationId,
            };

            // Always add to array and set active to the newly added session
            setEditorSessions(prev => {
                const newSessions = [...prev, newSession];
                setActiveSessionIndex(newSessions.length - 1);
                return newSessions;
            });
        } catch (error) {
            console.error("Error launching verb editor:", error);
            throw error; // Re-throw to allow caller to handle
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
        const newSession: EditorSession = {
            id: `${objectCurie}:${verbName}`,
            title,
            objectCurie,
            verbName,
            content,
            uploadAction,
        };

        // Always add to array and set active to the newly added session
        setEditorSessions(prev => {
            const newSessions = [...prev, newSession];
            setActiveSessionIndex(newSessions.length - 1);
            return newSessions;
        });
    }, []);

    // Close a specific editor session
    const closeEditor = useCallback((sessionId?: string) => {
        if (sessionId) {
            // Close specific session
            setEditorSessions(prev => {
                const newSessions = prev.filter(s => s.id !== sessionId);
                // Adjust active index if needed
                setActiveSessionIndex(current => {
                    if (current >= newSessions.length) {
                        return Math.max(0, newSessions.length - 1);
                    }
                    return current;
                });
                return newSessions;
            });
        } else {
            // Close all sessions (for backward compatibility)
            setEditorSessions([]);
            setActiveSessionIndex(0);
        }
    }, []);

    // Navigate to previous session
    const previousSession = useCallback(() => {
        setActiveSessionIndex(prev => (prev > 0 ? prev - 1 : editorSessions.length - 1));
    }, [editorSessions.length]);

    // Navigate to next session
    const nextSession = useCallback(() => {
        setActiveSessionIndex(prev => (prev < editorSessions.length - 1 ? prev + 1 : 0));
    }, [editorSessions.length]);

    // Get the first editor session (for backward compatibility with single-editor mode)
    const editorSession = editorSessions.length > 0 ? editorSessions[activeSessionIndex] : null;

    return {
        editorSession,
        editorSessions,
        activeSessionIndex,
        launchVerbEditor,
        showVerbEditor,
        closeEditor,
        previousSession,
        nextSession,
    };
};
