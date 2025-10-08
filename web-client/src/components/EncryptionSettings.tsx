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
import { useEncryptionContext } from "../context/EncryptionContext";
import { EncryptionSetupPrompt } from "./EncryptionSetupPrompt";

export const EncryptionSettings: React.FC = () => {
    const { encryptionState, forgetKey, setupEncryption } = useEncryptionContext();
    const [showForgetConfirm, setShowForgetConfirm] = useState(false);
    const [showSetupPrompt, setShowSetupPrompt] = useState(false);

    const handleForgetKey = () => {
        forgetKey();
        setShowForgetConfirm(false);
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
                        {encryptionState.derivedKeyBytes
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

                {showSetupPrompt && (
                    <EncryptionSetupPrompt
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
