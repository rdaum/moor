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
        contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback";
        noNewline?: boolean;
    }>;
    onLoadMoreHistory?: () => void;
    isLoadingHistory?: boolean;
    onLinkClick?: (url: string) => void;
    fontSize?: number;
}

export const OutputWindow: React.FC<OutputWindowProps> = ({
    messages,
    onLoadMoreHistory,
    isLoadingHistory = false,
    onLinkClick,
    fontSize,
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

    const resolvedFontSize = fontSize ?? 14;

    return (
        <div
            ref={outputRef}
            id="output_window"
            className="output_window"
            role="log"
            aria-live="polite"
            aria-atomic="false"
            aria-relevant="additions"
            onScroll={handleScroll}
            style={{
                paddingBottom: "1rem",
                fontSize: `${resolvedFontSize}px`,
            }}
        >
            {/* History indicator - "Jump to Now" button */}
            {isViewingHistory && (
                <div className="history_indicator">
                    <span>Viewing history</span>
                    <button
                        onClick={jumpToNow}
                        aria-label="Return to latest messages"
                        aria-describedby="history-status"
                        className="history_indicator_button"
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
                <div className="output_window_load_more">
                    {isLoadingHistory && (
                        <span role="status" aria-live="polite">
                            Loading more history...
                        </span>
                    )}
                </div>
            )}

            {/* Render all messages, grouping no_newline messages */}
            {(() => {
                const groupedMessages = [];
                let currentGroup = [];

                for (let i = 0; i < messages.length; i++) {
                    const message = messages[i];
                    currentGroup.push(message);

                    // If this message doesn't suppress newlines or it's the last message,
                    // complete the current group
                    if (!message.noNewline || i === messages.length - 1) {
                        groupedMessages.push(currentGroup);
                        currentGroup = [];
                    }
                }

                return groupedMessages.map((group, groupIndex) => {
                    if (group.length === 1) {
                        // Single message - render normally without any grouping changes
                        const message = group[0];
                        return (
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
                        );
                    } else {
                        // Multiple messages grouped together - combine content preserving each message's format
                        const lastMessage = group[group.length - 1];

                        // Convert all messages to their rendered form and combine as HTML
                        const combinedHtml = group.map(msg => {
                            const content = typeof msg.content === "string"
                                ? msg.content
                                : Array.isArray(msg.content)
                                ? msg.content.join("")
                                : "";

                            // If it's HTML, use as-is; if it's plain text, escape it
                            if (msg.contentType === "text/html") {
                                return content;
                            } else {
                                // Escape HTML characters for non-HTML content
                                return content.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
                            }
                        }).join("");

                        return (
                            <div
                                key={`group_${groupIndex}_${lastMessage.id}`}
                                className={getMessageClassName(lastMessage.type, lastMessage.isHistorical)}
                            >
                                <ContentRenderer
                                    content={combinedHtml}
                                    contentType="text/html"
                                    onLinkClick={onLinkClick}
                                />
                            </div>
                        );
                    }
                });
            })()}
        </div>
    );
};
