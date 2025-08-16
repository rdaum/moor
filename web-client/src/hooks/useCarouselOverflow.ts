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
