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
    const clientToken = localStorage.getItem("client_token");
    const clientId = localStorage.getItem("client_id");

    if (clientToken) {
        headers["X-Moor-Client-Token"] = clientToken;
    }
    if (clientId) {
        headers["X-Moor-Client-Id"] = clientId;
    }

    return headers;
};
