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

import { expect, type Page, test } from "@playwright/test";
import { readFileSync } from "node:fs";

const styles = ["variables", "base", "components", "prism-moo"].map(name =>
    readFileSync(new URL(`../src/styles/${name}.css`, import.meta.url), "utf8")
).join("\n");

async function narrative(page: Page, html: string, width = 800) {
    // Use the same wrappers as OutputWindow and ContentRenderer.
    await page.setContent(
        `<div class="text_narrative live_narrative" style="width:${width}px">`
            + `<span class="content-renderer"><span class="content-html">${html}</span></span></div>`,
    );
    await page.addStyleTag({ content: styles });
}

test("HTML source whitespace does not add blank lines to room descriptions", async ({ page }) => {
    const blocks = [
        "<h3>The garden</h3>",
        "<p>A sheltered garden.<br>\nA path leads east.</p>",
        "<p>You see a brass key here.</p>",
        "<pre><code>first\n  second\nthird</code></pre>",
    ];
    const bounds = () =>
        page.locator(".content-html > *").evaluateAll(elements =>
            elements.map(element => {
                const { y, height } = element.getBoundingClientRect();
                return { y, height };
            })
        );
    await narrative(page, blocks.join(""));
    const compact = await bounds();
    await narrative(page, blocks.join("\n"));
    expect(await bounds()).toEqual(compact);
    const paragraph = page.locator("p").first();
    const lineHeight = await paragraph.evaluate(element => parseFloat(getComputedStyle(element).lineHeight));
    expect((await paragraph.boundingBox())!.height).toBeCloseTo(2 * lineHeight, 0);
    expect(await page.locator("pre code").innerText()).toBe("first\n  second\nthird");

    await page.locator(".content-html").evaluate(element => {
        element.setAttribute("class", "content-text");
        element.textContent = "first\n  second\nthird";
    });
    expect(await page.locator(".content-text").innerText()).toBe("first\n  second\nthird");
});

for (const sourceLinks of [false, true]) {
    test(`reference lists use responsive columns with source links: ${sourceLinks}`, async ({ page }) => {
        const labels = Array.from({ length: 20 }, (_, index) => `Command ${index + 1}`);
        const items = labels.map(label =>
            `<li><p><span class="semantic-annotation">${label}</span>`
            + (sourceLinks ? " <span class=\"semantic-annotation semantic-source-link\">↗</span>" : "")
            + "</p></li>"
        );
        const content = `<ul class="reference-columns">${items.join("\n")}</ul>`;
        const columns = () =>
            page.locator("li").evaluateAll(elements =>
                new Set(elements.map(element => Math.round(element.getBoundingClientRect().x))).size
            );
        await narrative(page, content);
        expect(await columns()).toBeGreaterThan(1);
        await page.setViewportSize({ width: 360, height: 800 });
        await narrative(page, content, 300);
        expect(await columns()).toBe(1);
        const list = page.locator("ul");
        expect(await list.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true);
        await expect(page.locator("li")).toHaveText(labels.map(label => label + (sourceLinks ? " ↗" : "")));
    });
}

test("core-rendered room panels preserve paragraphs and wrap their reference lists", async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 800 });
    const links = Array.from(
        { length: 8 },
        (_, i) => `<li><p><span class="semantic-annotation">East door ${i + 1}</span></p></li>`,
    ).join("\n");
    await page.setContent(
        `<div class="room_look_panel" style="width:300px"><div class="top_dock_panel_content">`
            + `<span class="content-renderer"><span class="content-html">`
            + `<p>A <strong>brass</strong> bell.</p>\n<p>A second paragraph.</p>\n`
            + `<h4>Exits</h4><ul class="reference-columns">${links}</ul>`
            + `</span></span></div></div>`,
    );
    await page.addStyleTag({ content: styles });
    await expect(page.locator("strong")).toHaveText("brass");
    const paragraphs = await page.locator(".content-html > p").all();
    expect(paragraphs).toHaveLength(2);
    const first = (await paragraphs[0].boundingBox())!;
    const second = (await paragraphs[1].boundingBox())!;
    expect(second.y).toBeGreaterThanOrEqual(first.y + first.height);
    const list = page.locator("ul");
    expect(await list.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true);
    const rows = await page.locator("li").evaluateAll(elements =>
        new Set(elements.map(element => Math.round(element.getBoundingClientRect().y))).size
    );
    expect(rows).toBeGreaterThan(1);
    await expect(page.locator("li")).toHaveCount(8);
});
