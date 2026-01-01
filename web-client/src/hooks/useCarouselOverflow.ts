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

// ! Custom hook for detecting carousel overflow and managing visual indicators

import { useCallback, useEffect, useRef, useState } from "react";

export const useCarouselOverflow = () => {
    const containerRef = useRef<HTMLDivElement>(null);
    const [hasOverflow, setHasOverflow] = useState(false);
    const [hasScroll, setHasScroll] = useState(false);

    const checkOverflow = useCallback(() => {
        const container = containerRef.current;
        if (!container) return;

        const { scrollWidth, clientWidth, scrollLeft } = container;
        const maxScrollLeft = scrollWidth - clientWidth;

        // Show right indicator if we can scroll right (not at the end)
        const canScrollRight = scrollLeft < (maxScrollLeft - 5); // 5px threshold
        // Show left indicator if we can scroll left (not at the beginning)
        const canScrollLeft = scrollLeft > 5; // 5px threshold

        // Debug logging - remove when stable
        // console.log('Carousel state:', {
        //     scrollWidth,
        //     clientWidth,
        //     scrollLeft,
        //     maxScrollLeft,
        //     canScrollRight,
        //     canScrollLeft,
        //     className: container.className
        // });

        setHasOverflow(canScrollRight);
        setHasScroll(canScrollLeft);
    }, []);

    useEffect(() => {
        const container = containerRef.current;
        if (!container) return;

        // Check overflow on mount and resize
        checkOverflow();
        const resizeObserver = new ResizeObserver(checkOverflow);
        resizeObserver.observe(container);

        // Listen for scroll events to update scroll indicator
        const handleScroll = () => {
            // Use requestAnimationFrame to ensure we get the latest scroll position
            requestAnimationFrame(checkOverflow);
        };

        container.addEventListener("scroll", handleScroll, { passive: true });

        return () => {
            resizeObserver.disconnect();
            container.removeEventListener("scroll", handleScroll);
        };
    }, [checkOverflow]);

    return {
        containerRef,
        hasOverflow,
        hasScroll,
    };
};
