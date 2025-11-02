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

// ! Account menu with user-specific settings and logout

import React, { useMemo, useRef, useState } from "react";
import AvatarEditor from "react-avatar-editor";
import { usePlayerDescription } from "../hooks/usePlayerDescription";
import { useProfilePicture } from "../hooks/useProfilePicture";
import { EncryptionSettings } from "./EncryptionSettings";

interface AccountMenuProps {
    isOpen: boolean;
    onClose: () => void;
    onLogout?: () => void;
    historyAvailable: boolean;
    authToken: string | null;
    playerOid: string | null;
}

export const AccountMenu: React.FC<AccountMenuProps> = ({
    isOpen,
    onClose,
    onLogout,
    historyAvailable,
    authToken,
    playerOid,
}) => {
    const { profilePicture, loading, uploadProfilePicture } = useProfilePicture(authToken, playerOid);
    const {
        playerDescription,
        loading: descriptionLoading,
        updatePlayerDescription,
    } = usePlayerDescription(authToken, playerOid);

    const fileInputRef = useRef<HTMLInputElement>(null);
    const editorRef = useRef<AvatarEditor>(null);
    const [editorOpen, setEditorOpen] = useState(false);
    const [selectedImage, setSelectedImage] = useState<File | null>(null);
    const [editorScale, setEditorScale] = useState(1.0);

    // Description editor state
    const [descriptionEditorOpen, setDescriptionEditorOpen] = useState(false);
    const [editingDescription, setEditingDescription] = useState("");

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
            <div className="settings-backdrop" onClick={onClose} />

            {/* Account menu */}
            <div className="account-menu">
                <div className="settings-header">
                    <h2>Account</h2>
                    <button
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
                    <div className="dialog-backdrop" onClick={handleCropCancel} />
                    <div className="dialog-sheet dialog-form">
                        <div className="dialog-sheet-header">
                            <h2>Crop Profile Picture</h2>
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
                    <div className="dialog-backdrop" onClick={handleDescriptionCancel} />
                    <div className="dialog-sheet dialog-form">
                        <div className="dialog-sheet-header">
                            <h2>Edit Description</h2>
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
