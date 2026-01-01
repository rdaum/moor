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

// Common window wrapper for all editor components (VerbEditor, PropertyEditor, ObjectBrowser, etc.)
// Handles dragging, resizing, split mode, focus management, and keyboard events

import React, { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";

interface EditorWindowContextValue {
    splitMode: boolean;
    isDragging: boolean;
    isResizing: boolean;
    startDrag: (clientX: number, clientY: number) => void;
}

const EditorWindowContext = createContext<EditorWindowContextValue | null>(null);

export const useEditorWindow = () => {
    const context = useContext(EditorWindowContext);
    if (!context) {
        throw new Error("useEditorWindow must be used within EditorWindow");
    }
    return context;
};

export interface EditorWindowProps {
    visible: boolean;
    onClose: () => void;
    splitMode?: boolean;
    children: React.ReactNode;
    // Window configuration
    defaultPosition?: { x: number; y: number };
    defaultSize?: { width: number; height: number };
    minSize?: { width: number; height: number };
    // ARIA
    ariaLabel?: string;
    className?: string;
}

const DEFAULT_POSITION = { x: 50, y: 50 };
const DEFAULT_SIZE = { width: 800, height: 600 };
const DEFAULT_MIN_SIZE = { width: 400, height: 300 };

export const EditorWindow: React.FC<EditorWindowProps> = ({
    visible,
    onClose,
    splitMode = false,
    children,
    defaultPosition = DEFAULT_POSITION,
    defaultSize = DEFAULT_SIZE,
    minSize = DEFAULT_MIN_SIZE,
    ariaLabel = "Editor window",
    className = "editor_container",
}) => {
    const containerRef = useRef<HTMLDivElement | null>(null);
    const [position, setPosition] = useState(defaultPosition);
    const [size, setSize] = useState(defaultSize);
    const [isDragging, setIsDragging] = useState(false);
    const [isResizing, setIsResizing] = useState(false);
    const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
    const [resizeStart, setResizeStart] = useState({ x: 0, y: 0, width: 0, height: 0 });

    // Function to start dragging (called by title bar via context)
    const startDrag = useCallback((clientX: number, clientY: number) => {
        if (splitMode) return;
        setIsDragging(true);
        setDragStart({
            x: clientX - position.x,
            y: clientY - position.y,
        });
    }, [splitMode, position]);

    // Mouse/touch event handlers for dragging and resizing
    const handleMouseMove = useCallback((e: MouseEvent) => {
        if (isDragging) {
            const newX = e.clientX - dragStart.x;
            const newY = e.clientY - dragStart.y;

            // Keep window within viewport bounds
            const maxX = window.innerWidth - size.width;
            const maxY = window.innerHeight - size.height;

            setPosition({
                x: Math.max(0, Math.min(maxX, newX)),
                y: Math.max(0, Math.min(maxY, newY)),
            });
        } else if (isResizing) {
            const deltaX = e.clientX - resizeStart.x;
            const deltaY = e.clientY - resizeStart.y;

            const newWidth = Math.max(minSize.width, resizeStart.width + deltaX);
            const newHeight = Math.max(minSize.height, resizeStart.height + deltaY);

            setSize({ width: newWidth, height: newHeight });
        }
    }, [isDragging, isResizing, dragStart, resizeStart, size.width, size.height, minSize]);

    const handleMouseUp = useCallback(() => {
        setIsDragging(false);
        setIsResizing(false);
    }, []);

    // Add global mouse event listeners when dragging/resizing
    useEffect(() => {
        if (isDragging || isResizing) {
            document.addEventListener("mousemove", handleMouseMove);
            document.addEventListener("mouseup", handleMouseUp);
            document.body.style.userSelect = "none";
            document.body.style.cursor = isDragging ? "grabbing" : "nwse-resize";

            return () => {
                document.removeEventListener("mousemove", handleMouseMove);
                document.removeEventListener("mouseup", handleMouseUp);
                document.body.style.userSelect = "";
                document.body.style.cursor = "";
            };
        }
    }, [isDragging, isResizing, handleMouseMove, handleMouseUp]);

    // Focus management and keyboard events for modal mode
    useEffect(() => {
        if (!visible || splitMode) return;

        const previouslyFocused = document.activeElement as HTMLElement;

        // Focus the container when it opens
        if (containerRef.current) {
            containerRef.current.focus();
        }

        // Handle keyboard events
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === "Escape") {
                onClose();
                return;
            }

            // Focus trapping for Tab key
            if (e.key === "Tab") {
                const focusableElements = containerRef.current?.querySelectorAll(
                    "button, [href], input, select, textarea, [tabindex]:not([tabindex=\"-1\"])",
                );

                if (!focusableElements || focusableElements.length === 0) return;

                const firstElement = focusableElements[0] as HTMLElement;
                const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

                if (e.shiftKey) {
                    // Shift+Tab: if focus is on first element, move to last
                    if (document.activeElement === firstElement) {
                        e.preventDefault();
                        lastElement.focus();
                    }
                } else {
                    // Tab: if focus is on last element, move to first
                    if (document.activeElement === lastElement) {
                        e.preventDefault();
                        firstElement.focus();
                    }
                }
            }
        };

        document.addEventListener("keydown", handleKeyDown);

        // Cleanup: restore focus when modal closes
        return () => {
            document.removeEventListener("keydown", handleKeyDown);
            if (previouslyFocused) {
                previouslyFocused.focus();
            }
        };
    }, [visible, splitMode, onClose]);

    if (!visible) {
        return null;
    }

    // Split mode styling - fills container
    const splitStyle: React.CSSProperties = {
        width: "100%",
        height: "100%",
        backgroundColor: "var(--color-bg-input)",
        border: "1px solid var(--color-border-medium)",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
    };

    // Modal mode styling - floating window
    const modalStyle: React.CSSProperties = {
        position: "fixed",
        top: `${position.y}px`,
        left: `${position.x}px`,
        width: `${size.width}px`,
        height: `${size.height}px`,
        backgroundColor: "var(--color-bg-input)",
        border: "1px solid var(--color-border-medium)",
        borderRadius: "var(--radius-lg)",
        boxShadow: "0 8px 32px var(--color-shadow)",
        zIndex: 1000,
        display: "flex",
        flexDirection: "column",
        cursor: isDragging ? "grabbing" : "default",
    };

    const contextValue: EditorWindowContextValue = {
        splitMode,
        isDragging,
        isResizing,
        startDrag,
    };

    return (
        <EditorWindowContext.Provider value={contextValue}>
            <div
                ref={containerRef}
                className={className}
                role={splitMode ? "region" : "dialog"}
                aria-modal={splitMode ? undefined : "true"}
                aria-label={ariaLabel}
                tabIndex={-1}
                style={splitMode ? splitStyle : modalStyle}
            >
                {children}

                {/* Resize handle - only in modal mode */}
                {!splitMode && (
                    <div
                        onMouseDown={(e) => {
                            if (e.button !== 0) return;
                            setIsResizing(true);
                            setResizeStart({
                                x: e.clientX,
                                y: e.clientY,
                                width: size.width,
                                height: size.height,
                            });
                            e.preventDefault();
                            e.stopPropagation();
                        }}
                        onTouchStart={(e) => {
                            if (e.touches.length === 1) {
                                const touch = e.touches[0];
                                setIsResizing(true);
                                setResizeStart({
                                    x: touch.clientX,
                                    y: touch.clientY,
                                    width: size.width,
                                    height: size.height,
                                });
                                e.preventDefault();
                            }
                        }}
                        tabIndex={0}
                        role="button"
                        aria-label="Resize editor window"
                        onKeyDown={(e) => {
                            if (e.key === "Enter" || e.key === " ") {
                                e.preventDefault();
                                setIsResizing(true);
                                setResizeStart({
                                    x: size.width + position.x,
                                    y: size.height + position.y,
                                    width: size.width,
                                    height: size.height,
                                });
                            }
                        }}
                        className="editor-resize-handle"
                    >
                        <div className="editor-resize-handle-inner" />
                        <div className="editor-resize-handle-triangle" />
                        <span aria-hidden="true" className="editor-resize-handle-symbol">
                            ↘
                        </span>
                    </div>
                )}
            </div>
        </EditorWindowContext.Provider>
    );
};

