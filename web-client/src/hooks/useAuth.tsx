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

import * as flatbuffers from "flatbuffers";
import { useCallback, useEffect, useState } from "react";
import { ClientSuccess } from "../generated/moor-rpc/client-success";
import { unionToDaemonToClientReplyUnion } from "../generated/moor-rpc/daemon-to-client-reply-union";
import { LoginResult } from "../generated/moor-rpc/login-result";
import { ReplyResult } from "../generated/moor-rpc/reply-result";
import { ReplyResultUnion, unionToReplyResultUnion } from "../generated/moor-rpc/reply-result-union";
import { objToString } from "../lib/var";

export interface Player {
    oid: string;
    authToken: string;
    connected: boolean;
    flags: number;
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

    // Check for OAuth2 credentials in localStorage on mount
    useEffect(() => {
        const oauth2Token = localStorage.getItem("oauth2_auth_token");
        const oauth2PlayerOid = localStorage.getItem("oauth2_player_oid");

        if (oauth2Token && oauth2PlayerOid) {
            // Get flags from localStorage (stored during OAuth2 login)
            const oauth2Flags = localStorage.getItem("oauth2_player_flags");
            const flags = oauth2Flags ? parseInt(oauth2Flags, 10) : 0;

            // Set auth state (keep credentials in localStorage for future sessions)
            setAuthState({
                player: {
                    oid: oauth2PlayerOid,
                    authToken: oauth2Token,
                    connected: false,
                    flags,
                },
                isConnecting: false,
                error: null,
            });

            onSystemMessage("Authenticated via OAuth2! Loading history...", 2);
        }
    }, [onSystemMessage]);

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

            // Parse FlatBuffer response
            const arrayBuffer = await result.arrayBuffer();
            const bytes = new Uint8Array(arrayBuffer);
            const replyResult = ReplyResult.getRootAsReplyResult(
                new flatbuffers.ByteBuffer(bytes),
            );
            const authToken = result.headers.get("X-Moor-Auth-Token");

            // Validate authentication token
            if (!authToken) {
                const error = "Authentication failed: No token received";
                console.error(error);
                onSystemMessage(error, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error }));
                return;
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

            const playerOid = objToString(playerObj);
            if (!playerOid) {
                const error = "Authentication failed: Invalid player object";
                console.error(error);
                onSystemMessage(error, 5);
                setAuthState(prev => ({ ...prev, isConnecting: false, error }));
                return;
            }

            const playerFlags = loginResult.playerFlags() || 0;

            // Update player state (authorized but not yet connected)
            const player: Player = {
                oid: playerOid,
                authToken,
                connected: false,
                flags: playerFlags,
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
        // Clear OAuth2 credentials from localStorage
        localStorage.removeItem("oauth2_auth_token");
        localStorage.removeItem("oauth2_player_oid");

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

    return {
        authState,
        connect,
        disconnect,
        setPlayerConnected,
        setPlayerFlags,
    };
};
