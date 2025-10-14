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

import * as flatbuffers from "flatbuffers";
import { useCallback, useState } from "react";
import { decryptEventBlob } from "../lib/age-decrypt";
import { getCurrentPresentationsFlatBuffer } from "../lib/rpc-fb";
import { Presentation, PresentationData, SemanticTarget, TARGET_TYPES } from "../types/presentation";
import { useMediaQuery } from "./useMediaQuery";

// Responsive mapping of semantic targets to visual placement
const useSemanticMapping = () => {
    const isMobile = useMediaQuery("(max-width: 768px)");

    const getPlacementForTarget = useCallback((target: SemanticTarget): "left" | "right" | "top" | "bottom" => {
        if (isMobile) {
            // On mobile, map most things to bottom for better UX
            switch (target) {
                case "navigation":
                case "communication":
                    return "top";
                case "status":
                    return "top";
                case "inventory":
                case "tools":
                default:
                    return "bottom";
            }
        } else {
            // Desktop placement
            switch (target) {
                case "navigation":
                case "communication":
                    return "left";
                case "inventory":
                case "status":
                case "tools":
                    return "right";
                default:
                    return "right";
            }
        }
    }, [isMobile]);

    return { getPlacementForTarget };
};

export const usePresentations = () => {
    const [presentations, setPresentations] = useState<Map<string, Presentation>>(new Map());
    const { getPlacementForTarget } = useSemanticMapping();

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

    // Get presentations for visual placement (mapped from semantic targets)
    const getLeftDockPresentations = useCallback((): Presentation[] => {
        return Array.from(presentations.values()).filter(p => {
            if (p.target === TARGET_TYPES.WINDOW || p.target === TARGET_TYPES.VERB_EDITOR) return false;
            const semanticTarget = p.target as SemanticTarget;
            return getPlacementForTarget(semanticTarget) === "left";
        });
    }, [presentations, getPlacementForTarget]);

    const getRightDockPresentations = useCallback((): Presentation[] => {
        return Array.from(presentations.values()).filter(p => {
            if (p.target === TARGET_TYPES.WINDOW || p.target === TARGET_TYPES.VERB_EDITOR) return false;
            const semanticTarget = p.target as SemanticTarget;
            return getPlacementForTarget(semanticTarget) === "right";
        });
    }, [presentations, getPlacementForTarget]);

    const getTopDockPresentations = useCallback((): Presentation[] => {
        return Array.from(presentations.values()).filter(p => {
            if (p.target === TARGET_TYPES.WINDOW || p.target === TARGET_TYPES.VERB_EDITOR) return false;
            const semanticTarget = p.target as SemanticTarget;
            return getPlacementForTarget(semanticTarget) === "top";
        });
    }, [presentations, getPlacementForTarget]);

    const getBottomDockPresentations = useCallback((): Presentation[] => {
        return Array.from(presentations.values()).filter(p => {
            if (p.target === TARGET_TYPES.WINDOW || p.target === TARGET_TYPES.VERB_EDITOR) return false;
            const semanticTarget = p.target as SemanticTarget;
            return getPlacementForTarget(semanticTarget) === "bottom";
        });
    }, [presentations, getPlacementForTarget]);

    const getWindowPresentations = useCallback(() => getPresentationsByTarget(TARGET_TYPES.WINDOW), [
        getPresentationsByTarget,
    ]);

    const getHelpPresentations = useCallback(() => getPresentationsByTarget(TARGET_TYPES.HELP), [
        getPresentationsByTarget,
    ]);

    const getVerbEditorPresentations = useCallback(() => getPresentationsByTarget(TARGET_TYPES.VERB_EDITOR), [
        getPresentationsByTarget,
    ]);

    // Clear all presentations (used on logout)
    const clearAll = useCallback(() => {
        setPresentations(new Map());
    }, []);

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
    const fetchCurrentPresentations = useCallback(async (authToken: string, ageIdentity: string | null = null) => {
        try {
            // Get presentations as FlatBuffer
            const currentPresentations = await getCurrentPresentationsFlatBuffer(authToken);

            const presentationsLength = currentPresentations.presentationsLength();

            // Process each presentation
            for (let i = 0; i < presentationsLength; i++) {
                const presentationFB = currentPresentations.presentations(i);
                if (!presentationFB) continue;

                try {
                    // Decrypt the presentation content if we have an encryption key
                    let content = "";

                    if (ageIdentity) {
                        // Content is encrypted - get as bytes and decrypt
                        const contentBytes = presentationFB.content(flatbuffers.Encoding.UTF8_BYTES);
                        if (contentBytes && contentBytes instanceof Uint8Array && contentBytes.length > 0) {
                            try {
                                const decryptedBytes = await decryptEventBlob(contentBytes, ageIdentity);
                                // Convert decrypted bytes to string
                                content = new TextDecoder().decode(decryptedBytes);
                            } catch (decryptError) {
                                console.error("Failed to decrypt presentation content:", decryptError);
                                // Leave content empty if decryption fails
                            }
                        }
                    } else {
                        // No encryption - get as string
                        content = presentationFB.content() || "";
                    }

                    // Convert FlatBuffer Presentation to PresentationData format
                    const attributes: Array<[string, string]> = [];
                    const attrsLength = presentationFB.attributesLength();
                    for (let j = 0; j < attrsLength; j++) {
                        const attr = presentationFB.attributes(j);
                        if (attr && attr.key() && attr.value()) {
                            attributes.push([attr.key()!, attr.value()!]);
                        }
                    }

                    const presentationData: PresentationData = {
                        id: presentationFB.id() || `unknown-${i}`,
                        target: presentationFB.target() || TARGET_TYPES.WINDOW,
                        content,
                        content_type: presentationFB.contentType() || "text/plain",
                        attributes,
                    };

                    addPresentation(presentationData);
                } catch (presentationError) {
                    console.error("Failed to process presentation:", presentationError);
                    continue;
                }
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
        getHelpPresentations,
        getVerbEditorPresentations,
        dismissPresentation,
        fetchCurrentPresentations,
        clearAll,
    };
};
