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

import React, { createContext, useContext, useEffect } from "react";
import { useEventLogEncryption } from "../hooks/useEventLogEncryption";

interface EncryptionContextType {
    encryptionState: {
        hasEncryption: boolean;
        isChecking: boolean;
        hasCheckedOnce: boolean;
        ageIdentity: string | null;
    };
    checkEncryptionStatus: () => Promise<void>;
    setupEncryption: (password: string) => Promise<{ success: boolean; error?: string }>;
    unlockEncryption: (password: string) => Promise<{ success: boolean; error?: string }>;
    forgetKey: () => void;
    getKeyForHistoryRequest: () => string | null;
}

const EncryptionContext = createContext<EncryptionContextType | undefined>(undefined);

interface EncryptionProviderProps {
    children: React.ReactNode;
    authToken: string | null;
    playerOid: string | null;
}

export const EncryptionProvider: React.FC<EncryptionProviderProps> = ({
    children,
    authToken,
    playerOid,
}) => {
    const encryption = useEventLogEncryption(authToken, playerOid);
    const { checkEncryptionStatus } = encryption;

    // Check encryption status when auth token changes
    useEffect(() => {
        if (authToken && playerOid) {
            checkEncryptionStatus();
        }
    }, [authToken, playerOid, checkEncryptionStatus]);

    return (
        <EncryptionContext.Provider value={encryption}>
            {children}
        </EncryptionContext.Provider>
    );
};

export const useEncryptionContext = (): EncryptionContextType => {
    const context = useContext(EncryptionContext);
    if (context === undefined) {
        throw new Error("useEncryptionContext must be used within an EncryptionProvider");
    }
    return context;
};
