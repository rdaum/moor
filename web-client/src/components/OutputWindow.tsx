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
import { uuObjIdToString } from "../lib/var";
import { ContentRenderer } from "./ContentRenderer";
import { LinkPreview, LinkPreviewCard } from "./LinkPreviewCard";
import { getSpeechBubblesEnabled } from "./SpeechBubbleToggle";

interface EventMetadata {
    verb?: string;
    actor?: any;
    actorName?: string;
    content?: string;
    thisObj?: any;
    thisName?: string;
    dobj?: any;
    iobj?: any;
    timestamp?: number;
}

interface OutputWindowProps {
    messages: Array<{
        id: string;
        content: string | string[];
        type: "narrative" | "input_echo" | "system" | "error";
        timestamp?: number;
        isHistorical?: boolean;
        contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback";
        noNewline?: boolean;
        presentationHint?: string;
        groupId?: string;
        ttsText?: string;
        thumbnail?: { contentType: string; data: string };
        linkPreview?: LinkPreview;
        eventMetadata?: EventMetadata;
    }>;
    onLoadMoreHistory?: () => void;
    isLoadingHistory?: boolean;
    onLinkClick?: (url: string, position?: { x: number; y: number }) => void;
    onLinkHoldStart?: (url: string, position: { x: number; y: number }) => void;
    onLinkHoldEnd?: () => void;
    fontSize?: number;
    shouldShowDisconnectDivider?: boolean;
    playerOid?: string | null;
}

// Compare actor from event metadata with playerOid
const isActorPlayer = (actor: any, playerOid: string | null | undefined): boolean => {
    if (!actor || !playerOid) return false;

    // Normalize playerOid - strip # prefix and oid:/uuid: prefix
    let normalizedPlayerOid = playerOid;
    if (normalizedPlayerOid.startsWith("#")) {
        normalizedPlayerOid = normalizedPlayerOid.substring(1);
    }
    if (normalizedPlayerOid.startsWith("oid:")) {
        normalizedPlayerOid = normalizedPlayerOid.substring(4);
    } else if (normalizedPlayerOid.startsWith("uuid:")) {
        normalizedPlayerOid = normalizedPlayerOid.substring(5);
    }

    // actor has { oid: number } or { uuid: string (packed bigint) }
    if (actor.oid !== undefined) {
        return normalizedPlayerOid === String(actor.oid);
    }
    if (actor.uuid !== undefined) {
        // Convert packed bigint string to formatted UUID for comparison
        const formattedUuid = uuObjIdToString(BigInt(actor.uuid));
        return normalizedPlayerOid === formattedUuid;
    }
    return false;
};

