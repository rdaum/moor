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

// ! Encryption settings component for settings panel

import React, { useEffect, useState } from "react";
import { useAuthContext } from "../context/AuthContext";
import { useEncryptionContext } from "../context/EncryptionContext";
import { useHistoryExport } from "../hooks/useHistoryExport";
import { useTitle } from "../hooks/useTitle";
import { EncryptionPasswordPrompt } from "./EncryptionPasswordPrompt";
import { EncryptionSetupPrompt } from "./EncryptionSetupPrompt";

interface EncryptionSettingsProps {
    isAvailable: boolean;
}

export const EncryptionSettings: React.FC<EncryptionSettingsProps> = ({ isAvailable }) => {
    const { authState } = useAuthContext();
    const { encryptionState, forgetKey, setupEncryption } = useEncryptionContext();
    const { exportState, startExport, cancelExport, downloadReady, dismissReady } = useHistoryExport();
    const systemTitle = useTitle();
    const [showForgetConfirm, setShowForgetConfirm] = useState(false);
    const [showSetupPrompt, setShowSetupPrompt] = useState(false);
    const [showPasswordPrompt, setShowPasswordPrompt] = useState(false);
    const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
    const [isDeleting, setIsDeleting] = useState(false);

    useEffect(() => {
        if (!isAvailable) {
            setShowForgetConfirm(false);
            setShowSetupPrompt(false);
            setShowPasswordPrompt(false);
            setShowDeleteConfirm(false);
            setIsDeleting(false);
        }
    }, [isAvailable]);

    const handleForgetKey = () => {
        forgetKey();
        setShowForgetConfirm(false);
    };

    const handleDeleteHistory = async () => {
        setIsDeleting(true);
        try {
            const authToken = authState.player?.authToken;
            if (!authToken) {
                throw new Error("No auth token found");
            }
            const response = await fetch("/api/event-log/history", {
                method: "DELETE",
                headers: {
                    "X-Moor-Auth-Token": authToken,
                },
            });

            if (!response.ok) {
                throw new Error("Failed to delete history");
            }

            const result = await response.json();
            if (result.success) {
                alert("Event history deleted successfully");
            } else {
                alert("Failed to delete history");
            }
        } catch (error) {
            console.error("Error deleting history:", error);
            alert("Error deleting history");
        } finally {
            setIsDeleting(false);
            setShowDeleteConfirm(false);
        }
    };

    const handleDownloadHistory = async () => {
        const authToken = authState.player?.authToken;
        const ageIdentity = encryptionState.ageIdentity;
        const playerOid = authState.player?.oid;

        if (!authToken) {
            alert("No auth token found");
            return;
        }

        if (!ageIdentity) {
            alert("No encryption key available. Please enter your password to unlock encryption.");
            return;
        }

        if (!playerOid) {
            alert("No player ID found");
            return;
        }

        // Warn user about potential wait time
        if (
            !confirm(
                "This will download and decrypt your entire event history. Depending on the size of your history, this may take several minutes. Continue?",
            )
        ) {
            return;
        }

        try {
            await startExport(authToken, ageIdentity, systemTitle, playerOid);
        } catch (error) {
            console.error("Error downloading history:", error);
            alert(`Error downloading history: ${error instanceof Error ? error.message : "Unknown error"}`);
        }
    };

    if (!isAvailable) {
        return (
            <div className="settings-item" role="region" aria-labelledby="encryption-settings-label">
                <div className="settings-stack">
                    <div className="settings-header">
                        <span id="encryption-settings-label">History Encryption</span>
                        <span
                            role="status"
                            aria-label="History features disabled"
                            className="settings-badge unavailable"
                        >
                            Unavailable
                        </span>
                    </div>
                    <p className="settings-description">
                        Message history is not available on this server.
                    </p>
                </div>
            </div>
        );
    }

    return (
        <div className="settings-item" role="region" aria-labelledby="encryption-settings-label">
            <div className="settings-stack">
                <div className="settings-header">
                    <span id="encryption-settings-label">History Encryption</span>
                    <span
                        role="status"
                        aria-label={`History encryption is ${encryptionState.hasEncryption ? "enabled" : "not set up"}`}
                        className={`settings-badge ${encryptionState.hasEncryption ? "enabled" : "disabled"}`}
                    >
                        {encryptionState.hasEncryption ? "Enabled" : "Not Set Up"}
                    </span>
                </div>

                {!encryptionState.hasEncryption && (
                    <div className="settings-action-group">
                        <button
                            onClick={() => setShowSetupPrompt(true)}
                            aria-label="Set up history encryption with a password"
                            className="btn btn-primary"
                        >
                            Set Up Encryption
                        </button>
                    </div>
                )}

                {encryptionState.hasEncryption && (
                    <div className="encryption-section">
                        {encryptionState.ageIdentity
                            ? (
                                <>
                                    <div className="mb-sm">
                                        Password saved in browser storage. You won't need to re-enter it on this device.
                                    </div>
                                    <div className="mt-sm">
                                        {!showForgetConfirm
                                            ? (
                                                <button
                                                    onClick={() => setShowForgetConfirm(true)}
                                                    aria-label="Remove saved password from this browser"
                                                    className="btn btn-danger btn-sm"
                                                >
                                                    Remove Saved Password
                                                </button>
                                            )
                                            : (
                                                <div
                                                    role="alertdialog"
                                                    aria-labelledby="remove-password-title"
                                                    aria-describedby="remove-password-description"
                                                    className="encryption-dialog"
                                                >
                                                    <div
                                                        id="remove-password-description"
                                                        className="encryption-description"
                                                    >
                                                        <p>
                                                            This will remove the saved password from this browser.
                                                            You'll need to re-enter it next time. Your history on the
                                                            server will remain encrypted and accessible with your
                                                            password.
                                                        </p>
                                                        <p className="warning">
                                                            ⚠️ Remember: If you've forgotten your password, there is no
                                                            way to recover it or your history.
                                                        </p>
                                                    </div>
                                                    <div className="encryption-button-row">
                                                        <button
                                                            onClick={handleForgetKey}
                                                            aria-label="Confirm removal of saved password"
                                                            className="btn btn-danger btn-sm"
                                                        >
                                                            Remove Password
                                                        </button>
                                                        <button
                                                            onClick={() => setShowForgetConfirm(false)}
                                                            aria-label="Cancel password removal"
                                                            className="btn btn-secondary btn-sm"
                                                        >
                                                            Cancel
                                                        </button>
                                                    </div>
                                                </div>
                                            )}
                                    </div>
                                </>
                            )
                            : (
                                <div role="status">
                                    <div className="mb-md">
                                        History encryption is enabled on the server, but your password isn't saved in
                                        this browser. You'll need to unlock it with your password to view history.
                                    </div>
                                    <button
                                        onClick={() => setShowPasswordPrompt(true)}
                                        aria-label="Unlock history with password"
                                        className="btn btn-primary"
                                    >
                                        Unlock History
                                    </button>
                                </div>
                            )}
                    </div>
                )}

                {encryptionState.hasEncryption && (
                    <div className="encryption-section-title">
                        <div className="font-semibold mb-sm">History Management</div>
                        <div className="flex-col gap-sm">
                            {exportState.readyBlob && exportState.readyFilename && (
                                <div className="encryption-message-box-success">
                                    <div className="font-semibold mb-sm text-success">
                                        ✓ Export Ready!
                                    </div>
                                    <div className="mb-md">
                                        Your history export is ready to download:{" "}
                                        <strong>{exportState.readyFilename}</strong>
                                    </div>
                                    <div className="encryption-button-row">
                                        <button
                                            onClick={downloadReady}
                                            className="btn btn-primary"
                                        >
                                            Download Now
                                        </button>
                                        <button
                                            onClick={dismissReady}
                                            className="btn btn-secondary"
                                        >
                                            Dismiss
                                        </button>
                                    </div>
                                </div>
                            )}
                            {exportState.isExporting && exportState.progress && (
                                <div className="encryption-progress-container">
                                    <div className="encryption-progress-label">
                                        Exporting history...
                                        {exportState.progress.total && (
                                            <span>
                                                {" "}
                                                {exportState.progress.processed} / {exportState.progress.total}
                                            </span>
                                        )}
                                        {!exportState.progress.total && exportState.progress.processed > 0 && (
                                            <span>({exportState.progress.processed} events)</span>
                                        )}
                                    </div>
                                    {exportState.progress.total && (
                                        <div className="encryption-progress-bar-track">
                                            <div
                                                className="encryption-progress-bar-fill"
                                                style={{
                                                    width: `${
                                                        (exportState.progress.processed / exportState.progress.total)
                                                        * 100
                                                    }%`,
                                                }}
                                            />
                                        </div>
                                    )}
                                    <button
                                        onClick={cancelExport}
                                        className="btn btn-secondary btn-sm mt-sm"
                                    >
                                        Cancel
                                    </button>
                                </div>
                            )}
                            <button
                                onClick={handleDownloadHistory}
                                disabled={exportState.isExporting || !encryptionState.ageIdentity}
                                aria-label="Download all encrypted event history"
                                className="btn btn-secondary"
                            >
                                Download All History (JSON)
                            </button>

                            {!showDeleteConfirm
                                ? (
                                    <button
                                        onClick={() => setShowDeleteConfirm(true)}
                                        aria-label="Delete all event history from server"
                                        className="btn btn-danger"
                                    >
                                        Delete All History
                                    </button>
                                )
                                : (
                                    <div className="flex-col gap-sm">
                                        <div className="encryption-warning">
                                            ⚠️ This will permanently delete all your event history from the server. This
                                            cannot be undone!
                                        </div>
                                        <div className="encryption-button-row">
                                            <button
                                                onClick={handleDeleteHistory}
                                                disabled={isDeleting}
                                                aria-label="Confirm deletion of all history"
                                                className="btn btn-danger btn-sm"
                                            >
                                                {isDeleting ? "Deleting..." : "Yes, Delete Everything"}
                                            </button>
                                            <button
                                                onClick={() => setShowDeleteConfirm(false)}
                                                disabled={isDeleting}
                                                aria-label="Cancel history deletion"
                                                className="btn btn-secondary btn-sm"
                                            >
                                                Cancel
                                            </button>
                                        </div>
                                    </div>
                                )}
                        </div>
                    </div>
                )}

                {showSetupPrompt && (
                    <EncryptionSetupPrompt
                        systemTitle={systemTitle}
                        onSetup={async (password) => {
                            const result = await setupEncryption(password);
                            if (result.success) {
                                setShowSetupPrompt(false);
                            }
                            return result;
                        }}
                        onSkip={() => setShowSetupPrompt(false)}
                    />
                )}

                {showPasswordPrompt && (
                    <EncryptionPasswordPrompt
                        systemTitle={systemTitle}
                        onUnlock={async (password) => {
                            const result = await setupEncryption(password);
                            if (result.success) {
                                setShowPasswordPrompt(false);
                            }
                            return result;
                        }}
                        onForgotPassword={() => {
                            setShowPasswordPrompt(false);
                            setShowSetupPrompt(true);
                        }}
                        onSkip={() => setShowPasswordPrompt(false)}
                    />
                )}
            </div>
        </div>
    );
};
