// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Lesser General Public License as published by the Free Software Foundation,
// version 3 or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Lesser General Public License for more
// details.
//
// You should have received a copy of the GNU Lesser General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

export function createUnauthorizedAwareFetch(onUnauthorized: () => void) {
    return async function moorFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
        const response = await window.fetch(input, init);
        if (response.status === 401) {
            onUnauthorized();
            // Stop further execution while caller lifecycle handles redirect/reload.
            return new Promise(() => {
                // intentionally pending
            });
        }
        return response;
    };
}
