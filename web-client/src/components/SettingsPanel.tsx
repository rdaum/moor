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

// ! Settings panel with theme toggle and other options

import React, { useEffect, useRef, useState } from "react";
import { CommandEchoToggle } from "./CommandEchoToggle";
import { EmojiToggle } from "./EmojiToggle";
import { FontSizeControl } from "./FontSizeControl";
import { FontToggle } from "./FontToggle";
import { SayModeToggle } from "./SayModeToggle";
import { ThemeToggle } from "./ThemeToggle";
import { VerbPaletteToggle } from "./VerbPaletteToggle";

interface SettingsPanelProps {
    isOpen: boolean;
    onClose: () => void;
    narrativeFontSize: number;
    onDecreaseNarrativeFontSize: () => void;
    onIncreaseNarrativeFontSize: () => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({
    isOpen,
    onClose,
    narrativeFontSize,
    onDecreaseNarrativeFontSize,
    onIncreaseNarrativeFontSize,
}) => {
    const closeButtonRef = useRef<HTMLButtonElement>(null);
    const panelRef = useRef<HTMLDivElement>(null);
    const previousActiveElementRef = useRef<HTMLElement | null>(null);
    const [copyAnnouncement, setCopyAnnouncement] = useState("");

    // Store the previously focused element and focus the close button when opened
    useEffect(() => {
        if (isOpen) {
            previousActiveElementRef.current = document.activeElement as HTMLElement;
            // Small delay to ensure the panel is rendered
            requestAnimationFrame(() => {
                closeButtonRef.current?.focus();
            });
        }
    }, [isOpen]);

    // Return focus to the previous element when closed
    useEffect(() => {
        if (!isOpen && previousActiveElementRef.current) {
            previousActiveElementRef.current.focus();
            previousActiveElementRef.current = null;
        }
    }, [isOpen]);

    // Handle Escape key to close
    useEffect(() => {
        if (!isOpen) return;

        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === "Escape") {
                e.preventDefault();
                e.stopPropagation();
                onClose();
            }
        };

        document.addEventListener("keydown", handleKeyDown);
        return () => document.removeEventListener("keydown", handleKeyDown);
    }, [isOpen, onClose]);

    // Trap focus within the dialog
    useEffect(() => {
        if (!isOpen) return;

        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key !== "Tab" || !panelRef.current) return;

            const focusableElements = panelRef.current.querySelectorAll<HTMLElement>(
                "button, [href], input, select, textarea, [tabindex]:not([tabindex=\"-1\"])",
            );
            const firstElement = focusableElements[0];
            const lastElement = focusableElements[focusableElements.length - 1];

            if (e.shiftKey && document.activeElement === firstElement) {
                e.preventDefault();
                lastElement?.focus();
            } else if (!e.shiftKey && document.activeElement === lastElement) {
                e.preventDefault();
                firstElement?.focus();
            }
        };

        document.addEventListener("keydown", handleKeyDown);
        return () => document.removeEventListener("keydown", handleKeyDown);
    }, [isOpen]);

    const handleCopyVersion = () => {
        navigator.clipboard.writeText(__GIT_HASH__);
        setCopyAnnouncement("Version hash copied to clipboard");
        // Clear announcement after it's been read
        setTimeout(() => setCopyAnnouncement(""), 2000);
    };

    if (!isOpen) return null;

    return (
        <>
            {/* Backdrop */}
            <div
                className="settings-backdrop"
                onClick={onClose}
                aria-hidden="true"
            />

            {/* Settings panel - proper dialog */}
            <div
                ref={panelRef}
                className="settings-panel"
                role="dialog"
                aria-modal="true"
                aria-labelledby="settings-dialog-title"
            >
                <div className="settings-header">
                    <h2 id="settings-dialog-title">Settings</h2>
                    <button
                        ref={closeButtonRef}
                        className="settings-close"
                        onClick={onClose}
                        aria-label="Close settings"
                    >
                        ×
                    </button>
                </div>

                <div className="settings-content">
                    <div className="settings-section">
                        <h3>Display</h3>
                        <ThemeToggle />
                        <FontToggle />
                        <div className="settings-item">
                            <span>Font size</span>
                            <FontSizeControl
                                fontSize={narrativeFontSize}
                                onDecrease={onDecreaseNarrativeFontSize}
                                onIncrease={onIncreaseNarrativeFontSize}
                            />
                        </div>
                        <EmojiToggle />
                    </div>

                    <div className="settings-section">
                        <h3>Interface</h3>
                        <CommandEchoToggle />
                        <SayModeToggle />
                        <VerbPaletteToggle />
                    </div>

                    <div className="settings-section">
                        <h3>About</h3>
                        <div className="settings-item">
                            <span>Version</span>
                            <button
                                className="version-copy-button"
                                onClick={handleCopyVersion}
                                aria-label={`Version ${__GIT_HASH__}. Click to copy to clipboard`}
                            >
                                {__GIT_HASH__}
                            </button>
                        </div>
                    </div>
                </div>

                {/* Accessible announcement for copy action */}
                <div
                    role="status"
                    aria-live="polite"
                    aria-atomic="true"
                    className="sr-only"
                >
                    {copyAnnouncement}
                </div>
            </div>
        </>
    );
};
