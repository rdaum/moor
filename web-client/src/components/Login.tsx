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

interface LoginProps {
    visible: boolean;
    welcomeMessage: string;
    onConnect: (mode: "connect" | "create", username: string, password: string) => void;
}

/**
 * Hook to fetch welcome message from the server
 */
export const useWelcomeMessage = () => {
    const [welcomeMessage, setWelcomeMessage] = useState<string>("");

    useEffect(() => {
        const fetchWelcome = async () => {
            try {
                const response = await fetch("/system_property/login/welcome_message");
                if (response.ok) {
                    const welcomeText = await response.json() as string[];
                    setWelcomeMessage(welcomeText.join("\n"));
                } else {
                    console.error(`Failed to retrieve welcome text: ${response.status} ${response.statusText}`);
                    setWelcomeMessage("Welcome to mooR");
                }
            } catch (error) {
                console.error("Error fetching welcome message:", error);
                setWelcomeMessage("Welcome to mooR");
            }
        };

        fetchWelcome();
    }, []);

    return welcomeMessage;
};

/**
 * Simple Djot-like content renderer (placeholder for now)
 * TODO: Integrate actual @djot/djot library
 */
const DjotContent: React.FC<{ content: string }> = ({ content }) => {
    return (
        <div className="welcome_box">
            <pre style={{ whiteSpace: "pre-wrap", fontFamily: "inherit" }}>
        {content}
            </pre>
        </div>
    );
};

/**
 * Login Component
 *
 * Renders a login form that allows users to either connect to an existing
 * account or create a new one. The component automatically hides when
 * the user is connected and shows when disconnected.
 */
export const Login: React.FC<LoginProps> = ({ visible, welcomeMessage, onConnect }) => {
    const [mode, setMode] = useState<"connect" | "create">("connect");
    const [username, setUsername] = useState("");
    const [password, setPassword] = useState("");

    const usernameRef = useRef<HTMLInputElement>(null);
    const passwordRef = useRef<HTMLInputElement>(null);

    const handleSubmit = (e?: React.FormEvent) => {
        e?.preventDefault();

        // Validate inputs
        if (!username.trim()) {
            usernameRef.current?.focus();
            return;
        }

        if (!password) {
            passwordRef.current?.focus();
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

    if (!visible) {
        return null;
    }

    return (
        <div className="login_window" style={{ display: "block" }}>
            {/* Welcome message display */}
            <DjotContent content={welcomeMessage} />
            <br />

            {/* Login form */}
            <div className="login_prompt">
                <fieldset>
                    <legend>Player Authentication</legend>
                    <label htmlFor="mode_select" className="sr-only">Connection type:</label>
                    <select
                        id="mode_select"
                        value={mode}
                        onChange={(e) => setMode(e.target.value as "connect" | "create")}
                    >
                        <option value="connect">Connect</option>
                        <option value="create">Create</option>
                    </select>{" "}
                    <label htmlFor="login_username" className="login_label">
                        Player:{" "}
                        <input
                            ref={usernameRef}
                            id="login_username"
                            type="text"
                            placeholder="Username"
                            autoComplete="username"
                            spellCheck={false}
                            value={username}
                            onChange={(e) => setUsername(e.target.value)}
                            onKeyUp={handleKeyPress}
                        />
                    </label>{" "}
                    <label htmlFor="login_password" className="login_label">
                        Password:{" "}
                        <input
                            ref={passwordRef}
                            id="login_password"
                            type="password"
                            placeholder="Password"
                            autoComplete="current-password"
                            value={password}
                            onChange={(e) => setPassword(e.target.value)}
                            onKeyUp={handleKeyPress}
                        />
                    </label>{" "}
                    <button
                        onClick={handleSubmit}
                        className="login_button"
                    >
                        Go
                    </button>
                </fieldset>
            </div>
        </div>
    );
};
