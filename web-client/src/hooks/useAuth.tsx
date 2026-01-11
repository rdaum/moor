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

import * as flatbuffers from "flatbuffers";
import { useCallback, useEffect, useState } from "react";
import { ClientSuccess } from "../generated/moor-rpc/client-success";
import { unionToDaemonToClientReplyUnion } from "../generated/moor-rpc/daemon-to-client-reply-union";
import { LoginResult } from "../generated/moor-rpc/login-result";
import { ReplyResult } from "../generated/moor-rpc/reply-result";
import { ReplyResultUnion, unionToReplyResultUnion } from "../generated/moor-rpc/reply-result-union";
import { generateKeypairFromPassword } from "../lib/keyDerivation";
import { objToCurie } from "../lib/var";

export interface Player {
    oid: string;
    authToken: string;
    connected: boolean;
    flags: number;
    clientToken?: string | null;
    clientId?: string | null;
    isInitialAttach?: boolean;
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

    // Check for auth credentials in localStorage on mount and validate them
    useEffect(() => {
        const validateAndRestore = async () => {
            const oauth2Token = localStorage.getItem("oauth2_auth_token");
            const oauth2PlayerOid = localStorage.getItem("oauth2_player_oid");
            const authToken = localStorage.getItem("auth_token");
            const playerOid = localStorage.getItem("player_oid");
            const playerFlags = localStorage.getItem("player_flags");
            // Connection credentials are per-tab (sessionStorage) - may not exist in new tabs
            const storedClientToken = sessionStorage.getItem("client_token");
            const storedClientId = sessionStorage.getItem("client_id");

            // Determine which auth token to validate
            const tokenToValidate = authToken || oauth2Token;
            const oidToRestore = playerOid || oauth2PlayerOid;
            const flagsToRestore = playerFlags || localStorage.getItem("oauth2_player_flags");

            // Must have auth token and player OID (but NOT client credentials - those are per-tab)
            if (!tokenToValidate || !oidToRestore) {
                // No user session - need fresh login
                console.log("No user session - requiring fresh login");
                localStorage.removeItem("auth_token");
                localStorage.removeItem("player_oid");
                localStorage.removeItem("player_flags");
                localStorage.removeItem("oauth2_auth_token");
                localStorage.removeItem("oauth2_player_oid");
                localStorage.removeItem("oauth2_player_flags");
                sessionStorage.removeItem("client_token");
                sessionStorage.removeItem("client_id");
                localStorage.setItem("client_session_active", "false");
                return;
            }

            // Check if event log encryption is set up for this player
            const eventLogEncryptionKey = localStorage.getItem(`moor_event_log_identity_${oidToRestore}`);
            const hasEventLogEncryption = eventLogEncryptionKey !== null;

            // Validate the stored auth token with the server
            // Client credentials are optional - new tabs won't have them
            try {
                const headers: Record<string, string> = {
                    "X-Moor-Auth-Token": tokenToValidate,
                };
                // Only include client credentials if we have them (same tab reload)
                if (storedClientToken && storedClientId) {
                    headers["X-Moor-Client-Token"] = storedClientToken;
                    headers["X-Moor-Client-Id"] = storedClientId;
                }

                const response = await fetch("/auth/validate", {
                    method: "GET",
                    headers,
                });

                if (!response.ok) {
                    // Validation failed - auth token expired or invalid
                    console.log("Auth token validation failed - clearing credentials");
                    localStorage.removeItem("auth_token");
                    localStorage.removeItem("player_oid");
                    localStorage.removeItem("player_flags");
                    localStorage.removeItem("oauth2_auth_token");
                    localStorage.removeItem("oauth2_player_oid");
                    localStorage.removeItem("oauth2_player_flags");
                    sessionStorage.removeItem("client_token");
                    sessionStorage.removeItem("client_id");
                    localStorage.setItem("client_session_active", "false");
                    return;
                }

                // Auth token is valid - restore session
                // Client credentials may be null (new tab) - WebSocket will create new connection
                const flags = flagsToRestore ? parseInt(flagsToRestore, 10) : 0;

                setAuthState({
                    player: {
                        oid: oidToRestore,
                        authToken: tokenToValidate,
                        connected: false,
                        flags,
                        clientToken: storedClientToken ?? undefined,
                        clientId: storedClientId ?? undefined,
                        // New tab (no client credentials) = new connection = trigger :user_connected
                        // Same tab reload with credentials = reattach = no :user_connected
                        // Also: if no event log encryption, treat as initial to trigger :user_connected
                        isInitialAttach: !storedClientToken || !storedClientId || !hasEventLogEncryption,
                    },
                    isConnecting: false,
                    error: null,
                });

                if (oauth2Token) {
                    onSystemMessage(
                        hasEventLogEncryption
                            ? "Authenticated via OAuth2! Loading history..."
                            : "Authenticated via OAuth2!",
                        2,
                    );
                } else {
                    onSystemMessage(
                        hasEventLogEncryption
                            ? "Restoring session..."
                            : "Session restored",
                        2,
                    );
                }
            } catch (error) {
                console.error("Error validating auth token:", error);
                onSystemMessage("Error restoring session", 3);
            }
        };

        validateAndRestore();
    }, [onSystemMessage]);

