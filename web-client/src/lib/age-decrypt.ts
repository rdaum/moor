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

// Age encryption/decryption utilities for event log
// Matches the Rust implementation in crates/web-host/src/host/event_log.rs

import { bech32 } from "@scure/base";
import { Decrypter, identityToRecipient } from "age-encryption";

/**
 * Create an age identity from 32 derived bytes (from Argon2)
 * Encodes bytes as bech32 AGE-SECRET-KEY string that age can parse
 *
 * This matches identity_from_derived_bytes() in Rust:
 * - Takes 32 bytes
 * - Encodes as bech32 with "age-secret-key-" prefix (note: hyphenated in Rust, but bech32 removes hyphens)
 * - Returns uppercase string
 */
export function identityFromDerivedBytes(bytes: Uint8Array): string {
    if (bytes.length !== 32) {
        throw new Error(`Expected 32 bytes, got ${bytes.length}`);
    }

    // Convert bytes to bech32 format with age-secret-key- prefix
    // The age-encryption library expects AGE-SECRET-KEY-1... format
    // Use encodeFromBytes which handles the byte-to-words conversion
    const encoded = bech32.encodeFromBytes("age-secret-key-", bytes);
    return encoded.toUpperCase();
}

/**
 * Decrypt an age-encrypted event blob using the provided identity
 *
 * This matches decrypt_event_blob() in Rust:
 * - Takes encrypted blob and identity string (AGE-SECRET-KEY-1...)
 * - Returns decrypted bytes
 */
export async function decryptEventBlob(
    encryptedBlob: Uint8Array,
    identityStr: string,
): Promise<Uint8Array> {
    try {
        // Create decryptor and add the identity
        const decrypter = new Decrypter();
        decrypter.addIdentity(identityStr);

        // Decrypt and return the plaintext bytes
        const decrypted = await decrypter.decrypt(encryptedBlob, "uint8array");
        return decrypted;
    } catch (error) {
        throw new Error(`Failed to decrypt event blob: ${error instanceof Error ? error.message : String(error)}`);
    }
}

/**
 * Helper to convert base64-encoded derived bytes to age identity
 * This is the typical flow:
 * 1. User enters password
 * 2. Argon2 derives 32 bytes
 * 3. Bytes stored as base64 in localStorage
 * 4. On decrypt: base64 -> bytes -> age identity string
 */
export function identityFromBase64DerivedBytes(base64Bytes: string): string {
    // Decode base64 to bytes
    const binaryString = atob(base64Bytes);
    const bytes = new Uint8Array(binaryString.length);
    for (let i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
    }

    if (bytes.length !== 32) {
        throw new Error(`Invalid derived key length: expected 32 bytes, got ${bytes.length}`);
    }

    return identityFromDerivedBytes(bytes);
}

/**
 * Extract the public key (recipient) from an age identity string
 * Takes an AGE-SECRET-KEY-1... identity and returns the corresponding age1... public key
 */
export async function publicKeyFromIdentity(identityStr: string): Promise<string> {
    try {
        const recipient = await identityToRecipient(identityStr);
        return recipient;
    } catch (error) {
        throw new Error(
            `Failed to derive public key from identity: ${error instanceof Error ? error.message : String(error)}`,
        );
    }
}
