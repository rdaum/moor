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

/** Structured actions carry untrusted context for the core's do_action handler. */
export const ACTION_PREFIX = 0x04;

export function encodeAction(action: Record<string, unknown>): Uint8Array<ArrayBuffer> {
    const body = new TextEncoder().encode(JSON.stringify(action));
    if (body.byteLength > 16 * 1024) throw new Error("Action is too large");
    const frame = new Uint8Array(body.length + 1);
    frame[0] = ACTION_PREFIX;
    frame.set(body, 1);
    return frame;
}

export function actionBody(data: string | Uint8Array<ArrayBuffer> | ArrayBuffer): Uint8Array<ArrayBuffer> | undefined {
    if (typeof data === "string") return undefined;
    const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
    return bytes[0] === ACTION_PREFIX && bytes[1] === 0x7b ? bytes.subarray(1) : undefined;
}
