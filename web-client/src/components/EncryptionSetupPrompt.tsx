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

// ! Modal prompt for setting up event log encryption

import React, { useState } from "react";

interface EncryptionSetupPromptProps {
    systemTitle: string;
    onSetup: (password: string) => Promise<{ success: boolean; error?: string }>;
    onSkip: () => void;
}

export const EncryptionSetupPrompt: React.FC<EncryptionSetupPromptProps> = ({ systemTitle, onSetup, onSkip }) => {
    const [password, setPassword] = useState("");
    const [confirmPassword, setConfirmPassword] = useState("");
    const [understood, setUnderstood] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [isSubmitting, setIsSubmitting] = useState(false);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError(null);

        if (!password) {
            setError("Password is required");
            return;
        }

        if (password !== confirmPassword) {
            setError("Passwords do not match");
            return;
        }

        if (password.length < 8) {
            setError("Password must be at least 8 characters");
            return;
        }

        if (!understood) {
            setError("Please confirm you understand this password cannot be recovered");
            return;
        }

        setIsSubmitting(true);
        const result = await onSetup(password);
        setIsSubmitting(false);

        if (!result.success) {
            setError(result.error || "Setup failed");
        }
    };

    return (
        <>
            <div
                className="dialog-sheet-backdrop"
                onClick={onSkip}
                role="presentation"
                aria-hidden="true"
            />
            <div
                className="dialog-sheet"
                style={{ maxWidth: "500px" }}
                role="dialog"
                aria-labelledby="encryption-setup-title"
                aria-describedby="encryption-setup-description"
            >
                <div className="dialog-sheet-header">
                    <h2 id="encryption-setup-title">Set Up History Encryption</h2>
                </div>

                <div className="dialog-sheet-content">
                    <div
                        id="encryption-setup-description"
                        className="mb-md"
                        style={{
                            color: "var(--color-text-secondary)",
                            lineHeight: "1.5",
                        }}
                    >
                        <p className="mb-md">
                            Your <strong>history</strong> is a complete record of everything you see and do in{" "}
                            {systemTitle}—conversations, actions, descriptions, and more. It's stored encrypted so only
                            you can read it.
                        </p>
                        <p className="mb-md">
                            With the same encryption password, your history follows you across devices and persists over
                            time. Sign in on your phone, tablet, or laptop—your history is always there.
                        </p>
                        <p>
                            This password is <strong>separate from your {systemTitle} login</strong>{" "}
                            and is used only to protect your history.
                        </p>
                    </div>

                    <div
                        role="alert"
                        aria-live="polite"
                        style={{
                            padding: "0.75em",
                            backgroundColor: "color-mix(in srgb, var(--color-text-accent) 15%, transparent)",
                            border: "1px solid var(--color-text-accent)",
                            borderRadius: "var(--radius-md)",
                            marginBottom: "1em",
                            color: "var(--color-text-primary)",
                        }}
                    >
                        <strong>⚠️ Important:</strong>{" "}
                        If you lose this password, you lose access to your history permanently. There is no password
                        recovery. Write it down somewhere safe.
                    </div>

                    <form onSubmit={handleSubmit} aria-label="History encryption setup form">
                        <div className="mb-md">
                            <label
                                htmlFor="encryption-password"
                                className="block mb-sm"
                                style={{
                                    color: "var(--color-text-primary)",
                                }}
                            >
                                Encryption Password
                            </label>
                            <input
                                id="encryption-password"
                                type="password"
                                value={password}
                                onChange={(e) => setPassword(e.target.value)}
                                disabled={isSubmitting}
                                required
                                minLength={8}
                                aria-required="true"
                                aria-invalid={error ? "true" : "false"}
                                aria-describedby={error ? "password-error" : undefined}
                                placeholder="Enter encryption password"
                                autoComplete="new-password"
                                style={{
                                    width: "100%",
                                    padding: "0.5em",
                                    borderRadius: "var(--radius-md)",
                                    border: "1px solid var(--color-border-medium)",
                                    backgroundColor: "var(--color-bg-input)",
                                    color: "var(--color-text-primary)",
                                    fontSize: "1em",
                                    outline: "none",
                                }}
                            />
                        </div>

                        <div className="mb-md">
                            <label
                                htmlFor="encryption-password-confirm"
                                className="block mb-sm"
                                style={{
                                    color: "var(--color-text-primary)",
                                }}
                            >
                                Confirm Password
                            </label>
                            <input
                                id="encryption-password-confirm"
                                type="password"
                                value={confirmPassword}
                                onChange={(e) => setConfirmPassword(e.target.value)}
                                disabled={isSubmitting}
                                required
                                minLength={8}
                                aria-required="true"
                                aria-invalid={error ? "true" : "false"}
                                aria-describedby={error ? "password-error" : undefined}
                                placeholder="Re-enter encryption password"
                                autoComplete="new-password"
                                style={{
                                    width: "100%",
                                    padding: "0.5em",
                                    borderRadius: "var(--radius-md)",
                                    border: "1px solid var(--color-border-medium)",
                                    backgroundColor: "var(--color-bg-input)",
                                    color: "var(--color-text-primary)",
                                    fontSize: "1em",
                                    outline: "none",
                                }}
                            />
                        </div>

                        <div className="mb-md">
                            <label
                                className="flex flex-start gap-sm"
                                style={{
                                    color: "var(--color-text-primary)",
                                    cursor: "pointer",
                                    userSelect: "none",
                                    fontSize: "0.95em",
                                }}
                            >
                                <input
                                    id="encryption-understand-checkbox"
                                    type="checkbox"
                                    checked={understood}
                                    onChange={(e) => setUnderstood(e.target.checked)}
                                    disabled={isSubmitting}
                                    required
                                    aria-required="true"
                                    aria-label="Confirm understanding of password irrecoverability"
                                    style={{
                                        marginTop: "0.2em",
                                        cursor: isSubmitting ? "not-allowed" : "pointer",
                                        minWidth: "16px",
                                        minHeight: "16px",
                                    }}
                                />
                                <span>
                                    I understand that if I lose this password, I will permanently lose access to my
                                    history. There is no way to recover it.
                                </span>
                            </label>
                        </div>

                        {error && (
                            <div
                                id="password-error"
                                role="alert"
                                aria-live="assertive"
                                className="encryption-message-box-error mb-md"
                            >
                                {error}
                            </div>
                        )}

                        <div className="flex gap-sm" style={{ justifyContent: "flex-end" }}>
                            <button
                                type="button"
                                onClick={onSkip}
                                disabled={isSubmitting}
                                aria-label="Skip encryption setup and continue without history encryption"
                                className="btn btn-secondary"
                            >
                                Skip for Now
                            </button>
                            <button
                                type="submit"
                                disabled={isSubmitting || !understood}
                                aria-label={understood
                                    ? "Set up history encryption with entered password"
                                    : "Cannot set up encryption until you confirm understanding"}
                                aria-disabled={!understood}
                                className="btn btn-primary"
                            >
                                {isSubmitting ? "Setting up..." : "Set Up Encryption"}
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </>
    );
};
