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

// ! Account menu with user-specific settings and logout

import React, { useEffect, useMemo, useRef, useState } from "react";
import AvatarEditor from "react-avatar-editor";
import { usePlayerDescription } from "../hooks/usePlayerDescription";
import { useProfilePicture } from "../hooks/useProfilePicture";
import { usePronouns } from "../hooks/usePronouns";
import { EncryptionSettings } from "./EncryptionSettings";

interface AccountMenuProps {
    isOpen: boolean;
    onClose: () => void;
    onLogout?: () => void;
    historyAvailable: boolean;
    authToken: string | null;
    playerOid: string | null;
    refreshKey?: number;
}

export const AccountMenu: React.FC<AccountMenuProps> = ({
    isOpen,
    onClose,
    onLogout,
    historyAvailable,
    authToken,
    playerOid,
    refreshKey,
}) => {
    const { profilePicture, loading, uploadProfilePicture, refreshProfilePicture } = useProfilePicture(
        authToken,
        playerOid,
    );
    const {
        playerDescription,
        loading: descriptionLoading,
        updatePlayerDescription,
        refreshPlayerDescription,
    } = usePlayerDescription(authToken, playerOid);
    const {
        currentPronouns,
        availablePresets,
        pronounsAvailable,
        loading: pronounsLoading,
        updatePronouns,
        refreshPronouns,
    } = usePronouns(authToken, playerOid);

    const fileInputRef = useRef<HTMLInputElement>(null);
    const editorRef = useRef<AvatarEditor>(null);
    const closeButtonRef = useRef<HTMLButtonElement>(null);
    const panelRef = useRef<HTMLDivElement>(null);
    const previousActiveElementRef = useRef<HTMLElement | null>(null);
    const [editorOpen, setEditorOpen] = useState(false);
    const [selectedImage, setSelectedImage] = useState<File | null>(null);
    const [editorScale, setEditorScale] = useState(1.0);

    // Description editor state
    const [descriptionEditorOpen, setDescriptionEditorOpen] = useState(false);
    const [editingDescription, setEditingDescription] = useState("");

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

    // Handle Escape key to close (only when no nested modals are open)
    useEffect(() => {
        if (!isOpen) return;

        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === "Escape" && !editorOpen && !descriptionEditorOpen) {
                e.preventDefault();
                e.stopPropagation();
                onClose();
            }
        };

        document.addEventListener("keydown", handleKeyDown);
        return () => document.removeEventListener("keydown", handleKeyDown);
    }, [isOpen, editorOpen, descriptionEditorOpen, onClose]);

    // Trap focus within the dialog (only when no nested modals are open)
    useEffect(() => {
        if (!isOpen || editorOpen || descriptionEditorOpen) return;

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
    }, [isOpen, editorOpen, descriptionEditorOpen]);

    // Convert binary data to blob URL for display
    const profilePictureUrl = useMemo(() => {
        if (!profilePicture) return null;
        const blob = new Blob([profilePicture.data as BlobPart], { type: profilePicture.contentType });
        return URL.createObjectURL(blob);
    }, [profilePicture]);

    // Cleanup blob URL when component unmounts or picture changes
    React.useEffect(() => {
        return () => {
            if (profilePictureUrl) {
                URL.revokeObjectURL(profilePictureUrl);
            }
        };
    }, [profilePictureUrl]);

    // Refresh profile data when refreshKey changes (e.g., after profile setup)
    React.useEffect(() => {
        if (refreshKey && refreshKey > 0) {
            refreshProfilePicture();
            refreshPlayerDescription();
            refreshPronouns();
        }
    }, [refreshKey, refreshProfilePicture, refreshPlayerDescription, refreshPronouns]);

    const handleFileSelect = (event: React.ChangeEvent<HTMLInputElement>) => {
        const file = event.target.files?.[0];
        if (!file) return;

        // Validate file type
        if (!file.type.startsWith("image/")) {
            alert("Please select an image file");
            return;
        }

        // Validate file size (e.g., max 5MB)
        if (file.size > 5 * 1024 * 1024) {
            alert("Image must be smaller than 5MB");
            return;
        }

        // Show editor modal
        setSelectedImage(file);
        setEditorScale(1.0);
        setEditorOpen(true);

        // Reset file input
        event.target.value = "";
    };

    const handleCropConfirm = async () => {
        if (!editorRef.current || !selectedImage) return;

        try {
            // Get the cropped canvas from the editor
            const canvas = editorRef.current.getImageScaledToCanvas();

            // Convert canvas to blob
            canvas.toBlob(async (blob: Blob | null) => {
                if (!blob) {
                    alert("Failed to process image");
                    return;
                }

                // Create a File from the blob
                const croppedFile = new File([blob], selectedImage.name, {
                    type: selectedImage.type,
                });

                // Upload the cropped image
                try {
                    await uploadProfilePicture(croppedFile);
                    setEditorOpen(false);
                    setSelectedImage(null);
                } catch (err) {
                    console.error("Upload failed:", err);
                    alert("Failed to upload profile picture. The verb may not be implemented.");
                }
            }, selectedImage.type);
        } catch (err) {
            console.error("Crop failed:", err);
            alert("Failed to process image");
        }
    };

    const handleCropCancel = () => {
        setEditorOpen(false);
        setSelectedImage(null);
        setEditorScale(1.0);
    };

    const handleEditDescription = () => {
        setEditingDescription(playerDescription || "");
        setDescriptionEditorOpen(true);
    };

    const handleDescriptionSave = async () => {
        try {
            await updatePlayerDescription(editingDescription);
            setDescriptionEditorOpen(false);
        } catch (err) {
            console.error("Save description failed:", err);
            alert("Failed to update description. The verb may not be implemented.");
        }
    };

    const handleDescriptionCancel = () => {
        setDescriptionEditorOpen(false);
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

            {/* Account menu - proper dialog */}
            <div
                ref={panelRef}
                className="account-menu"
                role="dialog"
                aria-modal="true"
                aria-labelledby="account-dialog-title"
            >
                <div className="settings-header">
                    <h2 id="account-dialog-title">Account</h2>
                    <button
                        ref={closeButtonRef}
                        className="settings-close"
                        onClick={onClose}
                        aria-label="Close account menu"
                    >
                        ×
                    </button>
                </div>

                <div className="settings-content">
                    {/* Profile Picture Section */}
                    <div className="settings-section">
                        <h3>Profile Picture</h3>
                        <div
                            style={{
                                display: "flex",
                                flexDirection: "column",
                                alignItems: "center",
                                gap: "12px",
                                padding: "12px 0",
                            }}
                        >
                            {loading
                                ? (
                                    <div
                                        style={{
                                            width: "128px",
                                            height: "128px",
                                            display: "flex",
                                            alignItems: "center",
                                            justifyContent: "center",
                                            border: "2px dashed var(--border-color)",
                                            borderRadius: "8px",
                                        }}
                                    >
                                        Loading...
                                    </div>
                                )
                                : profilePictureUrl
                                ? (
                                    <img
                                        src={profilePictureUrl}
                                        alt="Profile"
                                        style={{
                                            width: "128px",
                                            height: "128px",
                                            objectFit: "cover",
                                            borderRadius: "8px",
                                            border: "2px solid var(--border-color)",
                                        }}
                                    />
                                )
                                : (
                                    <div
                                        style={{
                                            width: "128px",
                                            height: "128px",
                                            display: "flex",
                                            alignItems: "center",
                                            justifyContent: "center",
                                            backgroundColor: "var(--bg-secondary)",
                                            borderRadius: "8px",
                                            border: "2px solid var(--border-color)",
                                            color: "var(--text-secondary)",
                                        }}
                                    >
                                        No picture
                                    </div>
                                )}
                            <input
                                ref={fileInputRef}
                                type="file"
                                accept="image/*"
                                onChange={handleFileSelect}
                                style={{ display: "none" }}
                                aria-label="Select profile picture"
                            />
                            <button
                                className="btn btn-secondary"
                                onClick={() => fileInputRef.current?.click()}
                                disabled={loading}
                            >
                                {profilePictureUrl ? "Change Picture" : "Upload Picture"}
                            </button>
                        </div>
                    </div>

                    {/* Player Description Section */}
                    <div className="settings-section">
                        <h3>Description</h3>
                        <div
                            style={{
                                display: "flex",
                                flexDirection: "column",
                                gap: "12px",
                                padding: "12px 0",
                            }}
                        >
                            {descriptionLoading
                                ? (
                                    <div style={{ color: "var(--text-secondary)" }}>
                                        Loading...
                                    </div>
                                )
                                : playerDescription
                                ? (
                                    <div
                                        style={{
                                            padding: "12px",
                                            backgroundColor: "var(--bg-secondary)",
                                            borderRadius: "8px",
                                            border: "1px solid var(--border-color)",
                                            whiteSpace: "pre-wrap",
                                            fontFamily: "var(--font-mono)",
                                        }}
                                    >
                                        {playerDescription}
                                    </div>
                                )
                                : (
                                    <div
                                        style={{
                                            padding: "12px",
                                            color: "var(--text-secondary)",
                                            fontStyle: "italic",
                                        }}
                                    >
                                        No description set
                                    </div>
                                )}
                            <button
                                className="btn btn-secondary"
                                onClick={handleEditDescription}
                                disabled={descriptionLoading}
                            >
                                {playerDescription ? "Edit Description" : "Add Description"}
                            </button>
                        </div>
                    </div>

                    {/* Pronouns Section - only shown if pronouns are supported */}
                    {pronounsAvailable && (
                        <div className="settings-section">
                            <h3>Pronouns</h3>
                            <div
                                style={{
                                    display: "flex",
                                    flexDirection: "column",
                                    gap: "12px",
                                    padding: "12px 0",
                                }}
                            >
                                {pronounsLoading
                                    ? (
                                        <div style={{ color: "var(--text-secondary)" }}>
                                            Loading...
                                        </div>
                                    )
                                    : (
                                        <select
                                            value={currentPronouns || ""}
                                            onChange={async (e) => {
                                                try {
                                                    await updatePronouns(e.target.value);
                                                } catch {
                                                    // Error handled by hook
                                                }
                                            }}
                                            disabled={pronounsLoading}
                                            aria-label="Select pronouns"
                                            style={{
                                                padding: "8px 12px",
                                                borderRadius: "6px",
                                                border: "1px solid var(--border-color)",
                                                backgroundColor: "var(--bg-secondary)",
                                                color: "var(--text-primary)",
                                                fontSize: "14px",
                                            }}
                                        >
                                            {availablePresets.map((preset) => (
                                                <option key={preset} value={preset}>
                                                    {preset}
                                                </option>
                                            ))}
                                        </select>
                                    )}
                            </div>
                        </div>
                    )}

                    <div className="settings-section">
                        <h3>Security</h3>
                        <EncryptionSettings isAvailable={historyAvailable} />
                    </div>
                </div>

                {onLogout && (
                    <div className="settings-footer">
                        <button
                            className="btn btn-secondary w-full"
                            onClick={() => {
                                onLogout();
                                onClose();
                            }}
                        >
                            Logout
                        </button>
                    </div>
                )}
            </div>

            {/* Avatar Editor Modal */}
            {editorOpen && selectedImage && (
                <>
                    <div className="dialog-backdrop" onClick={handleCropCancel} aria-hidden="true" />
                    <div
                        className="dialog-sheet dialog-form"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="crop-dialog-title"
                    >
                        <div className="dialog-sheet-header">
                            <h2 id="crop-dialog-title">Crop Profile Picture</h2>
                        </div>

                        <div className="dialog-sheet-content">
                            <div
                                style={{
                                    display: "flex",
                                    justifyContent: "center",
                                    padding: "16px 0",
                                }}
                            >
                                <AvatarEditor
                                    ref={editorRef}
                                    image={selectedImage}
                                    width={250}
                                    height={250}
                                    border={20}
                                    borderRadius={125}
                                    color={[0, 0, 0, 0.6]}
                                    scale={editorScale}
                                    rotate={0}
                                />
                            </div>

                            <div
                                style={{
                                    display: "flex",
                                    flexDirection: "column",
                                    gap: "8px",
                                }}
                            >
                                <label
                                    style={{
                                        fontSize: "14px",
                                        color: "var(--text-secondary)",
                                    }}
                                >
                                    Zoom
                                </label>
                                <input
                                    type="range"
                                    min="1"
                                    max="3"
                                    step="0.01"
                                    value={editorScale}
                                    onChange={(e) => setEditorScale(parseFloat(e.target.value))}
                                    style={{ width: "100%" }}
                                />
                            </div>
                        </div>

                        <div className="dialog-sheet-footer">
                            <button
                                className="btn btn-secondary"
                                onClick={handleCropCancel}
                                disabled={loading}
                            >
                                Cancel
                            </button>
                            <button
                                className="btn btn-primary"
                                onClick={handleCropConfirm}
                                disabled={loading}
                            >
                                {loading ? "Uploading..." : "Upload"}
                            </button>
                        </div>
                    </div>
                </>
            )}

            {/* Description Editor Modal */}
            {descriptionEditorOpen && (
                <>
                    <div className="dialog-backdrop" onClick={handleDescriptionCancel} aria-hidden="true" />
                    <div
                        className="dialog-sheet dialog-form"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="description-dialog-title"
                    >
                        <div className="dialog-sheet-header">
                            <h2 id="description-dialog-title">Edit Description</h2>
                        </div>

                        <div className="dialog-sheet-content">
                            <div
                                style={{
                                    display: "flex",
                                    flexDirection: "column",
                                    gap: "8px",
                                }}
                            >
                                <label
                                    style={{
                                        fontSize: "14px",
                                        color: "var(--text-secondary)",
                                    }}
                                >
                                    Description
                                </label>
                                <textarea
                                    value={editingDescription}
                                    onChange={(e) => setEditingDescription(e.target.value)}
                                    rows={10}
                                    style={{
                                        padding: "8px",
                                        borderRadius: "4px",
                                        border: "1px solid var(--border-color)",
                                        backgroundColor: "var(--bg-primary)",
                                        color: "var(--text-primary)",
                                        fontFamily: "var(--font-mono)",
                                        resize: "vertical",
                                    }}
                                />
                            </div>
                        </div>

                        <div className="dialog-sheet-footer">
                            <button
                                className="btn btn-secondary"
                                onClick={handleDescriptionCancel}
                                disabled={descriptionLoading}
                            >
                                Cancel
                            </button>
                            <button
                                className="btn btn-primary"
                                onClick={handleDescriptionSave}
                                disabled={descriptionLoading}
                            >
                                {descriptionLoading ? "Saving..." : "Save"}
                            </button>
                        </div>
                    </div>
                </>
            )}
        </>
    );
};
