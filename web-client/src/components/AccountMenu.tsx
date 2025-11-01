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

// ! Account menu with user-specific settings and logout

import React from "react";
import { EncryptionSettings } from "./EncryptionSettings";

interface AccountMenuProps {
    isOpen: boolean;
    onClose: () => void;
    onLogout?: () => void;
    historyAvailable: boolean;
}

export const AccountMenu: React.FC<AccountMenuProps> = ({ isOpen, onClose, onLogout, historyAvailable }) => {
    if (!isOpen) return null;

    return (
        <>
            {/* Backdrop */}
            <div className="settings-backdrop" onClick={onClose} />

            {/* Account menu */}
            <div className="account-menu">
                <div className="settings-header">
                    <h2>Account</h2>
                    <button
                        className="settings-close"
                        onClick={onClose}
                        aria-label="Close account menu"
                    >
                        ×
                    </button>
                </div>

                <div className="settings-content">
                    <div className="settings-section">
                        <h3>Profile</h3>
                        <div className="settings-item">
                            <span>Profile settings</span>
                            <span className="settings-placeholder">Coming soon</span>
                        </div>
                    </div>

                    <div className="settings-section">
                        <h3>Security</h3>
                        <EncryptionSettings isAvailable={historyAvailable} />
                    </div>

                    <div className="settings-section">
                        <h3>Session</h3>
                        {onLogout && (
                            <div className="settings-item">
                                <button
                                    className="btn btn-secondary"
                                    onClick={() => {
                                        onLogout();
                                        onClose();
                                    }}
                                >
                                    Logout
                                </button>
                            </div>
                        )}
                    </div>
                </div>
            </div>
        </>
    );
};