export const OutputWindow: React.FC<OutputWindowProps> = ({
    messages,
    onLoadMoreHistory,
    isLoadingHistory = false,
    onLinkClick,
    onLinkHoldStart,
    onLinkHoldEnd,
    fontSize,
    shouldShowDisconnectDivider = false,
    playerOid,
}) => {
    const outputRef = useRef<HTMLDivElement>(null);
    const shouldAutoScroll = useRef(true);
    const previousScrollHeight = useRef<number>(0);
    const [isViewingHistory, setIsViewingHistory] = useState(false);
    const [speechBubblesEnabled, setSpeechBubblesEnabled] = useState(getSpeechBubblesEnabled);

    // Listen for speech bubble setting changes
    useEffect(() => {
        const handleChange = (e: Event) => {
            const customEvent = e as CustomEvent<boolean>;
            setSpeechBubblesEnabled(customEvent.detail);
        };
        const handleStorage = (e: StorageEvent) => {
            if (e.key === "speechBubblesEnabled") {
                setSpeechBubblesEnabled(getSpeechBubblesEnabled());
            }
        };
        window.addEventListener("speechBubblesChanged", handleChange);
        window.addEventListener("storage", handleStorage);
        return () => {
            window.removeEventListener("speechBubblesChanged", handleChange);
            window.removeEventListener("storage", handleStorage);
        };
    }, []);

    // Render content with optional TTS text for screen readers
    const renderContentWithTts = useCallback((
        content: string | string[],
        contentType: "text/plain" | "text/djot" | "text/html" | "text/traceback" | undefined,
        ttsText: string | undefined,
        thumbnail?: { contentType: string; data: string },
        linkPreview?: LinkPreview,
    ) => {
        if (ttsText) {
            return (
                <>
                    {thumbnail && <img src={thumbnail.data} alt="thumbnail" className="narrative_thumbnail" />}
                    <span className="sr-only">{ttsText}</span>
                    <span aria-hidden="true">
                        <ContentRenderer
                            content={content}
                            contentType={contentType}
                            onLinkClick={onLinkClick}
                            onLinkHoldStart={onLinkHoldStart}
                            onLinkHoldEnd={onLinkHoldEnd}
                        />
                    </span>
                    {linkPreview && <LinkPreviewCard preview={linkPreview} />}
                </>
            );
        }
        return (
            <>
                {thumbnail && <img src={thumbnail.data} alt="thumbnail" className="narrative_thumbnail" />}
                <ContentRenderer
                    content={content}
                    contentType={contentType}
                    onLinkClick={onLinkClick}
                    onLinkHoldStart={onLinkHoldStart}
                    onLinkHoldEnd={onLinkHoldEnd}
                />
                {linkPreview && <LinkPreviewCard preview={linkPreview} />}
            </>
        );
    }, [onLinkClick, onLinkHoldStart, onLinkHoldEnd]);

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

    // Render a speech bubble for say events
    const renderSpeechBubble = (
        actorName: string,
        content: string,
        messageId: string,
        isHistorical?: boolean,
        fromMe?: boolean,
        contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback",
    ) => {
        const displayName = fromMe ? "You" : actorName;
        const bubbleClass = fromMe ? "speech_bubble speech_bubble_me" : "speech_bubble";
        const ariaLabel = fromMe ? `You say: ${content}` : `${actorName} says: ${content}`;
        return (
            <div
                key={messageId}
                className={`speech_bubble_container${fromMe ? " speech_bubble_container_me" : ""}${
                    isHistorical ? " historical_narrative" : " live_narrative"
                }`}
                role="article"
                aria-label={ariaLabel}
            >
                <span className="speech_bubble_actor" aria-hidden="true">{displayName}</span>
                <div className={bubbleClass} aria-hidden="true">
                    <ContentRenderer
                        content={content}
                        contentType={contentType}
                        onLinkClick={onLinkClick}
                        onLinkHoldStart={onLinkHoldStart}
                        onLinkHoldEnd={onLinkHoldEnd}
                    />
                </div>
            </div>
        );
    };

    // Check if current theme is a CRT theme (no speech bubbles in retro modes)
    const isCrtTheme = () => {
        return document.body.classList.contains("crt-theme")
            || document.body.classList.contains("crt-amber-theme");
    };

    // Check if a message should be rendered as a speech bubble
    const isSpeechBubble = (
        presentationHint?: string,
        eventMetadata?: EventMetadata,
    ): eventMetadata is EventMetadata & { content: string; actorName: string } => {
        return speechBubblesEnabled
            && !isCrtTheme()
            && presentationHint === "speech_bubble"
            && typeof eventMetadata?.content === "string"
            && typeof eventMetadata?.actorName === "string";
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

            {/* Render all messages, grouping no_newline messages and consecutive messages with same presentationHint+groupId */}
            {(() => {
                const groupedMessages: typeof messages[] = [];
                let currentGroup: typeof messages = [];

                for (let i = 0; i < messages.length; i++) {
                    const message = messages[i];
                    currentGroup.push(message);

                    const nextMessage = i < messages.length - 1 ? messages[i + 1] : null;

                    // Continue grouping if:
                    // 1. This message has noNewline, OR
                    // 2. This message has a presentationHint and the next message has the same hint AND same groupId
                    //    AND same actor (for speech bubbles, we don't want to merge different speakers)
                    const sameActor = () => {
                        const a1 = message.eventMetadata?.actor;
                        const a2 = nextMessage?.eventMetadata?.actor;
                        if (!a1 || !a2) return true; // If no actor info, allow grouping
                        if (a1.oid !== undefined && a2.oid !== undefined) return a1.oid === a2.oid;
                        if (a1.uuid !== undefined && a2.uuid !== undefined) return a1.uuid === a2.uuid;
                        return false; // Different actor representations = different actors
                    };
                    const sameHintGroup = message.presentationHint
                        && nextMessage?.presentationHint === message.presentationHint
                        && message.groupId === nextMessage?.groupId
                        && sameActor();
                    const shouldContinueGroup = message.noNewline || sameHintGroup;

                    // If we shouldn't continue grouping or it's the last message, complete the current group
                    if (!shouldContinueGroup || i === messages.length - 1) {
                        groupedMessages.push(currentGroup);
                        currentGroup = [];
                    }
                }

                return groupedMessages.map((group, groupIndex) => {
                    const firstMessage = group[0];
                    const isDividerGroup = shouldShowDisconnectDivider && !firstMessage.isHistorical && groupIndex > 0;
                    const previousGroup = groupIndex > 0 ? groupedMessages[groupIndex - 1] : null;
                    const shouldShowDivider = isDividerGroup && previousGroup
                        && previousGroup[previousGroup.length - 1].isHistorical;

                    const result = [];

                    // Add divider if this is the first non-historical message and we should show it
                    if (shouldShowDivider) {
                        result.push(
                            <div
                                key={`divider_${groupIndex}`}
                                className="history_separator"
                                role="separator"
                                aria-label="Reconnection point: messages before this occurred during a disconnection lasting more than 10 minutes"
                            >
                                <span aria-hidden="true">●●●</span>
                            </div>,
                        );
                    }

                    if (group.length === 1) {
                        // Single message
                        const message = group[0];

                        // Check if this should render as a speech bubble
                        if (isSpeechBubble(message.presentationHint, message.eventMetadata)) {
                            const fromMe = isActorPlayer(message.eventMetadata.actor, playerOid);
                            result.push(
                                renderSpeechBubble(
                                    message.eventMetadata.actorName,
                                    message.eventMetadata.content,
                                    message.id,
                                    message.isHistorical,
                                    fromMe,
                                    message.contentType,
                                ),
                            );
                        } else if (message.presentationHint) {
                            // If it has a presentationHint, wrap it like we do for groups
                            const baseClassName = getMessageClassName(message.type, message.isHistorical);
                            const wrapperClassName = message.presentationHint === "inset" ? "presentation_inset" : "";

                            result.push(
                                <div key={message.id} className={wrapperClassName}>
                                    <div className={baseClassName}>
                                        {renderContentWithTts(
                                            message.content,
                                            message.contentType,
                                            message.ttsText,
                                            message.thumbnail,
                                            message.linkPreview,
                                        )}
                                    </div>
                                </div>,
                            );
                        } else {
                            // Regular message without presentationHint
                            result.push(
                                <div
                                    key={message.id}
                                    className={getMessageClassName(
                                        message.type,
                                        message.isHistorical,
                                    )}
                                >
                                    {renderContentWithTts(
                                        message.content,
                                        message.contentType,
                                        message.ttsText,
                                        message.thumbnail,
                                        message.linkPreview,
                                    )}
                                </div>,
                            );
                        }

                        return result;
                    } else {
                        // Multiple messages grouped together

                        // Check if this group is for presentationHint or noNewline
                        const isHintGroup = firstMessage.presentationHint
                            && group.every(msg =>
                                msg.presentationHint === firstMessage.presentationHint
                                && msg.groupId === firstMessage.groupId
                            );

                        if (isHintGroup) {
                            // Check if this is a speech bubble group
                            if (isSpeechBubble(firstMessage.presentationHint, firstMessage.eventMetadata)) {
                                // Get actor name from first message (all in group are same speaker)
                                const actorName = firstMessage.eventMetadata.actorName;
                                const fromMe = isActorPlayer(firstMessage.eventMetadata.actor, playerOid);

                                // Combine all messages into one bubble with newlines
                                const combinedContent = group.map(msg =>
                                    msg.eventMetadata?.content
                                    || (typeof msg.content === "string" ? msg.content : "")
                                ).join("\n");

                                result.push(
                                    renderSpeechBubble(
                                        actorName,
                                        combinedContent,
                                        `speech_group_${firstMessage.id}`,
                                        firstMessage.isHistorical,
                                        fromMe,
                                        firstMessage.contentType,
                                    ),
                                );
                            } else {
                                // Regular hint group - render each on its own line
                                const baseClassName = getMessageClassName(
                                    firstMessage.type,
                                    firstMessage.isHistorical,
                                );
                                const wrapperClassName = firstMessage.presentationHint === "inset"
                                    ? "presentation_inset"
                                    : "";

                                // Use firstMessage.id for stable key - prevents re-render when group grows
                                // The stable key ensures React preserves the DOM node and only appends
                                // new children, so screen readers announce only additions (not the whole group)
                                result.push(
                                    <div
                                        key={`hint_${firstMessage.id}`}
                                        className={wrapperClassName}
                                        role="group"
                                        aria-label="Grouped content"
                                    >
                                        {group.map(msg => (
                                            <div key={msg.id} className={baseClassName}>
                                                {renderContentWithTts(
                                                    msg.content,
                                                    msg.contentType,
                                                    msg.ttsText,
                                                    msg.thumbnail,
                                                    msg.linkPreview,
                                                )}
                                            </div>
                                        ))}
                                    </div>,
                                );
                            }
                        } else {
                            // noNewline group - combine content on same line
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

                            // Combine ttsText from all messages in the group
                            const combinedTtsText = group
                                .filter(msg => msg.ttsText)
                                .map(msg => msg.ttsText)
                                .join(" ");

                            // Get linkPreview from the last message in the group (if any)
                            const lastLinkPreview = group.find(msg => msg.linkPreview)?.linkPreview;

                            result.push(
                                <div
                                    key={`noline_${firstMessage.id}`}
                                    className={getMessageClassName(
                                        firstMessage.type,
                                        firstMessage.isHistorical,
                                    )}
                                >
                                    {renderContentWithTts(
                                        combinedHtml,
                                        "text/html",
                                        combinedTtsText || undefined,
                                        undefined,
                                        lastLinkPreview,
                                    )}
                                </div>,
                            );
                        }

                        return result;
                    }
                });
            })()}
        </div>
    );
};
