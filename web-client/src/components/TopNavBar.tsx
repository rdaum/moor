//! Mobile top navigation bar with hamburger menu and settings

import React from "react";
import { useTitle } from "../hooks/useTitle";

interface TopNavBarProps {
    onSettingsToggle: () => void;
}

export const TopNavBar: React.FC<TopNavBarProps> = ({ onSettingsToggle }) => {
    const title = useTitle();
    
    return (
        <div className="top-nav-bar">
            <button 
                className="hamburger-menu"
                onClick={onSettingsToggle}
                aria-label="Open settings menu"
            >
                <span className="hamburger-line"></span>
                <span className="hamburger-line"></span>
                <span className="hamburger-line"></span>
            </button>
            
            <div className="nav-title">{title}</div>
            
            <button 
                className="account-icon"
                aria-label="Account settings"
            >
                <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
                    <circle cx="12" cy="8" r="4"/>
                    <path d="M12 14c-4 0-8 2-8 6v2h16v-2c0-4-4-6-8-6z"/>
                </svg>
            </button>
        </div>
    );
};