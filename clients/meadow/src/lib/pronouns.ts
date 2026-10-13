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

export interface CustomPronouns {
    subject: string;
    object: string;
    possessive_adjective: string;
    possessive_pronoun: string;
    reflexive: string;
    agreement: "is" | "are";
}

export type PronounInput = string | CustomPronouns;

export const pronounFields = [
    { key: "subject", label: "___ is ready.", example: "ze" },
    { key: "object", label: "I met ___.", example: "zir" },
    { key: "possessive_adjective", label: "This is ___ coat.", example: "zir" },
    { key: "possessive_pronoun", label: "The coat is ___.", example: "zirs" },
    { key: "reflexive", label: "Ze did it ___.", example: "zirself" },
] as const;

export function readCustomPronouns(value: unknown): CustomPronouns | null {
    if (!value || typeof value !== "object") return null;
    const fields = value as Record<string, unknown>;
    if (fields.agreement !== "is" && fields.agreement !== "are") return null;
    if (!pronounFields.every(({ key }) => typeof fields[key] === "string")) return null;
    return {
        subject: fields.subject as string,
        object: fields.object as string,
        possessive_adjective: fields.possessive_adjective as string,
        possessive_pronoun: fields.possessive_pronoun as string,
        reflexive: fields.reflexive as string,
        agreement: fields.agreement,
    };
}
