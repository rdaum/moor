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

// ! Hook for managing event log encryption (Argon2 key derivation)
// ! Age keypair generation happens server-side in web-host

import { useCallback, useState } from "react";

interface EncryptionState {
    hasEncryption: boolean;
    isChecking: boolean;
    hasCheckedOnce: boolean; // Track if we've checked at least once
    derivedKeyBytes: string | null; // base64-encoded 32 bytes from Argon2
}

/**
 * Derive 32 bytes from password using Argon2id KDF
 * Uses player OID as salt for domain separation
 * Same password + same player OID = same bytes (deterministic)
 * Returns base64-encoded bytes
 */
async function deriveKeyBytes(password: string, playerOid: string): Promise<string> {
    const saltString = `moor-event-log-v1-${playerOid}`;

    // Set WASM path before loading argon2
    // @ts-ignore
    window.argon2WasmPath = "/argon2.wasm";

    // Load argon2-browser - it attaches to window.argon2
    // @ts-ignore
    await import("argon2-browser");

    // @ts-ignore - UMD module attaches to window
    const argon2 = window.argon2;

    if (!argon2 || typeof argon2.hash !== "function") {
        throw new Error("argon2-browser failed to load");
    }

    const result = await argon2.hash({
        pass: password,
        salt: saltString,
        type: 2, // Argon2id
        time: 3,
        mem: 65536, // 64 MiB
        parallelism: 4,
        hashLen: 32,
    });

    // Convert to base64
    const bytes = new Uint8Array(result.hash);
    return btoa(String.fromCharCode(...bytes));
}

export const useEventLogEncryption = (
    authToken: string | null,
    playerOid: string | null,
) => {
    const [encryptionState, setEncryptionState] = useState<EncryptionState>(() => {
        // Initialize with saved key if available
        if (playerOid) {
            const storageKey = `moor_event_log_key_${playerOid}`;
            const savedKeyBytes = localStorage.getItem(storageKey);
            return {
                hasEncryption: false,
                isChecking: false,
                hasCheckedOnce: false,
                derivedKeyBytes: savedKeyBytes,
            };
        }
        return {
            hasEncryption: false,
            isChecking: false,
            hasCheckedOnce: false,
            derivedKeyBytes: null,
        };
    });

    const checkEncryptionStatus = useCallback(async () => {
        if (!authToken || !playerOid) return;

        setEncryptionState(prev => ({ ...prev, isChecking: true }));

        try {
            const response = await fetch("/api/event-log/pubkey", {
                headers: { "X-Moor-Auth-Token": authToken },
            });

            if (!response.ok) {
                console.error("Failed to check encryption status:", response.status);
                return;
            }

            const data = await response.json();
            const hasEncryption = !!data.public_key;

            const storageKey = `moor_event_log_key_${playerOid}`;
            const savedKeyBytes = localStorage.getItem(storageKey);

            setEncryptionState({
                hasEncryption,
                isChecking: false,
                hasCheckedOnce: true,
                derivedKeyBytes: savedKeyBytes,
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
            const derivedBytes = await deriveKeyBytes(password, playerOid);
            console.log("Derived key bytes (base64):", derivedBytes.substring(0, 20) + "...");

            console.log("Sending pubkey setup request...");
            const response = await fetch("/api/event-log/pubkey", {
                method: "PUT",
                headers: {
                    "X-Moor-Auth-Token": authToken,
                    "Content-Type": "application/json",
                },
                body: JSON.stringify({ derived_key_bytes: derivedBytes }),
            });

            console.log("Pubkey setup response status:", response.status);
            if (!response.ok) {
                const errorText = await response.text();
                console.error("Pubkey setup failed:", errorText);
                return { success: false, error: `Server error: ${response.status}` };
            }

            const responseData = await response.json();
            console.log("Pubkey setup response:", responseData);

            const storageKey = `moor_event_log_key_${playerOid}`;
            localStorage.setItem(storageKey, derivedBytes);

            setEncryptionState({
                hasEncryption: true,
                isChecking: false,
                hasCheckedOnce: true,
                derivedKeyBytes: derivedBytes,
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
            const derivedBytes = await deriveKeyBytes(password, playerOid);

            // TODO: Validate by fetching and decrypting a test event

            const storageKey = `moor_event_log_key_${playerOid}`;
            localStorage.setItem(storageKey, derivedBytes);

            setEncryptionState(prev => ({
                ...prev,
                derivedKeyBytes: derivedBytes,
            }));

            return { success: true };
        } catch (error) {
            console.error("Failed to unlock encryption:", error);
            return { success: false, error: error instanceof Error ? error.message : "Unknown error" };
        }
    }, [authToken, playerOid]);

    const forgetKey = useCallback(() => {
        if (!playerOid) return;

        const storageKey = `moor_event_log_key_${playerOid}`;
        localStorage.removeItem(storageKey);

        setEncryptionState(prev => ({
            ...prev,
            derivedKeyBytes: null,
        }));
    }, [playerOid]);

    const getKeyForHistoryRequest = useCallback((): string | null => {
        return encryptionState.derivedKeyBytes;
    }, [encryptionState.derivedKeyBytes]);

    return {
        encryptionState,
        checkEncryptionStatus,
        setupEncryption,
        unlockEncryption,
        forgetKey,
        getKeyForHistoryRequest,
    };
};
