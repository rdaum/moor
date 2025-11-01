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
                className="dialog-sheet-backdrop"
                onClick={onCancel}
                role="presentation"
                aria-hidden="true"
            />
            <div
                className="dialog-sheet"
                style={{ maxWidth: "500px" }}
                role="alertdialog"
                aria-labelledby="reset-confirm-title"
                aria-describedby="reset-confirm-description"
            >
                <div className="dialog-sheet-header">
                    <h2 id="reset-confirm-title">Reset Encryption?</h2>
                </div>

                <div className="dialog-sheet-content">
                    <div
                        id="reset-confirm-description"
                        role="alert"
                        aria-live="assertive"
                        className="encryption-message-box-error"
                    >
                        <p className="font-semibold mb-md">
                            ⚠️ Warning: You will permanently lose access to your existing history.
                        </p>
                        <p className="mb-md">
                            Setting up a new encryption password will create a fresh encrypted history. Your old history
                            will remain on the server, but without your old password, it cannot be decrypted.
                        </p>
                        <p>
                            This action cannot be undone. Your previous conversations, actions, and descriptions will be
                            permanently inaccessible.
                        </p>
                    </div>

                    <div className="mb-md">
                        <label
                            className="flex flex-start gap-sm"
                            style={{ color: "var(--color-text-primary)", cursor: "pointer", userSelect: "none" }}
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

                    <div className="flex gap-sm" style={{ justifyContent: "flex-end" }}>
                        <button
                            type="button"
                            onClick={onCancel}
                            aria-label="Cancel and go back"
                            className="btn btn-secondary"
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
                            className="btn btn-danger"
                        >
                            Reset Encryption
                        </button>
                    </div>
                </div>
            </div>
        </>
    );
};
