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
import { Presentation, PresentationData, TARGET_TYPES } from "../types/presentation";

export const usePresentations = () => {
    const [presentations, setPresentations] = useState<Map<string, Presentation>>(new Map());

    // Add a new presentation
    const addPresentation = useCallback((data: PresentationData) => {
        // Convert attributes array to object
        const attrs: { [key: string]: string } = {};
        for (const [key, value] of data.attributes) {
            attrs[key] = value;
        }

        // Normalize content type
        let contentType: "text/plain" | "text/djot" | "text/html" = "text/plain";
        if (data.content_type === "text/djot" || data.content_type === "text/html") {
            contentType = data.content_type;
        }

        const presentation: Presentation = {
            id: data.id,
            target: data.target,
            title: attrs.title || attrs.name || `Panel ${data.id}`,
            content: data.content,
            contentType,
            attrs,
        };

        setPresentations(prev => {
            const next = new Map(prev);
            next.set(data.id, presentation);
            return next;
        });
    }, []);

    // Remove a presentation
    const removePresentation = useCallback((id: string) => {
        setPresentations(prev => {
            const next = new Map(prev);
            next.delete(id);
            return next;
        });
    }, []);

    // Get presentations for a specific target
    const getPresentationsByTarget = useCallback((target: string): Presentation[] => {
        return Array.from(presentations.values()).filter(p => p.target === target);
    }, [presentations]);

    // Get presentations for specific dock
    const getLeftDockPresentations = useCallback(() => getPresentationsByTarget(TARGET_TYPES.LEFT_DOCK), [
        getPresentationsByTarget,
    ]);

    const getRightDockPresentations = useCallback(() => getPresentationsByTarget(TARGET_TYPES.RIGHT_DOCK), [
        getPresentationsByTarget,
    ]);

    const getTopDockPresentations = useCallback(() => getPresentationsByTarget(TARGET_TYPES.TOP_DOCK), [
        getPresentationsByTarget,
    ]);

    const getBottomDockPresentations = useCallback(() => getPresentationsByTarget(TARGET_TYPES.BOTTOM_DOCK), [
        getPresentationsByTarget,
    ]);

    const getWindowPresentations = useCallback(() => getPresentationsByTarget(TARGET_TYPES.WINDOW), [
        getPresentationsByTarget,
    ]);

    // API call to dismiss a presentation on the server
    const dismissPresentation = useCallback(async (id: string, authToken: string) => {
        try {
            const response = await fetch(`/api/presentations/${encodeURIComponent(id)}`, {
                method: "DELETE",
                headers: {
                    "X-Moor-Auth-Token": authToken,
                },
            });

            if (!response.ok) {
                console.error(`Failed to dismiss presentation ${id}: ${response.status} ${response.statusText}`);
                return;
            }

            // Remove locally (server will also send unpresent message)
            removePresentation(id);
        } catch (error) {
            console.error(`Error dismissing presentation ${id}:`, error);
        }
    }, [removePresentation]);

    // Fetch current presentations from the server (called on connect)
    const fetchCurrentPresentations = useCallback(async (authToken: string) => {
        try {
            const response = await fetch("/api/presentations", {
                headers: {
                    "X-Moor-Auth-Token": authToken,
                },
            });

            if (!response.ok) {
                throw new Error(`Failed to fetch presentations: ${response.status} ${response.statusText}`);
            }

            const data = await response.json();
            const presentationsData = data.presentations || [];

            // Add each presentation to the state
            for (const presentationData of presentationsData) {
                addPresentation(presentationData);
            }

            return true;
        } catch (error) {
            console.error("Error fetching current presentations:", error);
            return false;
        }
    }, [addPresentation]);

    return {
        presentations: Array.from(presentations.values()),
        addPresentation,
        removePresentation,
        getPresentationsByTarget,
        getLeftDockPresentations,
        getRightDockPresentations,
        getTopDockPresentations,
        getBottomDockPresentations,
        getWindowPresentations,
        dismissPresentation,
        fetchCurrentPresentations,
    };
};
