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

export const buildAuthHeaders = (authToken: string): Record<string, string> => {
    const headers: Record<string, string> = {
        "X-Moor-Auth-Token": authToken,
    };
    // Connection credentials are per-tab (sessionStorage), not shared across tabs
    const clientToken = sessionStorage.getItem("client_token");
    const clientId = sessionStorage.getItem("client_id");

    if (clientToken) {
        headers["X-Moor-Client-Token"] = clientToken;
    }
    if (clientId) {
        headers["X-Moor-Client-Id"] = clientId;
    }

    return headers;
};

/**
 * Handle 401 Unauthorized responses by clearing local session state
 * and forcing a reload to show the login/welcome screen.
 */
export const handleUnauthorized = () => {
    console.warn("Session unauthorized or expired. Clearing credentials and reloading...");

    // Clear regular auth credentials
    localStorage.removeItem("auth_token");
    localStorage.removeItem("player_oid");
    localStorage.removeItem("player_flags");

    // Clear OAuth2 credentials
    localStorage.removeItem("oauth2_auth_token");
    localStorage.removeItem("oauth2_player_oid");
    localStorage.removeItem("oauth2_player_flags");

    // Clear connection credentials for this tab
    sessionStorage.removeItem("client_token");
    sessionStorage.removeItem("client_id");
    localStorage.setItem("client_session_active", "false");

    // Force reload to return to welcome/login screen
    window.location.href = "/";
};
