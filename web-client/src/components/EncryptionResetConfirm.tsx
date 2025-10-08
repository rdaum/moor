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

// ! Confirmation dialog for resetting encryption (losing old history)

import React, { useState } from "react";

interface EncryptionResetConfirmProps {
    onConfirm: () => void;
    onCancel: () => void;
}

export const EncryptionResetConfirm: React.FC<EncryptionResetConfirmProps> = ({ onConfirm, onCancel }) => {
    const [understood, setUnderstood] = useState(false);

    return (
        <>
            <div
                className="settings-backdrop"
                onClick={onCancel}
                role="presentation"
                aria-hidden="true"
            />
            <div
                className="settings-panel"
                style={{ maxWidth: "500px" }}
                role="alertdialog"
                aria-labelledby="reset-confirm-title"
                aria-describedby="reset-confirm-description"
            >
                <div className="settings-header">
                    <h2 id="reset-confirm-title">Reset Encryption?</h2>
                </div>

                <div className="settings-content">
                    <div
                        id="reset-confirm-description"
                        role="alert"
                        aria-live="assertive"
                        style={{
                            marginBottom: "1em",
                            padding: "0.75em",
                            backgroundColor: "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                            border: "1px solid var(--color-text-error)",
                            borderRadius: "var(--radius-md)",
                            color: "var(--color-text-primary)",
                            lineHeight: "1.5",
                            fontFamily: "inherit",
                        }}
                    >
                        <p style={{ marginBottom: "0.75em", fontWeight: "bold", fontFamily: "inherit" }}>
                            ⚠️ Warning: You will permanently lose access to your existing history.
                        </p>
                        <p style={{ marginBottom: "0.75em", fontFamily: "inherit" }}>
                            Setting up a new encryption password will create a fresh encrypted history. Your old history
                            will remain on the server, but without your old password, it cannot be decrypted.
                        </p>
                        <p style={{ marginBottom: "0", fontFamily: "inherit" }}>
                            This action cannot be undone. Your previous conversations, actions, and descriptions will be
                            permanently inaccessible.
                        </p>
                    </div>

                    <div style={{ marginBottom: "1em" }}>
                        <label
                            style={{
                                display: "flex",
                                alignItems: "flex-start",
                                gap: "0.5em",
                                color: "var(--color-text-primary)",
                                fontFamily: "inherit",
                                fontSize: "0.95em",
                                cursor: "pointer",
                                userSelect: "none",
                            }}
                        >
                            <input
                                id="reset-understand-checkbox"
                                type="checkbox"
                                checked={understood}
                                onChange={(e) => setUnderstood(e.target.checked)}
                                required
                                aria-required="true"
                                aria-label="Confirm understanding of permanent history loss"
                                style={{
                                    marginTop: "0.2em",
                                    cursor: "pointer",
                                    minWidth: "16px",
                                    minHeight: "16px",
                                }}
                            />
                            <span>
                                I understand that I will permanently lose access to my existing history and this cannot
                                be undone.
                            </span>
                        </label>
                    </div>

                    <div style={{ display: "flex", gap: "0.5em", justifyContent: "flex-end" }}>
                        <button
                            type="button"
                            onClick={onCancel}
                            aria-label="Cancel and go back"
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-md)",
                                border: "1px solid var(--color-border-medium)",
                                backgroundColor: "var(--color-bg-secondary)",
                                color: "var(--color-text-primary)",
                                cursor: "pointer",
                                fontFamily: "inherit",
                                fontSize: "1em",
                                transition: "background-color var(--transition-fast)",
                            }}
                        >
                            Go Back
                        </button>
                        <button
                            type="button"
                            onClick={onConfirm}
                            disabled={!understood}
                            aria-label={understood
                                ? "Confirm reset encryption and lose old history"
                                : "Cannot reset until you confirm understanding"}
                            aria-disabled={!understood}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-md)",
                                border: "none",
                                backgroundColor: "var(--color-text-error)",
                                color: "var(--color-bg-base)",
                                cursor: understood ? "pointer" : "not-allowed",
                                fontFamily: "inherit",
                                fontSize: "1em",
                                fontWeight: "bold",
                                transition: "opacity var(--transition-fast)",
                                opacity: understood ? 1 : 0.5,
                            }}
                        >
                            Reset Encryption
                        </button>
                    </div>
                </div>
            </div>
        </>
    );
};
