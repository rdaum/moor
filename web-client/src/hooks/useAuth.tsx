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

import { useCallback, useState } from "react";

export interface Player {
    oid: string;
    authToken: string;
    connected: boolean;
}

export interface AuthState {
    player: Player | null;
    isConnecting: boolean;
    error: string | null;
}

export const useAuth = (onSystemMessage: (message: string, duration?: number) => void) => {
    const [authState, setAuthState] = useState<AuthState>({
        player: null,
        isConnecting: false,
        error: null,
    });

    const connect = useCallback(async (
        mode: "connect" | "create",
        username: string,
        password: string,
    ) => {
        try {
            setAuthState(prev => ({ ...prev, isConnecting: true, error: null }));

            // Validate inputs
            if (!username.trim()) {
                onSystemMessage("Please enter a username", 3);
                return;
            }

            if (!password) {
                onSystemMessage("Please enter a password", 3);
                return;
            }

            // Build authentication request
            const url = `/auth/${mode}`;
            const data = new URLSearchParams();
            data.set("player", username.trim());
            data.set("password", password);

            // Show connecting status
            onSystemMessage("Connecting to server...", 2);

            // Send authentication request
            const result = await fetch(url, {
                method: "POST",
                body: data,
            });

            // Handle HTTP errors
            if (!result.ok) {
                const errorMessage = result.status === 401
                    ? "Invalid username or password"
                    : `Failed to connect (${result.status}: ${result.statusText})`;

                console.error(`Authentication failed: ${result.status}`, result);
                onSystemMessage(errorMessage, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error: errorMessage }));
                return;
            }

            // Parse authentication response
            const loginResult = await result.text();
            const loginComponents = loginResult.split(" ");
            const playerOid = loginComponents[0];
            const authToken = result.headers.get("X-Moor-Auth-Token");

            // Validate authentication token
            if (!authToken) {
                const error = "Authentication failed: No token received";
                console.error(error);
                onSystemMessage(error, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error }));
                return;
            }

            // Update player state (authorized but not yet connected)
            const player: Player = {
                oid: playerOid,
                authToken,
                connected: false,
            };

            setAuthState({
                player,
                isConnecting: false,
                error: null,
            });

            onSystemMessage("Authenticated! Loading history...", 2);

            // TODO: Fetch and display historical events and current presentations
            // WebSocket connection will be handled by useWebSocket hook
        } catch (error) {
            const errorMessage = `Connection error: ${error instanceof Error ? error.message : "Unknown error"}`;
            console.error("Connection error:", error);
            onSystemMessage(errorMessage, 5);
            setAuthState(prev => ({
                ...prev,
                isConnecting: false,
                error: errorMessage,
            }));
        }
    }, [onSystemMessage]);

    const disconnect = useCallback(() => {
        setAuthState({
            player: null,
            isConnecting: false,
            error: null,
        });
        onSystemMessage("Disconnected", 2);
    }, [onSystemMessage]);

    const setPlayerConnected = useCallback((connected: boolean) => {
        setAuthState(prev => ({
            ...prev,
            player: prev.player ? { ...prev.player, connected } : null,
        }));
    }, []);

    return {
        authState,
        connect,
        disconnect,
        setPlayerConnected,
    };
};
