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
import { usePresentations } from "../hooks/usePresentations";
import { Presentation, PresentationData } from "../types/presentation";

interface PresentationContextType {
    presentations: Presentation[];
    addPresentation: (data: PresentationData) => void;
    removePresentation: (id: string) => void;
    getPresentationsByTarget: (target: string) => Presentation[];
    getLeftDockPresentations: () => Presentation[];
    getRightDockPresentations: () => Presentation[];
    getTopDockPresentations: () => Presentation[];
    getBottomDockPresentations: () => Presentation[];
    getWindowPresentations: () => Presentation[];
    getHelpPresentations: () => Presentation[];
    getVerbEditorPresentations: () => Presentation[];
    dismissPresentation: (id: string, authToken: string) => Promise<void>;
    fetchCurrentPresentations: (authToken: string, ageIdentity?: string | null) => Promise<boolean>;
    clearAll: () => void;
}

const PresentationContext = createContext<PresentationContextType | undefined>(undefined);

interface PresentationProviderProps {
    children: React.ReactNode;
}

export const PresentationProvider: React.FC<PresentationProviderProps> = ({ children }) => {
    const presentationHook = usePresentations();

    return (
        <PresentationContext.Provider value={presentationHook}>
            {children}
        </PresentationContext.Provider>
    );
};

export const usePresentationContext = (): PresentationContextType => {
    const context = useContext(PresentationContext);
    if (context === undefined) {
        throw new Error("usePresentationContext must be used within a PresentationProvider");
    }
    return context;
};
