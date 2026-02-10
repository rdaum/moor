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

export interface MockHttpRequest {
    method: string;
    url: URL;
    headers: Headers;
    body: string | null;
}

export interface MockHttpRoute {
    method?: string;
    path: string | RegExp;
    handler: (request: MockHttpRequest) => Response | Promise<Response>;
}

export interface MockHttpHostController {
    setRoutes(routes: MockHttpRoute[]): void;
    getRequests(): MockHttpRequest[];
    restore(): void;
}

function matchesRoute(route: MockHttpRoute, method: string, pathname: string): boolean {
    if (route.method && route.method.toUpperCase() !== method) {
        return false;
    }

    if (typeof route.path === "string") {
        return route.path === pathname;
    }

    return route.path.test(pathname);
}

export function installMockWebHostFetch(initialRoutes: MockHttpRoute[] = []): MockHttpHostController {
    const root = globalThis as typeof globalThis & {
        fetch: typeof fetch;
        location: Location;
    };
    const originalFetch = root.fetch.bind(root);
    let routes = [...initialRoutes];
    const requests: MockHttpRequest[] = [];

    root.fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
        const method = (init?.method || "GET").toUpperCase();
        const base = root.location?.origin || "http://localhost";
        const url = input instanceof URL
            ? input
            : new URL(
                typeof input === "string" ? input : input.url,
                base,
            );

        const request: MockHttpRequest = {
            method,
            url,
            headers: new Headers(init?.headers),
            body: typeof init?.body === "string" ? init.body : null,
        };
        requests.push(request);

        const route = routes.find((r) => matchesRoute(r, method, url.pathname));
        if (!route) {
            return new Response(`No mock route for ${method} ${url.pathname}`, { status: 404 });
        }
        return await route.handler(request);
    };

    return {
        setRoutes(nextRoutes: MockHttpRoute[]) {
            routes = [...nextRoutes];
        },
        getRequests() {
            return [...requests];
        },
        restore() {
            root.fetch = originalFetch;
        },
    };
}
