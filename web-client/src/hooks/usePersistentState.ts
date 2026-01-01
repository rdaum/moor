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

import { Dispatch, SetStateAction, useEffect, useMemo, useState } from "react";

export interface PersistentStateOptions<T> {
    serialize?: (value: T) => string;
    deserialize?: (value: string) => T | null;
    shouldPersist?: (value: T) => boolean;
}

type InitialValue<T> = T | (() => T);

export function usePersistentState<T>(
    key: string,
    initialValue: InitialValue<T>,
    options?: PersistentStateOptions<T>,
): [T, Dispatch<SetStateAction<T>>] {
    const serialize = options?.serialize ?? defaultSerialize;
    const deserialize = options?.deserialize ?? defaultDeserialize<T>;
    const shouldPersist = useMemo(() => options?.shouldPersist ?? (() => true), [options?.shouldPersist]);

    const readInitialValue = () => {
        const fallback = typeof initialValue === "function" ? (initialValue as () => T)() : initialValue;
        if (typeof window === "undefined") {
            return fallback;
        }
        try {
            const stored = window.localStorage.getItem(key);
            if (stored === null) {
                return fallback;
            }
            const parsed = deserialize(stored);
            return parsed ?? fallback;
        } catch {
            return fallback;
        }
    };

    const [value, setValue] = useState<T>(readInitialValue);

    useEffect(() => {
        if (typeof window === "undefined") {
            return;
        }
        if (!shouldPersist(value)) {
            window.localStorage.removeItem(key);
            return;
        }
        try {
            window.localStorage.setItem(key, serialize(value));
        } catch {
            // Ignore quota/storage errors
        }
    }, [key, serialize, shouldPersist, value]);

    return [value, setValue];
}

const defaultSerialize = <T>(value: T): string => {
    return JSON.stringify(value);
};

const defaultDeserialize = <T>(raw: string): T | null => {
    try {
        return JSON.parse(raw) as T;
    } catch {
        return null;
    }
};
