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

/**
 * Utility for managing trusted external domains in localStorage.
 * Used to remember domains the user has approved for external link navigation.
 */

const TRUSTED_DOMAINS_KEY = "moor-trusted-external-domains";

interface TrustedDomainsStore {
    domains: string[];
    version: number;
}

/**
 * Get the list of trusted domains from localStorage.
 */
export function getTrustedDomains(): string[] {
    try {
        const stored = localStorage.getItem(TRUSTED_DOMAINS_KEY);
        if (!stored) return [];
        const parsed: TrustedDomainsStore = JSON.parse(stored);
        return parsed.domains || [];
    } catch {
        return [];
    }
}

/**
 * Add a domain to the trusted list.
 */
export function addTrustedDomain(domain: string): void {
    try {
        const current = getTrustedDomains();
        if (!current.includes(domain)) {
            const updated: TrustedDomainsStore = {
                domains: [...current, domain],
                version: 1,
            };
            localStorage.setItem(TRUSTED_DOMAINS_KEY, JSON.stringify(updated));
        }
    } catch (error) {
        console.warn("Failed to save trusted domain:", error);
    }
}

/**
 * Remove a domain from the trusted list.
 */
export function removeTrustedDomain(domain: string): void {
    try {
        const current = getTrustedDomains();
        const updated: TrustedDomainsStore = {
            domains: current.filter(d => d !== domain),
            version: 1,
        };
        localStorage.setItem(TRUSTED_DOMAINS_KEY, JSON.stringify(updated));
    } catch (error) {
        console.warn("Failed to remove trusted domain:", error);
    }
}

/**
 * Check if a URL's domain is in the trusted list.
 */
export function isDomainTrusted(url: string): boolean {
    try {
        const urlObj = new URL(url);
        const hostname = urlObj.hostname;
        return getTrustedDomains().includes(hostname);
    } catch {
        return false;
    }
}

/**
 * Extract the hostname from a URL.
 * Returns empty string if URL is invalid.
 */
export function getHostname(url: string): string {
    try {
        const urlObj = new URL(url);
        return urlObj.hostname;
    } catch {
        return "";
    }
}

/**
 * Clear all trusted domains.
 */
export function clearAllTrustedDomains(): void {
    localStorage.removeItem(TRUSTED_DOMAINS_KEY);
}
