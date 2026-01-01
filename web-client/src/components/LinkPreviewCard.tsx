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

export interface LinkPreview {
    url: string;
    title?: string;
    description?: string;
    image?: string;
    site_name?: string;
}

interface LinkPreviewCardProps {
    preview: LinkPreview;
}

export function LinkPreviewCard({ preview }: LinkPreviewCardProps) {
    const { url, title, description, image, site_name } = preview;

    const displayTitle = title || url;
    const hostname = (() => {
        try {
            return new URL(url).hostname;
        } catch {
            return site_name || "";
        }
    })();
    const siteName = site_name || hostname;
    const accessibleLabel = title
        ? `Link preview: ${title} from ${siteName}`
        : `Link to ${siteName}`;

    return (
        <article
            className="link-preview-card"
            role="article"
            aria-label={accessibleLabel}
        >
            <a
                href={url}
                target="_blank"
                rel="noopener noreferrer"
                aria-label={accessibleLabel}
            >
                {image && (
                    <div className="link-preview-image" aria-hidden="true">
                        <img src={image} alt="" loading="lazy" />
                    </div>
                )}
                <div className="link-preview-content">
                    <div className="link-preview-title">{displayTitle}</div>
                    {description && <div className="link-preview-description">{description}</div>}
                    <div className="link-preview-hostname">
                        {siteName}
                    </div>
                </div>
            </a>
        </article>
    );
}
