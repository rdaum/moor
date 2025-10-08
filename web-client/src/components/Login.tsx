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
import { ContentRenderer } from "./ContentRenderer";

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
export const Login: React.FC<LoginProps> = ({ visible, welcomeMessage, contentType, isServerReady, onConnect }) => {
    const [mode, setMode] = useState<"connect" | "create">("connect");
    const [username, setUsername] = useState("");
    const [password, setPassword] = useState("");
    const { authState } = useAuthContext();

    const usernameRef = useRef<HTMLInputElement>(null);
    const passwordRef = useRef<HTMLInputElement>(null);
    const modeSelectRef = useRef<HTMLSelectElement>(null);

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

        onConnect(mode, username.trim(), password);
    };

    const handleKeyPress = (e: React.KeyboardEvent) => {
        if (e.key === "Enter") {
            e.preventDefault();
            handleSubmit();
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

    return (
        <div className="login_window" style={{ display: "block" }}>
            {/* Welcome message display */}
            <div className="welcome_box" role="banner" aria-label="Welcome message">
                <ContentRenderer content={welcomeMessage} contentType={contentType} />
            </div>
            <br />

            {/* Login form */}
            <div className="login_prompt">
                <form onSubmit={handleSubmit} noValidate role="form" aria-label="Player authentication">
                    <fieldset>
                        <legend>Player Authentication</legend>
                        {authState.error && (
                            <div
                                id="login-error"
                                className="login_error"
                                style={{ color: "#ff6b6b", marginBottom: "10px", fontSize: "14px" }}
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
                                style={{ color: "#4dabf7", marginBottom: "10px", fontSize: "14px" }}
                                role="status"
                                aria-live="polite"
                                aria-atomic="true"
                            >
                                Connecting to server, please wait...
                            </div>
                        )}
                        <label htmlFor="mode_select" className="sr-only">Connection type:</label>
                        <select
                            ref={modeSelectRef}
                            id="mode_select"
                            value={mode}
                            onChange={(e) => setMode(e.target.value as "connect" | "create")}
                            disabled={authState.isConnecting}
                            aria-label="Choose whether to connect to existing account or create new account"
                            aria-describedby={authState.error ? "login-error" : undefined}
                        >
                            <option value="connect">Connect</option>
                            <option value="create">Create</option>
                        </select>{" "}
                        <label htmlFor="login_username" className="login_label">
                            Player:{" "}
                            <input
                                ref={usernameRef}
                                id="login_username"
                                name="username"
                                type="text"
                                placeholder="Enter your username"
                                autoComplete="username"
                                spellCheck={false}
                                value={username}
                                onChange={(e) => setUsername(e.target.value)}
                                onKeyUp={handleKeyPress}
                                disabled={authState.isConnecting}
                                required
                                aria-invalid={authState.error ? "true" : "false"}
                                aria-describedby={authState.error ? "login-error" : undefined}
                                aria-label="Enter your player username"
                            />
                        </label>{" "}
                        <label htmlFor="login_password" className="login_label">
                            Password:{" "}
                            <input
                                ref={passwordRef}
                                id="login_password"
                                name="password"
                                type="password"
                                placeholder="Enter your password"
                                autoComplete="current-password"
                                value={password}
                                onChange={(e) => setPassword(e.target.value)}
                                onKeyUp={handleKeyPress}
                                disabled={authState.isConnecting}
                                required
                                aria-invalid={authState.error ? "true" : "false"}
                                aria-describedby={authState.error ? "login-error" : undefined}
                                aria-label="Enter your password"
                            />
                        </label>{" "}
                        <button
                            type="submit"
                            className="login_button"
                            disabled={authState.isConnecting}
                            aria-describedby={authState.isConnecting
                                ? "login-status"
                                : authState.error
                                ? "login-error"
                                : undefined}
                            aria-label={authState.isConnecting
                                ? "Connecting to server, please wait"
                                : `${mode === "connect" ? "Connect to existing account" : "Create new account"}`}
                        >
                            {authState.isConnecting ? "Connecting..." : "Go"}
                        </button>
                    </fieldset>
                </form>
            </div>
        </div>
    );
};
