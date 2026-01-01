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

// ! Modal for setting up player profile after account creation

import React, { useRef, useState } from "react";
import AvatarEditor from "react-avatar-editor";
import { usePlayerDescription } from "../hooks/usePlayerDescription";
import { useProfilePicture } from "../hooks/useProfilePicture";
import { usePronouns } from "../hooks/usePronouns";
import { Presentation } from "../types/presentation";

interface ProfileSetupPanelProps {
    presentation: Presentation;
    authToken: string;
    playerOid: string;
    onComplete: () => void;
    onSkip: () => void;
}

export const ProfileSetupPanel: React.FC<ProfileSetupPanelProps> = ({
    presentation,
    authToken,
    playerOid,
    onComplete,
    onSkip,
}) => {
    // Use existing hooks for profile picture and description
    const { uploadProfilePicture } = useProfilePicture(authToken, playerOid);
    const { updatePlayerDescription } = usePlayerDescription(authToken, playerOid);
    const { availablePresets, updatePronouns } = usePronouns(authToken, playerOid);

    // Profile picture state
    const fileInputRef = useRef<HTMLInputElement>(null);
    const editorRef = useRef<AvatarEditor>(null);
    const [selectedImage, setSelectedImage] = useState<File | null>(null);
    const [editorScale, setEditorScale] = useState(1.0);
    const [croppedImageBlob, setCroppedImageBlob] = useState<Blob | null>(null);
    const [previewUrl, setPreviewUrl] = useState<string | null>(null);

    // Description state
    const [description, setDescription] = useState("");

    // Pronouns state - default to first available preset
    const [pronouns, setPronouns] = useState("");

    // Submission state
    const [submitting, setSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Parse fields from presentation attributes
    const fields = presentation.attrs.fields?.split(",") || ["pronouns", "description", "picture"];
    const showPronouns = fields.includes("pronouns") && availablePresets.length > 0;
    const showDescription = fields.includes("description");
    const showPicture = fields.includes("picture");

    // Set default pronouns when presets load
    React.useEffect(() => {
        if (availablePresets.length > 0 && !pronouns) {
            setPronouns(availablePresets[0]);
        }
    }, [availablePresets, pronouns]);

    const handleFileSelect = (event: React.ChangeEvent<HTMLInputElement>) => {
        const file = event.target.files?.[0];
        if (!file) return;

        if (!file.type.startsWith("image/")) {
            setError("Please select an image file");
            return;
        }

        if (file.size > 5 * 1024 * 1024) {
            setError("Image must be smaller than 5MB");
            return;
        }

        setSelectedImage(file);
        setEditorScale(1.0);
        setError(null);
        event.target.value = "";
    };

    const handleCropConfirm = () => {
        if (!editorRef.current || !selectedImage) return;

        const canvas = editorRef.current.getImageScaledToCanvas();
        canvas.toBlob((blob: Blob | null) => {
            if (!blob) {
                setError("Failed to process image");
                return;
            }

            setCroppedImageBlob(blob);
            // Create preview URL
            if (previewUrl) {
                URL.revokeObjectURL(previewUrl);
            }
            setPreviewUrl(URL.createObjectURL(blob));
            setSelectedImage(null);
        }, selectedImage.type);
    };

    const handleCropCancel = () => {
        setSelectedImage(null);
        setEditorScale(1.0);
    };

    const handleRemovePicture = () => {
        if (previewUrl) {
            URL.revokeObjectURL(previewUrl);
        }
        setPreviewUrl(null);
        setCroppedImageBlob(null);
    };

    const handleComplete = async () => {
        setSubmitting(true);
        setError(null);

        try {
            // Upload profile picture if one was selected (cropped or original)
            if (showPicture && (croppedImageBlob || selectedImage)) {
                try {
                    let fileToUpload: File;
                    if (croppedImageBlob) {
                        fileToUpload = new File([croppedImageBlob], "profile.jpg", {
                            type: croppedImageBlob.type || "image/jpeg",
                        });
                    } else if (selectedImage) {
                        fileToUpload = selectedImage;
                    } else {
                        throw new Error("No image to upload");
                    }
                    await uploadProfilePicture(fileToUpload);
                } catch (err) {
                    console.error("Failed to upload profile picture:", err);
                    // Don't fail the whole process for picture upload failure
                }
            }

            // Set pronouns if provided
            if (showPronouns && pronouns) {
                try {
                    await updatePronouns(pronouns);
                } catch (err) {
                    console.error("Failed to set pronouns:", err);
                    // Don't fail the whole process
                }
            }

            // Set description if provided
            if (showDescription && description.trim()) {
                try {
                    await updatePlayerDescription(description.trim());
                } catch (err) {
                    console.error("Failed to set description:", err);
                    // Don't fail the whole process
                }
            }

            onComplete();
        } catch (err) {
            console.error("Profile setup error:", err);
            setError(err instanceof Error ? err.message : "Failed to save profile");
            setSubmitting(false);
        }
    };

    // Cleanup on unmount
    React.useEffect(() => {
        return () => {
            if (previewUrl) {
                URL.revokeObjectURL(previewUrl);
            }
        };
    }, [previewUrl]);

    return (
        <>
            <div className="dialog-backdrop" onClick={onSkip} />
            <div className="dialog-sheet dialog-form profile-setup-panel">
                <div className="dialog-sheet-header">
                    <h2>{presentation.title || "Set Up Your Profile"}</h2>
                </div>

                <div className="dialog-sheet-content">
                    <p className="profile-setup-intro">
                        Welcome! Take a moment to personalize your profile. You can always change these later from the
                        Account menu.
                    </p>

                    {error && (
                        <div className="profile-setup-error">
                            {error}
                        </div>
                    )}

                    {/* Profile Picture */}
                    {showPicture && !selectedImage && (
                        <div className="profile-setup-section">
                            <label>Profile Picture</label>
                            <div className="profile-setup-picture">
                                {previewUrl
                                    ? (
                                        <>
                                            <img
                                                src={previewUrl}
                                                alt="Profile preview"
                                                className="profile-setup-preview"
                                            />
                                            <div className="profile-setup-picture-actions">
                                                <button
                                                    type="button"
                                                    className="btn btn-secondary"
                                                    onClick={() => fileInputRef.current?.click()}
                                                >
                                                    Change
                                                </button>
                                                <button
                                                    type="button"
                                                    className="btn btn-secondary"
                                                    onClick={handleRemovePicture}
                                                >
                                                    Remove
                                                </button>
                                            </div>
                                        </>
                                    )
                                    : (
                                        <>
                                            <div className="profile-setup-picture-placeholder">
                                                No picture
                                            </div>
                                            <button
                                                type="button"
                                                className="btn btn-secondary"
                                                onClick={() => fileInputRef.current?.click()}
                                            >
                                                Upload Picture
                                            </button>
                                        </>
                                    )}
                                <input
                                    ref={fileInputRef}
                                    type="file"
                                    accept="image/*"
                                    onChange={handleFileSelect}
                                    style={{ display: "none" }}
                                />
                            </div>
                        </div>
                    )}

                    {/* Avatar Editor Modal */}
                    {selectedImage && (
                        <div className="profile-setup-section profile-setup-editor">
                            <label>Crop Your Picture</label>
                            <div className="profile-setup-editor-canvas">
                                <AvatarEditor
                                    ref={editorRef}
                                    image={selectedImage}
                                    width={200}
                                    height={200}
                                    border={20}
                                    borderRadius={100}
                                    color={[0, 0, 0, 0.6]}
                                    scale={editorScale}
                                    rotate={0}
                                />
                            </div>
                            <div className="profile-setup-zoom">
                                <label>Zoom</label>
                                <input
                                    type="range"
                                    min="1"
                                    max="3"
                                    step="0.01"
                                    value={editorScale}
                                    onChange={(e) => setEditorScale(parseFloat(e.target.value))}
                                />
                            </div>
                            <div className="profile-setup-editor-actions">
                                <button
                                    type="button"
                                    className="btn btn-secondary"
                                    onClick={handleCropCancel}
                                >
                                    Remove
                                </button>
                                <button
                                    type="button"
                                    className="btn btn-primary"
                                    onClick={handleCropConfirm}
                                >
                                    Crop
                                </button>
                            </div>
                        </div>
                    )}

                    {/* Pronouns */}
                    {showPronouns && (
                        <div className="profile-setup-section">
                            <label htmlFor="profile-pronouns">Pronouns</label>
                            <select
                                id="profile-pronouns"
                                value={pronouns}
                                onChange={(e) => setPronouns(e.target.value)}
                                className="profile-setup-select"
                            >
                                {availablePresets.map((preset) => (
                                    <option key={preset} value={preset}>{preset}</option>
                                ))}
                            </select>
                        </div>
                    )}

                    {/* Description */}
                    {showDescription && (
                        <div className="profile-setup-section">
                            <label htmlFor="profile-description">Description</label>
                            <textarea
                                id="profile-description"
                                value={description}
                                onChange={(e) => setDescription(e.target.value)}
                                placeholder="Describe yourself..."
                                rows={4}
                                className="profile-setup-textarea"
                            />
                            <p className="profile-setup-hint">
                                This is what others see when they look at you.
                            </p>
                        </div>
                    )}
                </div>

                <div className="dialog-sheet-footer">
                    <button
                        className="btn btn-secondary"
                        onClick={onSkip}
                        disabled={submitting}
                    >
                        Skip for now
                    </button>
                    <button
                        className="btn btn-primary"
                        onClick={handleComplete}
                        disabled={submitting}
                    >
                        {submitting ? "Saving..." : "Complete Setup"}
                    </button>
                </div>
            </div>
        </>
    );
};
