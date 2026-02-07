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

import type { MoorHttpError } from "./types";

export async function expectOk(response: Response): Promise<Response> {
    if (response.ok) {
        return response;
    }

    const err: MoorHttpError = {
        status: response.status,
        statusText: response.statusText,
        body: await response.text().catch(() => undefined),
    };

    throw err;
}

export async function postPlainText(
    url: string,
    body: string,
    headers: Record<string, string>,
): Promise<Response> {
    return fetch(url, {
        method: "POST",
        headers: {
            ...headers,
            "Content-Type": "text/plain",
        },
        body,
    });
}
