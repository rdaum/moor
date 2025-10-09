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

// ! Modal prompt for entering existing encryption password

import React, { useState } from "react";
import { EncryptionResetConfirm } from "./EncryptionResetConfirm";

interface EncryptionPasswordPromptProps {
    systemTitle: string;
    onUnlock: (password: string) => Promise<{ success: boolean; error?: string }>;
    onForgotPassword: () => void;
    onSkip: () => void;
}

export const EncryptionPasswordPrompt: React.FC<EncryptionPasswordPromptProps> = ({
    systemTitle,
    onUnlock,
    onForgotPassword,
    onSkip,
}) => {
    const [password, setPassword] = useState("");
    const [error, setError] = useState<string | null>(null);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [showResetConfirm, setShowResetConfirm] = useState(false);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError(null);

        if (!password) {
            setError("Password is required");
            return;
        }

        setIsSubmitting(true);
        const result = await onUnlock(password);
        setIsSubmitting(false);

        if (!result.success) {
            setError(result.error || "Failed to unlock history. Please check your password.");
        }
    };

    if (showResetConfirm) {
        return (
            <EncryptionResetConfirm
                onConfirm={() => {
                    setShowResetConfirm(false);
                    onForgotPassword();
                }}
                onCancel={() => setShowResetConfirm(false)}
            />
        );
    }

    return (
        <>
            <div
                className="settings-backdrop"
                role="presentation"
                aria-hidden="true"
            />
            <div
                className="settings-panel"
                style={{ maxWidth: "500px" }}
                role="dialog"
                aria-labelledby="encryption-unlock-title"
                aria-describedby="encryption-unlock-description"
            >
                <div className="settings-header">
                    <h2 id="encryption-unlock-title">Enter History Password</h2>
                </div>

                <div className="settings-content">
                    <div
                        id="encryption-unlock-description"
                        style={{
                            marginBottom: "1em",
                            color: "var(--color-text-secondary)",
                            lineHeight: "1.5",
                            fontFamily: "inherit",
                        }}
                    >
                        <p style={{ marginBottom: "0.75em", fontFamily: "inherit" }}>
                            Your history is encrypted. Enter your encryption password to access it.
                        </p>
                        <p style={{ marginBottom: "0", fontFamily: "inherit" }}>
                            This is the password you set up for history encryption, separate from your {systemTitle}
                            {" "}
                            login.
                        </p>
                    </div>

                    <form onSubmit={handleSubmit} aria-label="History password entry form">
                        <div style={{ marginBottom: "1em" }}>
                            <label
                                htmlFor="unlock-password"
                                style={{
                                    display: "block",
                                    marginBottom: "0.5em",
                                    color: "var(--color-text-primary)",
                                    fontFamily: "inherit",
                                }}
                            >
                                Encryption Password
                            </label>
                            <input
                                id="unlock-password"
                                type="password"
                                value={password}
                                onChange={(e) => setPassword(e.target.value)}
                                disabled={isSubmitting}
                                required
                                aria-required="true"
                                aria-invalid={error ? "true" : "false"}
                                aria-describedby={error ? "unlock-error" : undefined}
                                placeholder="Enter your encryption password"
                                autoComplete="current-password"
                                autoFocus
                                style={{
                                    width: "100%",
                                    padding: "0.5em",
                                    borderRadius: "var(--radius-md)",
                                    border: "1px solid var(--color-border-medium)",
                                    backgroundColor: "var(--color-bg-input)",
                                    color: "var(--color-text-primary)",
                                    fontFamily: "inherit",
                                    fontSize: "1em",
                                    outline: "none",
                                }}
                            />
                        </div>

                        {error && (
                            <div
                                id="unlock-error"
                                role="alert"
                                aria-live="assertive"
                                style={{
                                    padding: "0.75em",
                                    backgroundColor: "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                                    border: "1px solid var(--color-text-error)",
                                    borderRadius: "var(--radius-md)",
                                    marginBottom: "1em",
                                    color: "var(--color-text-primary)",
                                    fontFamily: "inherit",
                                }}
                            >
                                {error}
                            </div>
                        )}

                        <div style={{ display: "flex", flexDirection: "column", gap: "0.5em" }}>
                            <button
                                type="submit"
                                disabled={isSubmitting}
                                aria-label="Unlock history with entered password"
                                style={{
                                    padding: "0.5em 1em",
                                    borderRadius: "var(--radius-md)",
                                    border: "none",
                                    backgroundColor: "var(--color-button-primary)",
                                    color: "var(--color-bg-base)",
                                    cursor: isSubmitting ? "not-allowed" : "pointer",
                                    fontFamily: "inherit",
                                    fontSize: "1em",
                                    fontWeight: "bold",
                                    transition: "opacity var(--transition-fast)",
                                }}
                            >
                                {isSubmitting ? "Unlocking..." : "Unlock History"}
                            </button>
                            <button
                                type="button"
                                onClick={() => setShowResetConfirm(true)}
                                disabled={isSubmitting}
                                aria-label="I forgot my password - set up new encryption and lose access to old history"
                                style={{
                                    padding: "0.5em 1em",
                                    borderRadius: "var(--radius-md)",
                                    border: "1px solid var(--color-border-medium)",
                                    backgroundColor: "var(--color-bg-secondary)",
                                    color: "var(--color-text-primary)",
                                    cursor: isSubmitting ? "not-allowed" : "pointer",
                                    fontFamily: "inherit",
                                    fontSize: "0.9em",
                                    transition: "background-color var(--transition-fast)",
                                }}
                            >
                                I Forgot My Password
                            </button>
                            <button
                                type="button"
                                onClick={onSkip}
                                disabled={isSubmitting}
                                aria-label="Skip history decryption for now"
                                style={{
                                    padding: "0.5em 1em",
                                    borderRadius: "var(--radius-md)",
                                    border: "1px solid var(--color-border-medium)",
                                    backgroundColor: "transparent",
                                    color: "var(--color-text-secondary)",
                                    cursor: isSubmitting ? "not-allowed" : "pointer",
                                    fontFamily: "inherit",
                                    fontSize: "0.9em",
                                    transition: "background-color var(--transition-fast)",
                                }}
                            >
                                Skip for Now
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </>
    );
};
