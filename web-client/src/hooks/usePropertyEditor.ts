// Hook for managing property editor state and operations
// Handles launching and managing property editing sessions

import { useCallback, useState } from "react";

export interface PropertyEditorSession {
    id: string;
    title: string;
    objectCurie: string;
    propertyName: string;
    content: string;
    uploadAction?: string; // For MCP-triggered editors
    contentType?: "text/plain" | "text/html" | "text/markdown"; // Future support for different content types
}

export const usePropertyEditor = () => {
    const [propertyEditorSession, setPropertyEditorSession] = useState<PropertyEditorSession | null>(null);

    // Launch property editor with specific content (fetched from server)
    const launchPropertyEditor = useCallback(async (
        title: string,
        objectCurie: string,
        propertyName: string,
        authToken: string,
    ) => {
        try {
            // Fetch property content from server
            const response = await fetch(
                `/properties/${encodeURIComponent(objectCurie)}/${encodeURIComponent(propertyName)}`,
                {
                    method: "GET",
                    headers: {
                        "X-Moor-Auth-Token": authToken,
                    },
                },
            );

            if (!response.ok) {
                throw new Error(`Failed to fetch property: ${response.status} ${response.statusText}`);
            }

            const content = await response.text();

            // Create editor session
            setPropertyEditorSession({
                id: `${objectCurie}.${propertyName}`,
                title,
                objectCurie,
                propertyName,
                content,
                contentType: "text/plain", // Default to plain text, could be detected from property metadata
            });
        } catch (error) {
            console.error("Error launching property editor:", error);
        }
    }, []);

    // Show property editor with provided content (for MCP workflow)
    const showPropertyEditor = useCallback((
        title: string,
        objectCurie: string,
        propertyName: string,
        content: string,
        uploadAction?: string,
        contentType: "text/plain" | "text/html" | "text/markdown" = "text/plain",
    ) => {
        setPropertyEditorSession({
            id: `${objectCurie}.${propertyName}`,
            title,
            objectCurie,
            propertyName,
            content,
            uploadAction,
            contentType,
        });
    }, []);

    // Close the property editor
    const closePropertyEditor = useCallback(() => {
        setPropertyEditorSession(null);
    }, []);

    return {
        propertyEditorSession,
        launchPropertyEditor,
        showPropertyEditor,
        closePropertyEditor,
    };
};
