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

// ! Hook for fetching and managing the MOO title

import { useEffect, useState } from "react";
import { getSystemPropertyFlatBuffer } from "../lib/rpc-fb";

export const useTitle = () => {
    const [title, setTitle] = useState<string>("mooR");

    useEffect(() => {
        // Fetch title from system property using FlatBuffer API
        getSystemPropertyFlatBuffer(["login"], "moo_title")
            .then(data => {
                if (data && typeof data === "string") {
                    setTitle(data);
                } else if (Array.isArray(data) && data.length > 0) {
                    // Handle array response (join lines)
                    setTitle(data.join(" ") || "mooR");
                }
            })
            .catch(() => {
                // Keep default "mooR" title
                console.log("Using default title: mooR");
            });
    }, []);

    return title;
};
