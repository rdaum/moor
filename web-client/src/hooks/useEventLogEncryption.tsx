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

// ! Hook for managing event log encryption (Argon2 key derivation + age keypair generation)
// ! Age keypair generation happens client-side - only public key is sent to server

import { useCallback, useState } from "react";
import { identityFromDerivedBytes, publicKeyFromIdentity } from "../lib/age-decrypt";
import { buildAuthHeaders } from "../lib/authHeaders";
import { deriveKeyBytes } from "../lib/keyDerivation";

interface EncryptionState {
    hasEncryption: boolean;
    isChecking: boolean;
    hasCheckedOnce: boolean; // Track if we've checked at least once
    ageIdentity: string | null; // AGE-SECRET-KEY-1... private key string
}

export const useEventLogEncryption = (
    authToken: string | null,
    playerOid: string | null,
) => {
    const [encryptionState, setEncryptionState] = useState<EncryptionState>(() => {
        // Initialize with saved identity if available
        if (playerOid) {
            const storageKey = `moor_event_log_identity_${playerOid}`;
            const savedIdentity = localStorage.getItem(storageKey);
            return {
                hasEncryption: false,
                isChecking: false,
                hasCheckedOnce: false,
                ageIdentity: savedIdentity,
            };
        }
        return {
            hasEncryption: false,
            isChecking: false,
            hasCheckedOnce: false,
            ageIdentity: null,
        };
    });

    const checkEncryptionStatus = useCallback(async () => {
        if (!authToken || !playerOid) return;

        setEncryptionState(prev => ({ ...prev, isChecking: true }));

        try {
            const headers = buildAuthHeaders(authToken);
            const response = await fetch("/api/event-log/pubkey", {
                headers,
            });

            if (!response.ok) {
                console.error("Failed to check encryption status:", response.status);
                return;
            }

            const data = await response.json();
            const hasEncryption = !!data.public_key;

            const storageKey = `moor_event_log_identity_${playerOid}`;
            const savedIdentity = localStorage.getItem(storageKey);

            setEncryptionState({
                hasEncryption,
                isChecking: false,
                hasCheckedOnce: true,
                ageIdentity: savedIdentity,
            });
        } catch (error) {
            console.error("Error checking encryption status:", error);
            setEncryptionState(prev => ({ ...prev, isChecking: false, hasCheckedOnce: true }));
        }
    }, [authToken, playerOid]);

    const setupEncryption = useCallback(async (password: string): Promise<{ success: boolean; error?: string }> => {
        if (!authToken || !playerOid) {
            return { success: false, error: "Not authenticated" };
        }

        try {
            console.log("Deriving encryption key for player:", playerOid);
            const bytes = await deriveKeyBytes(password, playerOid);

            // Generate age identity from derived bytes (client-side)
            console.log("Generating age keypair client-side...");
            const identity = identityFromDerivedBytes(bytes);
            console.log("Generated identity:", identity.substring(0, 30) + "...");

            // Extract public key from identity
            const publicKey = await publicKeyFromIdentity(identity);
            console.log("Extracted public key:", publicKey);

            // Send only the public key to server (NOT derived bytes or identity)
            console.log("Sending public key to server...");
            const headers = buildAuthHeaders(authToken);
            headers["Content-Type"] = "application/json";
            const response = await fetch("/api/event-log/pubkey", {
                method: "PUT",
                headers,
                body: JSON.stringify({ public_key: publicKey }),
            });

            console.log("Pubkey setup response status:", response.status);
            if (!response.ok) {
                const errorText = await response.text();
                console.error("Pubkey setup failed:", errorText);
                return { success: false, error: `Server error: ${response.status}` };
            }

            const responseData = await response.json();
            console.log("Pubkey setup response:", responseData);

            // Store the private key (identity) locally - server never sees this
            const storageKey = `moor_event_log_identity_${playerOid}`;
            localStorage.setItem(storageKey, identity);

            setEncryptionState({
                hasEncryption: true,
                isChecking: false,
                hasCheckedOnce: true,
                ageIdentity: identity,
            });

            return { success: true };
        } catch (error) {
            console.error("Encryption setup failed:", error);
            return { success: false, error: error instanceof Error ? error.message : "Unknown error" };
        }
    }, [authToken, playerOid]);

    const unlockEncryption = useCallback(async (password: string): Promise<{ success: boolean; error?: string }> => {
        if (!authToken || !playerOid) {
            return { success: false, error: "Not authenticated" };
        }

        try {
            const bytes = await deriveKeyBytes(password, playerOid);

            // Generate age identity from derived bytes
            const identity = identityFromDerivedBytes(bytes);

            // TODO: Validate by fetching and decrypting a test event

            const storageKey = `moor_event_log_identity_${playerOid}`;
            localStorage.setItem(storageKey, identity);

            setEncryptionState(prev => ({
                ...prev,
                ageIdentity: identity,
            }));

            return { success: true };
        } catch (error) {
            console.error("Failed to unlock encryption:", error);
            return { success: false, error: error instanceof Error ? error.message : "Unknown error" };
        }
    }, [authToken, playerOid]);

    const forgetKey = useCallback(() => {
        if (!playerOid) return;

        const storageKey = `moor_event_log_identity_${playerOid}`;
        localStorage.removeItem(storageKey);

        setEncryptionState(prev => ({
            ...prev,
            ageIdentity: null,
        }));
    }, [playerOid]);

    const getKeyForHistoryRequest = useCallback((): string | null => {
        return encryptionState.ageIdentity;
    }, [encryptionState.ageIdentity]);

    return {
        encryptionState,
        checkEncryptionStatus,
        setupEncryption,
        unlockEncryption,
        forgetKey,
        getKeyForHistoryRequest,
    };
};
