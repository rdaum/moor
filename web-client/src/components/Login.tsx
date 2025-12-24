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

import React, { useEffect, useRef, useState } from "react";
import { useAuthContext } from "../context/AuthContext";
import { OAuth2UserInfo } from "../lib/oauth2";
import { ContentRenderer } from "./ContentRenderer";
import { OAuth2Buttons } from "./OAuth2Buttons";

/**
 * Simple loading spinner component for server startup
 */
const LoadingSpinner: React.FC<{ message?: string }> = ({ message = "Connecting to server..." }) => (
    <div
        className="loading-spinner-container"
        role="status"
        aria-live="polite"
        aria-label={message}
    >
        <div className="loading-spinner" aria-hidden="true" />
        <div className="loading-spinner-message">
            {message}
        </div>
    </div>
);

interface LoginProps {
    visible: boolean;
    welcomeMessage: string;
    contentType: "text/plain" | "text/djot" | "text/html" | "text/traceback" | "text/x-uri";
    isServerReady: boolean;
    eventLogEnabled?: boolean | null;
    onConnect: (mode: "connect" | "create", username: string, password: string, encryptPassword?: string) => void;
    oauth2UserInfo?: OAuth2UserInfo | null;
    onOAuth2AccountChoice?: (choice: {
        mode: "oauth2_create" | "oauth2_connect";
        provider: string;
        external_id: string;
        email?: string;
        name?: string;
        username?: string;
        player_name?: string;
        existing_email?: string;
        existing_password?: string;
    }) => Promise<void>;
    onOAuth2Cancel?: () => void;
}

/**
 * Hook to fetch welcome message and content type from the server
 */
export const useWelcomeMessage = () => {
    const [welcomeMessage, setWelcomeMessage] = useState<string>("");
    const [contentType, setContentType] = useState<
        "text/plain" | "text/djot" | "text/html" | "text/traceback" | "text/x-uri"
    >("text/plain");
    const [isLoading, setIsLoading] = useState<boolean>(true);
    const [isServerReady, setIsServerReady] = useState<boolean>(false);

    useEffect(() => {
        let timeoutId: number;
        let isComponentMounted = true;

        const fetchWelcome = async (): Promise<boolean> => {
            try {
                // Import FlatBuffer function
                const { invokeWelcomeMessageFlatBuffer } = await import("../lib/rpc-fb");

                // Invoke welcome message system verb using FlatBuffer protocol
                const { welcomeMessage: welcomeText, contentType: contentTypeValue } =
                    await invokeWelcomeMessageFlatBuffer();

                if (isComponentMounted) {
                    setWelcomeMessage(welcomeText);
                    setContentType(contentTypeValue);
                    setIsServerReady(true);
                    setIsLoading(false);
                }
                return true;
            } catch (error) {
                console.log("Error fetching welcome message, retrying:", error);
                return false;
            }
        };

        const pollForWelcome = async () => {
            const success = await fetchWelcome();

            if (!success && isComponentMounted) {
                // Retry after 2 seconds
                timeoutId = window.setTimeout(pollForWelcome, 2000);
            }
        };

        // Start polling
        pollForWelcome();

        return () => {
            isComponentMounted = false;
            if (timeoutId) {
                window.clearTimeout(timeoutId);
            }
        };
    }, []);

    return { welcomeMessage, contentType, isLoading, isServerReady };
};

/**
 * Login Component
 *
 * Renders a login form that allows users to either connect to an existing
 * account or create a new one. The component automatically hides when
 * the user is connected and shows when disconnected.
 */
