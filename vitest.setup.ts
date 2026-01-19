// Polyfill ResizeObserver for jsdom
global.ResizeObserver = class ResizeObserver {
    callback: ResizeObserverCallback;

    constructor(callback: ResizeObserverCallback) {
        this.callback = callback;
    }

    observe() {
        // No-op in tests
    }

    unobserve() {
        // No-op in tests
    }

    disconnect() {
        // No-op in tests
    }
};

// Mock sessionStorage
const sessionStorageMock = (() => {
    let store: Record<string, string> = {};
    return {
        getItem: (key: string) => store[key] || null,
        setItem: (key: string, value: string) => {
            store[key] = value;
        },
        removeItem: (key: string) => {
            delete store[key];
        },
        clear: () => {
            store = {};
        },
    };
})();

Object.defineProperty(window, "sessionStorage", {
    value: sessionStorageMock,
});
