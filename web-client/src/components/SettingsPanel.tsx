//! Settings panel with theme toggle and other options

import React from "react";
import { ThemeToggle } from "./ThemeToggle";

interface SettingsPanelProps {
    isOpen: boolean;
    onClose: () => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({ isOpen, onClose }) => {
    if (!isOpen) return null;

    return (
        <>
            {/* Backdrop */}
            <div className="settings-backdrop" onClick={onClose} />
            
            {/* Settings panel */}
            <div className="settings-panel">
                <div className="settings-header">
                    <h2>Settings</h2>
                    <button 
                        className="settings-close"
                        onClick={onClose}
                        aria-label="Close settings"
                    >
                        ×
                    </button>
                </div>
                
                <div className="settings-content">
                    <div className="settings-section">
                        <h3>Appearance</h3>
                        <ThemeToggle />
                    </div>
                    
                    <div className="settings-section">
                        <h3>Account</h3>
                        <div className="settings-item">
                            <span>Profile settings</span>
                            <span className="settings-placeholder">Coming soon</span>
                        </div>
                    </div>
                </div>
            </div>
        </>
    );
};