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

import React, { useState } from "react";
import { useAuthContext } from "../context/AuthContext";
import { useEncryptionContext } from "../context/EncryptionContext";
import { useHistoryExport } from "../hooks/useHistoryExport";
import { useTitle } from "../hooks/useTitle";
import { EncryptionSetupPrompt } from "./EncryptionSetupPrompt";

export const EncryptionSettings: React.FC = () => {
    const { authState } = useAuthContext();
    const { encryptionState, forgetKey, setupEncryption } = useEncryptionContext();
    const { exportState, startExport, cancelExport, downloadReady, dismissReady } = useHistoryExport();
    const systemTitle = useTitle();
    const [showForgetConfirm, setShowForgetConfirm] = useState(false);
    const [showSetupPrompt, setShowSetupPrompt] = useState(false);
    const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
    const [isDeleting, setIsDeleting] = useState(false);

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

    return (
        <div className="settings-item" role="region" aria-labelledby="encryption-settings-label">
            <div style={{ display: "flex", flexDirection: "column", gap: "0.5em", width: "100%" }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <span id="encryption-settings-label">History Encryption</span>
                    <span
                        role="status"
                        aria-label={`History encryption is ${encryptionState.hasEncryption ? "enabled" : "not set up"}`}
                        style={{
                            padding: "0.25em 0.5em",
                            borderRadius: "var(--radius-md)",
                            fontSize: "0.85em",
                            backgroundColor: encryptionState.hasEncryption
                                ? "color-mix(in srgb, var(--color-text-success) 20%, transparent)"
                                : "color-mix(in srgb, var(--color-text-accent) 20%, transparent)",
                            color: "var(--color-text-primary)",
                            border: `1px solid ${
                                encryptionState.hasEncryption ? "var(--color-text-success)" : "var(--color-text-accent)"
                            }`,
                            fontFamily: "inherit",
                        }}
                    >
                        {encryptionState.hasEncryption ? "Enabled" : "Not Set Up"}
                    </span>
                </div>

                {!encryptionState.hasEncryption && (
                    <div style={{ fontSize: "0.9em", marginTop: "0.5em" }}>
                        <button
                            onClick={() => setShowSetupPrompt(true)}
                            aria-label="Set up history encryption with a password"
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-md)",
                                border: "none",
                                backgroundColor: "var(--color-button-primary)",
                                color: "var(--color-bg-base)",
                                cursor: "pointer",
                                fontFamily: "inherit",
                                fontSize: "1em",
                                fontWeight: "bold",
                                transition: "opacity var(--transition-fast)",
                            }}
                        >
                            Set Up Encryption
                        </button>
                    </div>
                )}

                {encryptionState.hasEncryption && (
                    <div style={{ fontSize: "0.9em", color: "var(--color-text-secondary)" }}>
                        {encryptionState.ageIdentity
                            ? (
                                <>
                                    <div style={{ marginBottom: "0.5em" }}>
                                        Password saved in browser storage. You won't need to re-enter it on this device.
                                    </div>
                                    <div style={{ marginTop: "0.5em" }}>
                                        {!showForgetConfirm
                                            ? (
                                                <button
                                                    onClick={() => setShowForgetConfirm(true)}
                                                    aria-label="Remove saved password from this browser"
                                                    style={{
                                                        padding: "0.4em 0.8em",
                                                        borderRadius: "var(--radius-md)",
                                                        border: "1px solid var(--color-text-error)",
                                                        backgroundColor:
                                                            "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                                                        color: "var(--color-text-primary)",
                                                        cursor: "pointer",
                                                        fontSize: "0.9em",
                                                        fontFamily: "inherit",
                                                        transition: "background-color var(--transition-fast)",
                                                    }}
                                                >
                                                    Remove Saved Password
                                                </button>
                                            )
                                            : (
                                                <div
                                                    role="alertdialog"
                                                    aria-labelledby="remove-password-title"
                                                    aria-describedby="remove-password-description"
                                                    style={{ display: "flex", flexDirection: "column", gap: "0.5em" }}
                                                >
                                                    <div
                                                        id="remove-password-description"
                                                        style={{
                                                            color: "var(--color-text-primary)",
                                                            fontSize: "0.95em",
                                                            lineHeight: "1.4",
                                                        }}
                                                    >
                                                        <p style={{ marginBottom: "0.5em" }}>
                                                            This will remove the saved password from this browser.
                                                            You'll need to re-enter it next time. Your history on the
                                                            server will remain encrypted and accessible with your
                                                            password.
                                                        </p>
                                                        <p style={{ marginBottom: "0", fontWeight: "bold" }}>
                                                            ⚠️ Remember: If you've forgotten your password, there is no
                                                            way to recover it or your history.
                                                        </p>
                                                    </div>
                                                    <div style={{ display: "flex", gap: "0.5em" }}>
                                                        <button
                                                            onClick={handleForgetKey}
                                                            aria-label="Confirm removal of saved password"
                                                            style={{
                                                                padding: "0.4em 0.8em",
                                                                borderRadius: "var(--radius-md)",
                                                                border: "1px solid var(--color-text-error)",
                                                                backgroundColor:
                                                                    "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                                                                color: "var(--color-text-primary)",
                                                                cursor: "pointer",
                                                                fontSize: "0.9em",
                                                                fontFamily: "inherit",
                                                                transition: "background-color var(--transition-fast)",
                                                            }}
                                                        >
                                                            Remove Password
                                                        </button>
                                                        <button
                                                            onClick={() => setShowForgetConfirm(false)}
                                                            aria-label="Cancel password removal"
                                                            style={{
                                                                padding: "0.4em 0.8em",
                                                                borderRadius: "var(--radius-md)",
                                                                border: "1px solid var(--color-border-medium)",
                                                                backgroundColor: "var(--color-bg-secondary)",
                                                                color: "var(--color-text-primary)",
                                                                cursor: "pointer",
                                                                fontSize: "0.9em",
                                                                fontFamily: "inherit",
                                                                transition: "background-color var(--transition-fast)",
                                                            }}
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
                                    Password not stored - you'll need to enter it each time to view history
                                </div>
                            )}
                    </div>
                )}

                {encryptionState.hasEncryption && (
                    <div
                        style={{
                            fontSize: "0.9em",
                            marginTop: "1em",
                            borderTop: "1px solid var(--color-border-light)",
                            paddingTop: "1em",
                        }}
                    >
                        <div style={{ marginBottom: "0.5em", fontWeight: "bold" }}>History Management</div>
                        <div style={{ display: "flex", flexDirection: "column", gap: "0.5em" }}>
                            {exportState.readyBlob && exportState.readyFilename && (
                                <div
                                    style={{
                                        padding: "0.75em",
                                        borderRadius: "var(--radius-md)",
                                        backgroundColor:
                                            "color-mix(in srgb, var(--color-text-success) 15%, transparent)",
                                        border: "1px solid var(--color-text-success)",
                                        fontSize: "0.9em",
                                    }}
                                >
                                    <div
                                        style={{
                                            marginBottom: "0.5em",
                                            fontWeight: "bold",
                                            color: "var(--color-text-success)",
                                        }}
                                    >
                                        ✓ Export Ready!
                                    </div>
                                    <div style={{ marginBottom: "0.75em", fontSize: "0.95em" }}>
                                        Your history export is ready to download:{" "}
                                        <strong>{exportState.readyFilename}</strong>
                                    </div>
                                    <div style={{ display: "flex", gap: "0.5em" }}>
                                        <button
                                            onClick={downloadReady}
                                            style={{
                                                padding: "0.5em 1em",
                                                borderRadius: "var(--radius-md)",
                                                border: "none",
                                                backgroundColor: "var(--color-button-primary)",
                                                color: "var(--color-bg-base)",
                                                cursor: "pointer",
                                                fontFamily: "inherit",
                                                fontSize: "0.95em",
                                                fontWeight: "bold",
                                            }}
                                        >
                                            Download Now
                                        </button>
                                        <button
                                            onClick={dismissReady}
                                            style={{
                                                padding: "0.5em 1em",
                                                borderRadius: "var(--radius-md)",
                                                border: "1px solid var(--color-border-medium)",
                                                backgroundColor: "var(--color-bg-secondary)",
                                                color: "var(--color-text-primary)",
                                                cursor: "pointer",
                                                fontFamily: "inherit",
                                                fontSize: "0.95em",
                                            }}
                                        >
                                            Dismiss
                                        </button>
                                    </div>
                                </div>
                            )}
                            {exportState.isExporting && exportState.progress && (
                                <div
                                    style={{
                                        padding: "0.5em",
                                        borderRadius: "var(--radius-md)",
                                        backgroundColor: "var(--color-bg-tertiary)",
                                        fontSize: "0.9em",
                                    }}
                                >
                                    <div style={{ marginBottom: "0.25em" }}>
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
                                        <div
                                            style={{
                                                width: "100%",
                                                height: "4px",
                                                backgroundColor: "var(--color-border-light)",
                                                borderRadius: "2px",
                                                overflow: "hidden",
                                            }}
                                        >
                                            <div
                                                style={{
                                                    width: `${
                                                        (exportState.progress.processed / exportState.progress.total)
                                                        * 100
                                                    }%`,
                                                    height: "100%",
                                                    backgroundColor: "var(--color-button-primary)",
                                                    transition: "width 0.2s ease",
                                                }}
                                            />
                                        </div>
                                    )}
                                    <button
                                        onClick={cancelExport}
                                        style={{
                                            marginTop: "0.5em",
                                            padding: "0.3em 0.6em",
                                            borderRadius: "var(--radius-sm)",
                                            border: "1px solid var(--color-border-medium)",
                                            backgroundColor: "var(--color-bg-secondary)",
                                            color: "var(--color-text-primary)",
                                            cursor: "pointer",
                                            fontSize: "0.85em",
                                            fontFamily: "inherit",
                                        }}
                                    >
                                        Cancel
                                    </button>
                                </div>
                            )}
                            <button
                                onClick={handleDownloadHistory}
                                disabled={exportState.isExporting || !encryptionState.ageIdentity}
                                aria-label="Download all encrypted event history"
                                style={{
                                    padding: "0.5em 1em",
                                    borderRadius: "var(--radius-md)",
                                    border: "1px solid var(--color-border-medium)",
                                    backgroundColor: "var(--color-bg-secondary)",
                                    color: "var(--color-text-primary)",
                                    cursor: (exportState.isExporting || !encryptionState.ageIdentity)
                                        ? "not-allowed"
                                        : "pointer",
                                    fontFamily: "inherit",
                                    fontSize: "0.95em",
                                    opacity: (exportState.isExporting || !encryptionState.ageIdentity) ? 0.6 : 1,
                                    transition: "opacity var(--transition-fast)",
                                }}
                            >
                                Download All History (JSON)
                            </button>

                            {!showDeleteConfirm
                                ? (
                                    <button
                                        onClick={() => setShowDeleteConfirm(true)}
                                        aria-label="Delete all event history from server"
                                        style={{
                                            padding: "0.5em 1em",
                                            borderRadius: "var(--radius-md)",
                                            border: "1px solid var(--color-text-error)",
                                            backgroundColor:
                                                "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                                            color: "var(--color-text-primary)",
                                            cursor: "pointer",
                                            fontFamily: "inherit",
                                            fontSize: "0.95em",
                                            transition: "background-color var(--transition-fast)",
                                        }}
                                    >
                                        Delete All History
                                    </button>
                                )
                                : (
                                    <div style={{ display: "flex", flexDirection: "column", gap: "0.5em" }}>
                                        <div style={{ color: "var(--color-text-error)", fontSize: "0.95em" }}>
                                            ⚠️ This will permanently delete all your event history from the server. This
                                            cannot be undone!
                                        </div>
                                        <div style={{ display: "flex", gap: "0.5em" }}>
                                            <button
                                                onClick={handleDeleteHistory}
                                                disabled={isDeleting}
                                                aria-label="Confirm deletion of all history"
                                                style={{
                                                    padding: "0.4em 0.8em",
                                                    borderRadius: "var(--radius-md)",
                                                    border: "1px solid var(--color-text-error)",
                                                    backgroundColor:
                                                        "color-mix(in srgb, var(--color-text-error) 25%, transparent)",
                                                    color: "var(--color-text-primary)",
                                                    cursor: isDeleting ? "wait" : "pointer",
                                                    fontSize: "0.9em",
                                                    fontFamily: "inherit",
                                                    fontWeight: "bold",
                                                    opacity: isDeleting ? 0.6 : 1,
                                                }}
                                            >
                                                {isDeleting ? "Deleting..." : "Yes, Delete Everything"}
                                            </button>
                                            <button
                                                onClick={() => setShowDeleteConfirm(false)}
                                                disabled={isDeleting}
                                                aria-label="Cancel history deletion"
                                                style={{
                                                    padding: "0.4em 0.8em",
                                                    borderRadius: "var(--radius-md)",
                                                    border: "1px solid var(--color-border-medium)",
                                                    backgroundColor: "var(--color-bg-secondary)",
                                                    color: "var(--color-text-primary)",
                                                    cursor: isDeleting ? "not-allowed" : "pointer",
                                                    fontSize: "0.9em",
                                                    fontFamily: "inherit",
                                                    opacity: isDeleting ? 0.6 : 1,
                                                }}
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
            </div>
        </div>
    );
};