export const Login: React.FC<LoginProps> = (
    {
        visible,
        welcomeMessage,
        contentType,
        isServerReady,
        eventLogEnabled,
        onConnect,
        oauth2UserInfo,
        onOAuth2AccountChoice,
        onOAuth2Cancel,
    },
) => {
    const [mode, setMode] = useState<"connect" | "create">("connect");
    const [username, setUsername] = useState("");
    const [password, setPassword] = useState("");
    const [confirmPassword, setConfirmPassword] = useState("");
    // Create wizard state: credentials -> encryption-info -> encryption-password
    const [createStep, setCreateStep] = useState<"credentials" | "encryption-info" | "encryption-password">(
        "credentials",
    );
    const [useSeparateEncryptPassword, setUseSeparateEncryptPassword] = useState(false);
    const [encryptPassword, setEncryptPassword] = useState("");
    const [confirmEncryptPassword, setConfirmEncryptPassword] = useState("");
    const [understoodEncryption, setUnderstoodEncryption] = useState(false);
    // OAuth2 state
    const [oauth2Mode, setOAuth2Mode] = useState<"create" | "link">("create");
    const [oauth2PlayerName, setOAuth2PlayerName] = useState("");
    const [oauth2ExistingUsername, setOAuth2ExistingUsername] = useState("");
    const [oauth2ExistingPassword, setOAuth2ExistingPassword] = useState("");
    const [oauth2IsSubmitting, setOAuth2IsSubmitting] = useState(false);
    const [showHelp, setShowHelp] = useState(false);
    const [helpMessage, setHelpMessage] = useState<string>("");
    const [helpContentType, setHelpContentType] = useState<"text/plain" | "text/djot" | "text/html" | "text/traceback">(
        "text/plain",
    );
    const { authState } = useAuthContext();

    const usernameRef = useRef<HTMLInputElement>(null);
    const passwordRef = useRef<HTMLInputElement>(null);

    // Proceed to encryption info step in create wizard (or skip if event log disabled)
    const handleNextToEncryptionInfo = (e?: React.FormEvent) => {
        e?.preventDefault();

        if (!username.trim()) {
            usernameRef.current?.focus();
            return;
        }

        if (!password) {
            passwordRef.current?.focus();
            return;
        }

        if (password !== confirmPassword) {
            return;
        }

        // Skip encryption wizard if event log is disabled
        if (eventLogEnabled === false) {
            onConnect("create", username.trim(), password);
            return;
        }

        setCreateStep("encryption-info");
    };

    // Final submit for create mode (from encryption-password step)
    const handleCreateSubmit = (e?: React.FormEvent) => {
        e?.preventDefault();

        // Must acknowledge understanding
        if (!understoodEncryption) {
            return;
        }

        // If using separate encryption password, validate it
        if (useSeparateEncryptPassword) {
            if (!encryptPassword) {
                return;
            }
            if (encryptPassword !== confirmEncryptPassword) {
                return;
            }
        }

        // Use encryption password if set, otherwise use account password
        const effectiveEncryptPassword = useSeparateEncryptPassword ? encryptPassword : password;
        onConnect(mode, username.trim(), password, effectiveEncryptPassword);
    };

    const handleSubmit = (e?: React.FormEvent) => {
        e?.preventDefault();

        // Validate inputs with accessibility announcements
        if (!username.trim()) {
            usernameRef.current?.focus();
            // Screen readers will announce the required field when focused
            return;
        }

        if (!password) {
            passwordRef.current?.focus();
            // Screen readers will announce the required field when focused
            return;
        }

        // For create mode, this shouldn't be called directly - use wizard
        if (mode === "create") {
            handleNextToEncryptionInfo(e);
            return;
        }

        onConnect(mode, username.trim(), password);
    };

    const handleKeyPress = (e: React.KeyboardEvent) => {
        if (e.key === "Enter") {
            e.preventDefault();
            handleSubmit();
        }
    };

    const handleOAuth2Submit = async (e?: React.FormEvent) => {
        e?.preventDefault();

        if (!oauth2UserInfo || !onOAuth2AccountChoice) return;

        setOAuth2IsSubmitting(true);

        try {
            if (oauth2Mode === "create") {
                if (!oauth2PlayerName.trim()) {
                    return;
                }
                await onOAuth2AccountChoice({
                    mode: "oauth2_create",
                    provider: oauth2UserInfo.provider,
                    external_id: oauth2UserInfo.external_id,
                    email: oauth2UserInfo.email,
                    name: oauth2UserInfo.name,
                    username: oauth2UserInfo.username,
                    player_name: oauth2PlayerName.trim(),
                });
            } else {
                if (!oauth2ExistingUsername.trim() || !oauth2ExistingPassword) {
                    return;
                }
                await onOAuth2AccountChoice({
                    mode: "oauth2_connect",
                    provider: oauth2UserInfo.provider,
                    external_id: oauth2UserInfo.external_id,
                    email: oauth2UserInfo.email,
                    name: oauth2UserInfo.name,
                    username: oauth2UserInfo.username,
                    existing_email: oauth2ExistingUsername.trim(),
                    existing_password: oauth2ExistingPassword,
                });
            }
        } finally {
            setOAuth2IsSubmitting(false);
        }
    };

    // Auto-focus first field when login becomes visible and clear previous errors
    useEffect(() => {
        if (visible && !authState.isConnecting) {
            // Small delay to ensure the component is fully rendered
            const timer = setTimeout(() => {
                usernameRef.current?.focus();
            }, 100);
            return () => clearTimeout(timer);
        }
    }, [visible, authState.isConnecting]);

    // Reset create wizard state when switching modes
    useEffect(() => {
        if (mode === "connect") {
            setConfirmPassword("");
            setCreateStep("credentials");
            setUseSeparateEncryptPassword(false);
            setEncryptPassword("");
            setConfirmEncryptPassword("");
            setUnderstoodEncryption(false);
        }
    }, [mode]);

    // Pre-populate OAuth2 player name with name (display name) or username from provider
    useEffect(() => {
        if (oauth2UserInfo && !oauth2PlayerName) {
            // Prefer name (which includes Discord's global_name), fall back to username
            const suggestedName = oauth2UserInfo.name || oauth2UserInfo.username;
            if (suggestedName) {
                setOAuth2PlayerName(suggestedName);
            }
        }
    }, [oauth2UserInfo, oauth2PlayerName]);

    // Fetch help message when component becomes visible
    useEffect(() => {
        if (!visible || !isServerReady) return;

        const fetchHelpMessage = async () => {
            try {
                const { getSystemPropertyFlatBuffer } = await import("../lib/rpc-fb");

                // Fetch help message
                const helpValue = await getSystemPropertyFlatBuffer(["login"], "help_message");
                if (helpValue !== null) {
                    let helpText = "";
                    if (Array.isArray(helpValue)) {
                        helpText = helpValue.join("\n");
                    } else if (typeof helpValue === "string") {
                        helpText = helpValue;
                    }
                    setHelpMessage(helpText);
                }

                // Fetch help message content type
                try {
                    const typeValue = await getSystemPropertyFlatBuffer(["login"], "help_message_content_type");
                    if (typeof typeValue === "string") {
                        if (
                            typeValue === "text/html" || typeValue === "text/djot" || typeValue === "text/plain"
                            || typeValue === "text/traceback"
                        ) {
                            setHelpContentType(typeValue);
                        }
                    }
                } catch (error) {
                    console.log("Help message content type not available, defaulting to text/plain:", error);
                }
            } catch (error) {
                console.error("Error fetching help message:", error);
            }
        };

        fetchHelpMessage();
    }, [visible, isServerReady]);

    if (!visible) {
        return null;
    }

    // Show loading spinner if server is not ready
    if (!isServerReady) {
        return (
            <div className="login_window" style={{ display: "block" }}>
                <LoadingSpinner message="Starting server, please wait..." />
            </div>
        );
    }

    // Show OAuth2 account choice form if we have user info
    if (oauth2UserInfo && onOAuth2AccountChoice) {
        return (
            <div className="login_window" style={{ display: "block" }}>
                <div className="welcome_box" role="banner" aria-label="Welcome message">
                    <ContentRenderer content={welcomeMessage} contentType={contentType} />
                </div>

                <div className="login_card">
                    <div className="oauth2_complete_header">
                        <h2 className="oauth2_complete_title">
                            {oauth2UserInfo.name ? `Welcome, ${oauth2UserInfo.name}!` : "Welcome!"}
                        </h2>
                        <p className="oauth2_complete_subtitle">
                            You've authenticated with{" "}
                            {oauth2UserInfo.provider.charAt(0).toUpperCase() + oauth2UserInfo.provider.slice(1)}.
                        </p>
                        <p className="oauth2_complete_subtitle">
                            This {oauth2UserInfo.provider.charAt(0).toUpperCase() + oauth2UserInfo.provider.slice(1)}
                            {" "}
                            account isn't linked to any existing player yet. You can create a new player or link it to
                            an existing one.
                        </p>
                    </div>

                    <div className="login_tabs" role="tablist" aria-label="Account option">
                        <button
                            type="button"
                            role="tab"
                            className={`login_tab ${oauth2Mode === "create" ? "active" : ""}`}
                            onClick={() => setOAuth2Mode("create")}
                            disabled={oauth2IsSubmitting}
                            aria-selected={oauth2Mode === "create"}
                            aria-controls="oauth2-form-panel"
                        >
                            Create New Account
                        </button>
                        <button
                            type="button"
                            role="tab"
                            className={`login_tab ${oauth2Mode === "link" ? "active" : ""}`}
                            onClick={() => setOAuth2Mode("link")}
                            disabled={oauth2IsSubmitting}
                            aria-selected={oauth2Mode === "link"}
                            aria-controls="oauth2-form-panel"
                        >
                            Link to Existing
                        </button>
                    </div>

                    <form
                        id="oauth2-form-panel"
                        className="login_form oauth2_complete_form"
                        onSubmit={handleOAuth2Submit}
                        noValidate
                        role="tabpanel"
                        aria-labelledby={oauth2Mode === "create" ? "tab-create" : "tab-link"}
                    >
                        {oauth2Mode === "create"
                            ? (
                                <div className="login_field">
                                    <label htmlFor="oauth2_player_name" className="login_field_label">
                                        Choose Your Player Name
                                    </label>
                                    <input
                                        id="oauth2_player_name"
                                        type="text"
                                        className="login_input"
                                        placeholder="Enter desired player name"
                                        value={oauth2PlayerName}
                                        onChange={(e) => setOAuth2PlayerName(e.target.value)}
                                        disabled={oauth2IsSubmitting}
                                        required
                                        autoFocus
                                    />
                                </div>
                            )
                            : (
                                <>
                                    <div className="login_field">
                                        <label htmlFor="oauth2_existing_username" className="login_field_label">
                                            Player Name
                                        </label>
                                        <input
                                            id="oauth2_existing_username"
                                            type="text"
                                            className="login_input"
                                            placeholder="Your existing player name"
                                            value={oauth2ExistingUsername}
                                            onChange={(e) => setOAuth2ExistingUsername(e.target.value)}
                                            disabled={oauth2IsSubmitting}
                                            required
                                            autoFocus
                                        />
                                    </div>
                                    <div className="login_field">
                                        <label htmlFor="oauth2_existing_password" className="login_field_label">
                                            Password
                                        </label>
                                        <input
                                            id="oauth2_existing_password"
                                            type="password"
                                            className="login_input"
                                            placeholder="Your existing password"
                                            value={oauth2ExistingPassword}
                                            onChange={(e) => setOAuth2ExistingPassword(e.target.value)}
                                            disabled={oauth2IsSubmitting}
                                            required
                                        />
                                    </div>
                                </>
                            )}

                        <button
                            type="submit"
                            className="login_submit_button"
                            disabled={oauth2IsSubmitting}
                        >
                            {oauth2IsSubmitting ? "Processing..." : "Continue"}
                        </button>

                        <button
                            type="button"
                            className="login_cancel_button"
                            onClick={() => {
                                setOAuth2PlayerName("");
                                setOAuth2ExistingUsername("");
                                setOAuth2ExistingPassword("");
                                setOAuth2Mode("create");
                                onOAuth2Cancel?.();
                            }}
                            disabled={oauth2IsSubmitting}
                        >
                            Cancel
                        </button>
                    </form>
                </div>
            </div>
        );
    }

    return (
        <div className="login_window" style={{ display: "block", position: "relative" }}>
            {/* Loading overlay with spinner - shown during authentication */}
            {authState.isConnecting && (
                <div
                    className="login-auth-overlay"
                    role="status"
                    aria-live="assertive"
                    aria-label="Authenticating, please wait"
                >
                    <div className="login-auth-spinner-card">
                        <div className="loading-spinner large" aria-hidden="true" />
                        <div className="login-auth-spinner-title">
                            Connecting to server...
                        </div>
                        <div className="login-auth-spinner-subtitle">
                            Please wait
                        </div>
                    </div>
                </div>
            )}

            {/* Welcome message display */}
            <div
                className="welcome_box"
                role="banner"
                aria-label="Welcome message"
                style={{
                    opacity: authState.isConnecting ? 0.5 : 1,
                    transition: "opacity 0.3s ease",
                    pointerEvents: authState.isConnecting ? "none" : "auto",
                }}
            >
                <div style={{ position: "relative" }}>
                    {helpMessage && (
                        <button
                            type="button"
                            onClick={() => setShowHelp(true)}
                            className="help_button"
                            aria-label="Show help"
                            title="Help"
                            disabled={authState.isConnecting}
                        >
                            ?
                        </button>
                    )}
                    <ContentRenderer content={welcomeMessage} contentType={contentType} />
                </div>
            </div>

            {/* Modern login card */}
            <div
                className="login_card"
                style={{
                    opacity: authState.isConnecting ? 0.5 : 1,
                    transition: "opacity 0.3s ease",
                }}
            >
                {/* Tab switcher for connect/create */}
                <div className="login_tabs" role="tablist" aria-label="Authentication mode">
                    <button
                        type="button"
                        role="tab"
                        className={`login_tab ${mode === "connect" ? "active" : ""}`}
                        onClick={() => setMode("connect")}
                        disabled={authState.isConnecting}
                        aria-selected={mode === "connect"}
                        aria-controls="login-form-panel"
                    >
                        Sign In
                    </button>
                    <button
                        type="button"
                        role="tab"
                        className={`login_tab ${mode === "create" ? "active" : ""}`}
                        onClick={() => setMode("create")}
                        disabled={authState.isConnecting}
                        aria-selected={mode === "create"}
                        aria-controls="login-form-panel"
                    >
                        Create Account
                    </button>
                </div>

                {/* Error/status messages */}
                {authState.error && (
                    <div
                        id="login-error"
                        className="login_error"
                        role="alert"
                        aria-live="assertive"
                        aria-atomic="true"
                    >
                        {authState.error}
                    </div>
                )}
                {authState.isConnecting && (
                    <div
                        id="login-status"
                        className="login_status"
                        role="status"
                        aria-live="polite"
                        aria-atomic="true"
                    >
                        Connecting to server, please wait...
                    </div>
                )}

                {/* OAuth2 login options at top */}
                <div className="login_oauth_section">
                    <OAuth2Buttons disabled={authState.isConnecting} mode={mode} />
                </div>

                {/* Divider - hide during encryption wizard steps */}
                {!(mode === "create" && createStep !== "credentials") && (
                    <div className="login_divider">
                        <span>{mode === "connect" ? "or continue with username" : "or create with username"}</span>
                    </div>
                )}

                {/* Connect form OR Create wizard step 1 (credentials) */}
                {(mode === "connect" || (mode === "create" && createStep === "credentials")) && (
                    <form
                        id="login-form-panel"
                        className="login_form"
                        onSubmit={handleSubmit}
                        noValidate
                        role="tabpanel"
                        aria-labelledby={mode === "connect" ? "tab-connect" : "tab-create"}
                    >
                        <div className="login_field">
                            <label htmlFor="login_username" className="login_field_label">
                                Player Name
                            </label>
                            <input
                                ref={usernameRef}
                                id="login_username"
                                name="username"
                                type="text"
                                className="login_input"
                                placeholder={mode === "connect"
                                    ? "Enter your player name"
                                    : "Enter your new player name"}
                                autoComplete="username"
                                spellCheck={false}
                                value={username}
                                onChange={(e) => setUsername(e.target.value)}
                                onKeyUp={handleKeyPress}
                                disabled={authState.isConnecting}
                                required
                                aria-invalid={authState.error ? "true" : "false"}
                                aria-describedby={authState.error ? "login-error" : undefined}
                            />
                        </div>

                        <div className="login_field">
                            <label htmlFor="login_password" className="login_field_label">
                                Password
                            </label>
                            <input
                                ref={passwordRef}
                                id="login_password"
                                name="password"
                                type="password"
                                className="login_input"
                                placeholder={mode === "connect" ? "Enter your password" : "Choose a password"}
                                autoComplete={mode === "connect" ? "current-password" : "new-password"}
                                value={password}
                                onChange={(e) => setPassword(e.target.value)}
                                onKeyUp={handleKeyPress}
                                disabled={authState.isConnecting}
                                required
                                aria-invalid={authState.error ? "true" : "false"}
                                aria-describedby={authState.error ? "login-error" : undefined}
                            />
                        </div>

                        {mode === "create" && (
                            <div className="login_field">
                                <label htmlFor="login_confirm_password" className="login_field_label">
                                    Confirm Password
                                </label>
                                <input
                                    id="login_confirm_password"
                                    name="confirm_password"
                                    type="password"
                                    className="login_input"
                                    placeholder="Re-enter your password"
                                    autoComplete="new-password"
                                    value={confirmPassword}
                                    onChange={(e) => setConfirmPassword(e.target.value)}
                                    onKeyUp={handleKeyPress}
                                    disabled={authState.isConnecting}
                                    required
                                    aria-invalid={password !== confirmPassword && confirmPassword !== ""
                                        ? "true"
                                        : "false"}
                                />
                                {password !== confirmPassword && confirmPassword !== "" && (
                                    <span className="login_field_error">Passwords do not match</span>
                                )}
                            </div>
                        )}

                        <button
                            type="submit"
                            className="login_submit_button"
                            disabled={authState.isConnecting}
                            aria-describedby={authState.isConnecting
                                ? "login-status"
                                : authState.error
                                ? "login-error"
                                : undefined}
                        >
                            {authState.isConnecting ? "Connecting..." : mode === "connect" ? "Sign In" : "Next"}
                        </button>
                    </form>
                )}

                {/* Create wizard step 2: Encryption explanation */}
                {mode === "create" && createStep === "encryption-info" && (
                    <div className="wizard_step">
                        <div className="wizard_step_indicator">Step 2 of 3</div>
                        <h3 className="wizard_step_title">Your History</h3>

                        <div className="wizard_step_content">
                            <p>
                                Your <strong>history</strong>{" "}
                                is a complete record of everything you see and do—conversations, actions, descriptions,
                                and more.
                            </p>
                            <p>
                                It's stored encrypted so only you can read it. With your encryption password, your
                                history follows you across devices and persists over time.
                            </p>
                        </div>

                        <div className="login_button_row">
                            <button
                                type="button"
                                className="login_back_button"
                                onClick={() => setCreateStep("credentials")}
                            >
                                Back
                            </button>
                            <button
                                type="button"
                                className="login_submit_button"
                                onClick={() => setCreateStep("encryption-password")}
                            >
                                Continue
                            </button>
                        </div>
                    </div>
                )}

                {/* Create wizard step 3: Encryption password choice */}
                {mode === "create" && createStep === "encryption-password" && (
                    <form
                        id="encryption-form-panel"
                        className="wizard_step"
                        onSubmit={handleCreateSubmit}
                        noValidate
                    >
                        <div className="wizard_step_indicator">Step 3 of 3</div>
                        <h3 className="wizard_step_title">Encryption Password</h3>

                        <div className="wizard_step_content">
                            <p>
                                By default, your current account password will be used to encrypt your history. If you
                                change your account password later, your encryption password stays the same.
                            </p>
                        </div>

                        <div className="login_field">
                            <label className="login_checkbox_label">
                                <input
                                    type="checkbox"
                                    checked={useSeparateEncryptPassword}
                                    onChange={(e) => {
                                        setUseSeparateEncryptPassword(e.target.checked);
                                        if (!e.target.checked) {
                                            setEncryptPassword("");
                                            setConfirmEncryptPassword("");
                                        }
                                    }}
                                    disabled={authState.isConnecting}
                                />
                                <span>Use a different password for encryption</span>
                            </label>
                        </div>

                        {useSeparateEncryptPassword && (
                            <>
                                <div className="login_field">
                                    <label htmlFor="encrypt_password" className="login_field_label">
                                        Encryption Password
                                    </label>
                                    <input
                                        id="encrypt_password"
                                        type="password"
                                        className="login_input"
                                        placeholder="Choose an encryption password"
                                        autoComplete="new-password"
                                        value={encryptPassword}
                                        onChange={(e) => setEncryptPassword(e.target.value)}
                                        disabled={authState.isConnecting}
                                        required
                                    />
                                </div>

                                <div className="login_field">
                                    <label htmlFor="confirm_encrypt_password" className="login_field_label">
                                        Confirm Encryption Password
                                    </label>
                                    <input
                                        id="confirm_encrypt_password"
                                        type="password"
                                        className="login_input"
                                        placeholder="Re-enter encryption password"
                                        autoComplete="new-password"
                                        value={confirmEncryptPassword}
                                        onChange={(e) => setConfirmEncryptPassword(e.target.value)}
                                        disabled={authState.isConnecting}
                                        required
                                        aria-invalid={encryptPassword !== confirmEncryptPassword
                                                && confirmEncryptPassword !== ""
                                            ? "true"
                                            : "false"}
                                    />
                                    {encryptPassword !== confirmEncryptPassword && confirmEncryptPassword !== "" && (
                                        <span className="login_field_error">Passwords do not match</span>
                                    )}
                                </div>
                            </>
                        )}

                        <div className="wizard_warning">
                            <strong>Important:</strong>{" "}
                            If you lose your encryption password, you lose access to your history permanently. There is
                            no recovery. You can change your account password later, but not your encryption password.
                        </div>

                        <div className="login_field">
                            <label className="login_checkbox_label">
                                <input
                                    type="checkbox"
                                    checked={understoodEncryption}
                                    onChange={(e) => setUnderstoodEncryption(e.target.checked)}
                                    disabled={authState.isConnecting}
                                    required
                                />
                                <span>I understand this password cannot be recovered</span>
                            </label>
                        </div>

                        <div className="login_button_row">
                            <button
                                type="button"
                                className="login_back_button"
                                onClick={() => setCreateStep("encryption-info")}
                                disabled={authState.isConnecting}
                            >
                                Back
                            </button>
                            <button
                                type="submit"
                                className="login_submit_button"
                                disabled={authState.isConnecting || !understoodEncryption}
                            >
                                {authState.isConnecting ? "Creating..." : "Create Account"}
                            </button>
                        </div>
                    </form>
                )}
            </div>

            {/* Help modal */}
            {showHelp && helpMessage && (
                <div
                    className="help_modal_backdrop"
                    onClick={() => setShowHelp(false)}
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="help-modal-title"
                >
                    <div
                        className="help_modal"
                        onClick={(e) => e.stopPropagation()}
                    >
                        <div className="help_modal_header">
                            <h2 id="help-modal-title" className="help_modal_title">Help</h2>
                            <button
                                type="button"
                                className="help_modal_close"
                                onClick={() => setShowHelp(false)}
                                aria-label="Close help"
                            >
                                ×
                            </button>
                        </div>
                        <div className="help_modal_content">
                            <ContentRenderer content={helpMessage} contentType={helpContentType} />
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};