/**
 * Hook that provides dragging functionality for title bars
 * Returns props to spread on the title bar element
 *
 * Usage:
 * const titleBarProps = useTitleBarDrag();
 * <div {...titleBarProps}>Title</div>
 */
export const useTitleBarDrag = () => {
    const { splitMode, isDragging, startDrag } = useEditorWindow();

    const handleMouseDown = useCallback((e: React.MouseEvent) => {
        if (e.button !== 0) return;
        // Don't trigger drag if clicking on interactive elements (buttons, inputs, etc.)
        const target = e.target as HTMLElement;
        if (target.closest("button, input, select, textarea, a")) {
            return;
        }
        startDrag(e.clientX, e.clientY);
        e.preventDefault();
    }, [startDrag]);

    const handleTouchStart = useCallback((e: React.TouchEvent) => {
        if (e.touches.length !== 1) return;
        const target = e.target as HTMLElement;
        if (target.closest("button, input, select, textarea, a")) {
            return;
        }
        const touch = e.touches[0];
        startDrag(touch.clientX, touch.clientY);
        e.preventDefault();
    }, [startDrag]);

    return {
        onMouseDown: handleMouseDown,
        onTouchStart: handleTouchStart,
        style: {
            cursor: splitMode ? "default" : (isDragging ? "grabbing" : "grab"),
        },
    };
};
