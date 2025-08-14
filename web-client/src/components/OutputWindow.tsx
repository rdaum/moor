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

import React, { useCallback, useEffect, useRef, useState } from "react";
import { ContentRenderer } from "./ContentRenderer";

interface OutputWindowProps {
    messages: Array<{
        id: string;
        content: string | string[];
        type: "narrative" | "input_echo" | "system" | "error";
        timestamp?: number;
        isHistorical?: boolean;
        contentType?: "text/plain" | "text/djot" | "text/html";
    }>;
    onLoadMoreHistory?: () => void;
    isLoadingHistory?: boolean;
    onLinkClick?: (url: string) => void;
}

export const OutputWindow: React.FC<OutputWindowProps> = ({
    messages,
    onLoadMoreHistory,
    isLoadingHistory = false,
    onLinkClick,
}) => {
    const outputRef = useRef<HTMLDivElement>(null);
    const shouldAutoScroll = useRef(true);
    const previousScrollHeight = useRef<number>(0);
    const [isViewingHistory, setIsViewingHistory] = useState(false);

    // Auto-scroll to bottom when new messages arrive
    useEffect(() => {
        if (shouldAutoScroll.current && outputRef.current) {
            outputRef.current.scrollTop = outputRef.current.scrollHeight;
        }
    }, [messages]);

    // Handle container resize (e.g., when input area grows/shrinks)
    useEffect(() => {
        const outputElement = outputRef.current;
        if (!outputElement) return;

        const resizeObserver = new ResizeObserver(() => {
            // If user was at or near the bottom, keep them there after resize
            if (shouldAutoScroll.current) {
                outputElement.scrollTop = outputElement.scrollHeight;
            }
        });

        resizeObserver.observe(outputElement);

        return () => {
            resizeObserver.disconnect();
        };
    }, []);

    // Maintain scroll position when history is loaded (prepended to top)
    useEffect(() => {
        if (outputRef.current && previousScrollHeight.current > 0) {
            const currentScrollHeight = outputRef.current.scrollHeight;
            const heightDifference = currentScrollHeight - previousScrollHeight.current;

            if (heightDifference > 0) {
                // Adjust scroll position to maintain user's view
                outputRef.current.scrollTop += heightDifference;
            }
        }

        // Update previous scroll height
        if (outputRef.current) {
            previousScrollHeight.current = outputRef.current.scrollHeight;
        }
    }, [messages.length]);

    // Jump to the bottom (latest messages)
    const jumpToNow = useCallback(() => {
        if (outputRef.current) {
            outputRef.current.scrollTop = outputRef.current.scrollHeight;
            shouldAutoScroll.current = true;
            setIsViewingHistory(false);
        }
    }, []);

    // Handle scroll events to detect if user is viewing history
    const handleScroll = useCallback(() => {
        if (outputRef.current) {
            const { scrollTop, scrollHeight, clientHeight } = outputRef.current;

            const isNearBottom = (scrollTop + clientHeight) >= (scrollHeight - 100);
            shouldAutoScroll.current = isNearBottom;

            // Track if user is viewing history (not at bottom)
            setIsViewingHistory(!isNearBottom);

            // Check if user scrolled to the very top (within 50px)
            const isAtTop = scrollTop <= 50;

            if (isAtTop && onLoadMoreHistory && !isLoadingHistory) {
                onLoadMoreHistory();
            }
        }
    }, [onLoadMoreHistory, isLoadingHistory]);

    const getMessageClassName = (type: string, isHistorical?: boolean) => {
        let baseClass = "";
        switch (type) {
            case "input_echo":
                baseClass = "input_echo";
                break;
            case "system":
                baseClass = "system_message_narrative";
                break;
            case "error":
                baseClass = "error_message_narrative";
                break;
            case "narrative":
            default:
                baseClass = "text_narrative";
                break;
        }

        // Add historical vs live class
        if (isHistorical) {
            baseClass += " historical_narrative";
        } else {
            baseClass += " live_narrative";
        }

        return baseClass;
    };

    return (
        <div
            ref={outputRef}
            id="output_window"
            className="output_window"
            role="log"
            aria-live="polite"
            aria-atomic="false"
            aria-label="Narrative content"
            aria-relevant="additions"
            onScroll={handleScroll}
            style={{
                paddingBottom: "1rem",
            }}
        >
            {/* History indicator - "Jump to Now" button */}
            {isViewingHistory && (
                <div
                    className="history_indicator"
                    style={{
                        position: "sticky",
                        top: "10px",
                        width: "fit-content",
                        margin: "0 auto 10px auto",
                        background: "color-mix(in srgb, var(--color-bg-base) 90%, transparent)",
                        backdropFilter: "blur(8px)",
                        color: "var(--color-text-primary)",
                        padding: "8px 16px",
                        borderRadius: "20px",
                        border: "1px solid var(--color-border-medium)",
                        zIndex: 1000,
                        display: "flex",
                        alignItems: "center",
                        gap: "10px",
                        fontSize: "14px",
                        fontFamily: "var(--font-sans)",
                    }}
                >
                    <span>Viewing history</span>
                    <button
                        onClick={jumpToNow}
                        aria-label="Return to latest messages"
                        aria-describedby="history-status"
                        style={{
                            background: "var(--color-button-primary)",
                            color: "white",
                            border: "none",
                            padding: "4px 12px",
                            borderRadius: "12px",
                            cursor: "pointer",
                            fontSize: "12px",
                            fontFamily: "var(--font-sans)",
                            transition: "all 0.2s ease",
                        }}
                        onMouseOver={(e) =>
                            e.currentTarget.style.background =
                                "color-mix(in srgb, var(--color-button-primary) 80%, white)"}
                        onMouseOut={(e) => e.currentTarget.style.background = "var(--color-button-primary)"}
                    >
                        Jump to Now
                    </button>
                    <div id="history-status" className="sr-only">
                        Currently viewing message history
                    </div>
                </div>
            )}

            {/* Add minimal top padding to ensure scrollability */}
            {onLoadMoreHistory && (
                <div
                    style={{
                        height: "50px",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        color: "var(--color-text-secondary)",
                        fontSize: "0.8em",
                    }}
                >
                    {isLoadingHistory && (
                        <span role="status" aria-live="polite">
                            Loading more history...
                        </span>
                    )}
                </div>
            )}

            {/* Render all messages */}
            {messages.map((message) => (
                <div
                    key={message.id}
                    className={getMessageClassName(message.type, message.isHistorical)}
                >
                    <ContentRenderer
                        content={message.content}
                        contentType={message.contentType}
                        onLinkClick={onLinkClick}
                    />
                </div>
            ))}
        </div>
    );
};
