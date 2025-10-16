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

import React, { createContext, useContext } from "react";
import { AuthState, useAuth } from "../hooks/useAuth";

interface AuthContextType {
    authState: AuthState;
    connect: (mode: "connect" | "create", username: string, password: string) => Promise<void>;
    disconnect: () => void;
    setPlayerConnected: (connected: boolean) => void;
    setPlayerFlags: (flags: number) => void;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

interface AuthProviderProps {
    children: React.ReactNode;
    showMessage: (message: string, duration?: number) => void;
}

export const AuthProvider: React.FC<AuthProviderProps> = ({ children, showMessage }) => {
    const authHook = useAuth(showMessage);

    return (
        <AuthContext.Provider value={authHook}>
            {children}
        </AuthContext.Provider>
    );
};

export const useAuthContext = (): AuthContextType => {
    const context = useContext(AuthContext);
    if (context === undefined) {
        throw new Error("useAuthContext must be used within an AuthProvider");
    }
    return context;
};
