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

import { useCallback, useRef } from "react";

interface MCPSpool {
    title: string;
    objectCurie: string;
    verbName: string;
    uploadAction: string;
    lines: string[];
}

export const useMCPHandler = (
    onShowVerbEditor: (
        title: string,
        objectCurie: string,
        verbName: string,
        content: string,
        uploadAction?: string,
    ) => void,
) => {
    const spoolRef = useRef<MCPSpool | null>(null);

    // Parse MCP edit command: "#$# edit name: Object:verb upload: @program #object:verb permissions"
    const parseEditCommand = useCallback((command: string) => {
        // Remove the MCP prefix
        const cleanCommand = command.replace(/^#\$#\s+/, "");

        // Extract the name part (format: "edit name: Object:verb")
        let nameMatch = cleanCommand.match(/edit\s+name:\s*([^:]+):(@?\w+)/);
        let isProp = false;

        // If it's not a verb, try a property (format: "edit name: first room.description upload: @set-note-string #70.description")
        if (!nameMatch) {
            nameMatch = cleanCommand.match(/edit\s+name:\s*([^.]+).(@?\w+)/);
            isProp = nameMatch ? true : false;
        }

        if (!nameMatch) {
            console.warn("Invalid MCP edit command format:", command);
            return null;
        }

        // Extract the upload action (format: "upload: @program #object:verb permissions" or "upload: @set-note-string #70.description")
        const uploadMatch = cleanCommand.match(/upload:\s*(.+)$/);
        if (!uploadMatch) {
            console.warn("Invalid MCP edit command - no upload action found:", command);
            return null;
        }

        const objectName = nameMatch[1].trim();
        const verbName = nameMatch[2].trim();
        const uploadAction = uploadMatch[1].trim();

        const title = isProp ? `${objectName}.${verbName}` : `${objectName}:${verbName}`;

        return { objectCurie: objectName, verbName, title, uploadAction };
    }, []);

    // Handle narrative messages that might be MCP commands or spool content
    const handleNarrativeMessage = useCallback((content: string, isHistorical: boolean = false) => {
        // Handle null/undefined content
        if (!content || typeof content !== "string") {
            return false; // Let it pass through if content is invalid
        }

        // Always filter out any MCP messages (anything starting with "#$#")
        if (content.startsWith("#$#")) {
            // For historical content, just filter it out without processing
            if (isHistorical) {
                return true; // Indicate this message was handled (filtered)
            }

            // For live content, process edit commands
            const editInfo = parseEditCommand(content);
            if (editInfo) {
                // Start spooling for this edit command
                spoolRef.current = {
                    title: editInfo.title,
                    objectCurie: editInfo.objectCurie,
                    verbName: editInfo.verbName,
                    uploadAction: editInfo.uploadAction,
                    lines: [],
                };
            }
            return true; // Indicate this message was handled as MCP
        }

        // Only process spooling for live content, not historical
        if (!isHistorical && spoolRef.current) {
            // Check for end marker (single "." on its own line)
            if (content.trim() === ".") {
                // End of spool - launch editor
                const spool = spoolRef.current;
                const editorContent = spool.lines.join("\n");

                onShowVerbEditor(spool.title, spool.objectCurie, spool.verbName, editorContent, spool.uploadAction);

                // Clear spool
                spoolRef.current = null;
                return true; // Indicate this message was handled as MCP
            } else {
                // Add line to spool
                spoolRef.current.lines.push(content);
                return true; // Indicate this message was handled as MCP
            }
        }

        return false; // This message was not MCP-related
    }, [parseEditCommand, onShowVerbEditor]);

    return {
        handleNarrativeMessage,
    };
};