    const connect = useCallback(async (
        mode: "connect" | "create",
        username: string,
        password: string,
        encryptPassword?: string,
    ) => {
        let generatedIdentity: string | null = null;

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

            // For create mode, generate encryption keypair using username as salt
            // This is done BEFORE the server request so the pubkey can be bundled with account creation
            // Use provided encryption password or fall back to account password
            if (mode === "create") {
                onSystemMessage("Setting up encryption...", 2);
                const effectiveEncryptPassword = encryptPassword || password;
                try {
                    const { identity, publicKey } = await generateKeypairFromPassword(
                        effectiveEncryptPassword,
                        username.trim(),
                    );
                    generatedIdentity = identity;
                    data.set("event_log_pubkey", publicKey);
                    console.log("Generated encryption keypair for new account");
                } catch (keyError) {
                    console.error("Failed to generate encryption keypair:", keyError);
                    // Continue without encryption - user can set it up later
                }
            }

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

            // Parse FlatBuffer response
            const arrayBuffer = await result.arrayBuffer();
            const bytes = new Uint8Array(arrayBuffer);
            const replyResult = ReplyResult.getRootAsReplyResult(
                new flatbuffers.ByteBuffer(bytes),
            );
            const authToken = result.headers.get("X-Moor-Auth-Token");
            const clientToken = result.headers.get("X-Moor-Client-Token");
            const clientId = result.headers.get("X-Moor-Client-Id");

            // Validate authentication token
            if (!authToken) {
                const error = "Authentication failed: No token received";
                console.error(error);
                onSystemMessage(error, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error }));
                return;
            }

            // Store connection credentials for this tab (sessionStorage = per-tab)
            if (clientToken && clientId) {
                sessionStorage.setItem("client_token", clientToken);
                sessionStorage.setItem("client_id", clientId);
            }

            // Extract player info from LoginResult
            const resultType = replyResult.resultType();
            if (resultType !== ReplyResultUnion.ClientSuccess) {
                const error = `Authentication failed: ${ReplyResultUnion[resultType]}`;
                console.error(error);
                onSystemMessage(error, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error }));
                return;
            }

            const clientSuccess = unionToReplyResultUnion(
                resultType,
                (obj) => replyResult.result(obj),
            ) as ClientSuccess | null;

            if (!clientSuccess) {
                const error = "Authentication failed: Failed to parse response";
                console.error(error);
                onSystemMessage(error, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error }));
                return;
            }

            const daemonReply = clientSuccess.reply();
            if (!daemonReply) {
                const error = "Authentication failed: Missing daemon reply";
                console.error(error);
                onSystemMessage(error, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error }));
                return;
            }

            const replyType = daemonReply.replyType();
            const replyUnion = unionToDaemonToClientReplyUnion(
                replyType,
                (obj: any) => daemonReply.reply(obj),
            );

            if (!replyUnion || !(replyUnion instanceof LoginResult)) {
                const error = "Authentication failed: Invalid login result";
                console.error(error);
                onSystemMessage(error, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error }));
                return;
            }

            const loginResult = replyUnion as LoginResult;

            if (!loginResult.success()) {
                const error = "Authentication failed: Login not successful";
                console.error(error);
                onSystemMessage(error, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error }));
                return;
            }

            const playerObj = loginResult.player();
            if (!playerObj) {
                const error = "Authentication failed: No player object";
                console.error(error);
                onSystemMessage(error, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error }));
                return;
            }

            const playerOid = objToCurie(playerObj);
            if (!playerOid) {
                const error = "Authentication failed: Invalid player object";
                console.error(error);
                onSystemMessage(error, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error }));
                return;
            }

            const playerFlags = loginResult.playerFlags() || 0;

            // Store auth state in localStorage for session persistence
            localStorage.setItem("auth_token", authToken);
            localStorage.setItem("player_oid", playerOid);
            localStorage.setItem("player_flags", playerFlags.toString());

            // For create mode, store the generated encryption identity keyed by playerOid
            if (mode === "create" && generatedIdentity) {
                const storageKey = `moor_event_log_identity_${playerOid}`;
                localStorage.setItem(storageKey, generatedIdentity);
                console.log("Stored encryption identity for new account:", playerOid);
            }

            // Update player state (authorized but not yet connected)
            const player: Player = {
                oid: playerOid,
                authToken,
                connected: false,
                flags: playerFlags,
                clientToken: clientToken ?? null,
                clientId: clientId ?? null,
                isInitialAttach: true,
            };

            setAuthState({
                player,
                isConnecting: false,
                error: null,
            });

            // Check if user has history encryption to show appropriate message
            const hasHistory = localStorage.getItem(`moor_event_log_identity_${playerOid}`) !== null;
            onSystemMessage(hasHistory ? "Authenticated! Loading history..." : "Authenticated!", 2);

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
        // Clear OAuth2 credentials from localStorage
        localStorage.removeItem("oauth2_auth_token");
        localStorage.removeItem("oauth2_player_oid");
        localStorage.removeItem("oauth2_player_flags");

        // Clear regular auth credentials
        localStorage.removeItem("auth_token");
        localStorage.removeItem("player_oid");
        localStorage.removeItem("player_flags");

        // Clear connection credentials for this tab
        sessionStorage.removeItem("client_token");
        sessionStorage.removeItem("client_id");
        localStorage.setItem("client_session_active", "false");

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

    const setPlayerFlags = useCallback((flags: number) => {
        setAuthState(prev => ({
            ...prev,
            player: prev.player ? { ...prev.player, flags } : null,
        }));
    }, []);

    const clearInitialAttach = useCallback(() => {
        setAuthState(prev => {
            if (!prev.player) return prev;
            // Check if user has history encryption - if not, keep isInitialAttach true
            // so reconnects will trigger user_connected (otherwise they'd see a blank page)
            const hasEventLogEncryption = localStorage.getItem(
                `moor_event_log_identity_${prev.player.oid}`,
            ) !== null;
            return {
                ...prev,
                player: { ...prev.player, isInitialAttach: !hasEventLogEncryption },
            };
        });
    }, []);

    return {
        authState,
        connect,
        disconnect,
        setPlayerConnected,
        setPlayerFlags,
        clearInitialAttach,
    };
};
