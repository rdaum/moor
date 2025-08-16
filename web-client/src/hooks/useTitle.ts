//! Hook for fetching and managing the MOO title

import { useEffect, useState } from "react";

export const useTitle = () => {
    const [title, setTitle] = useState<string>("mooR");

    useEffect(() => {
        // Fetch title from system property - same call as in index.html
        fetch('/system_property/login/moo_title')
            .then(response => response.ok ? response.json() : Promise.reject())
            .then(data => {
                if (data && typeof data === 'string') {
                    setTitle(data);
                } else if (Array.isArray(data) && data.length > 0) {
                    // Handle array response (like welcome message)
                    setTitle(data.join(' ') || "mooR");
                }
            })
            .catch(() => {
                // Keep default "mooR" title
                console.log('Using default title: mooR');
            });
    }, []);

    return title;
};