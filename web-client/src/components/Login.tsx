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
        className="loading_spinner_container"
        style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            padding: "40px",
            textAlign: "center",
        }}
        role="status"
        aria-live="polite"
        aria-label={message}
    >
        <div
            className="loading_spinner"
            style={{
                width: "40px",
                height: "40px",
                border: "3px solid #f3f3f3",
                borderTop: "3px solid #4dabf7",
                borderRadius: "50%",
                animation: "spin 1s linear infinite",
                marginBottom: "16px",
            }}
            aria-hidden="true"
        />
        <div style={{ color: "#666", fontSize: "14px" }}>
            {message}
        </div>
        <style>
            {`
                @keyframes spin {
                    0% { transform: rotate(0deg); }
                    100% { transform: rotate(360deg); }
                }
            `}
        </style>
    </div>
);

interface LoginProps {
    visible: boolean;
    welcomeMessage: string;
    contentType: "text/plain" | "text/djot" | "text/html" | "text/traceback";
    isServerReady: boolean;
    onConnect: (mode: "connect" | "create", username: string, password: string) => void;
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
    const [contentType, setContentType] = useState<"text/plain" | "text/djot" | "text/html" | "text/traceback">(
        "text/plain",
    );
    const [isLoading, setIsLoading] = useState<boolean>(true);
    const [isServerReady, setIsServerReady] = useState<boolean>(false);

    useEffect(() => {
        let timeoutId: number;
        let isComponentMounted = true;

        const fetchWelcome = async (): Promise<boolean> => {
            try {
                // Import FlatBuffer function
                const { getSystemPropertyFlatBuffer } = await import("../lib/rpc-fb");

                // Fetch welcome message using FlatBuffer protocol
                const welcomeValue = await getSystemPropertyFlatBuffer(["login"], "welcome_message");

                if (welcomeValue === null) {
                    // Server not ready yet, return false to retry
                    console.log("Server not ready, retrying...");
                    return false;
                }

                let welcomeText = "";
                if (Array.isArray(welcomeValue)) {
                    welcomeText = welcomeValue.join("\n");
                } else if (typeof welcomeValue === "string") {
                    welcomeText = welcomeValue;
                } else {
                    console.warn("Unexpected welcome message format:", welcomeValue);
                    welcomeText = "Welcome to mooR";
                }

                // Fetch content type using FlatBuffer protocol
                let contentTypeValue: "text/plain" | "text/djot" | "text/html" | "text/traceback" = "text/plain";
                try {
                    const typeValue = await getSystemPropertyFlatBuffer(["login"], "welcome_message_content_type");
                    if (typeof typeValue === "string") {
                        // Validate the content type
                        if (
                            typeValue === "text/html" || typeValue === "text/djot" || typeValue === "text/plain"
                            || typeValue === "text/traceback"
                        ) {
                            contentTypeValue = typeValue;
                        }
                    }
                    // If 404 or invalid value, default to text/plain (already set)
                } catch (error) {
                    console.log("Content type not available, defaulting to text/plain:", error);
                }

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
    const [oauth2Mode, setOAuth2Mode] = useState<"create" | "link">("create");
    const [oauth2PlayerName, setOAuth2PlayerName] = useState("");
    const [oauth2ExistingUsername, setOAuth2ExistingUsername] = useState("");
    const [oauth2ExistingPassword, setOAuth2ExistingPassword] = useState("");
    const [oauth2IsSubmitting, setOAuth2IsSubmitting] = useState(false);
    const { authState } = useAuthContext();

    const usernameRef = useRef<HTMLInputElement>(null);
    const passwordRef = useRef<HTMLInputElement>(null);

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

        // For create mode, validate password confirmation
        if (mode === "create" && password !== confirmPassword) {
            // Show error or focus confirm password field
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

    // Clear confirm password when switching to connect mode
    useEffect(() => {
        if (mode === "connect") {
            setConfirmPassword("");
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
        <div className="login_window" style={{ display: "block" }}>
            {/* Welcome message display */}
            <div className="welcome_box" role="banner" aria-label="Welcome message">
                <ContentRenderer content={welcomeMessage} contentType={contentType} />
            </div>

            {/* Modern login card */}
            <div className="login_card">
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

                {/* Divider */}
                <div className="login_divider">
                    <span>{mode === "connect" ? "or continue with username" : "or create with username"}</span>
                </div>

                {/* Login form */}
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
                            placeholder={mode === "connect" ? "Enter your player name" : "Enter your new player name"}
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
                                aria-invalid={password !== confirmPassword && confirmPassword !== "" ? "true" : "false"}
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
                        {authState.isConnecting ? "Connecting..." : mode === "connect" ? "Sign In" : "Create Account"}
                    </button>
                </form>
            </div>
        </div>
    );
};
