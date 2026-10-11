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
import type {} from "./review";
import { ChangesFixture, source } from "./changes-fixture";

let fixture: ChangesFixture;
test.beforeEach(async () => {
    fixture = new ChangesFixture();
    await fixture.start();
});
test.afterEach(async () => {
    await fixture?.close();
});

async function login(page: Page, player = "ArchWizard") {
    await page.goto(fixture.browserUrl);
    await page.waitForFunction(() => Boolean(window.changes));
    await page.evaluate(player => window.changes.login(player), player);
}
async function configure(page: Page, transport: "git" | "http", unbased = false) {
    const upstream = transport === "git"
        ? `["transport" -> "git", "repository" -> "${fixture.gitUrl}", "revision" -> ["ref" -> "refs/heads/main"], "path" -> "cores/sample/src"]`
        : `"${fixture.httpUrl}"`;
    await page.evaluate(
        ({ upstream, unbased }) =>
            window.changes.evaluate(`
        ${unbased ? "clear_verb_metadata(#9500, \"unbased\", \"objdef_base\");" : ""}
        return $change_manager:configure("acceptance", {#9500, #9501}, [], ${upstream});
    `),
        { upstream, unbased },
    );
}
async function stage(page: Page) {
    const review = await page.evaluate(async () => {
        const response = await window.changes.invoke("stage", ["acceptance"]) as { review_id: number };
        return response.review_id;
    });
    await expect.poll(() => page.evaluate(id => window.changes.client(id).status(), review)).toMatchObject({
        status: "ready",
    });
    return review;
}
async function choose(
    page: Page,
    review: number,
    member: string,
    choice: "incoming" | "local" | "edited" | "defer",
    program = "",
) {
    return page.evaluate(async ({ review, member, choice, program }) => {
        const client = window.changes.client(review);
        const status = await client.status();
        const row = (await client.page(status.generation)).rows.find(row => row.memberLabel === `:${member}`)!;
        return client.resolve(status.generation, row.id, choice, program);
    }, { review, member, choice, program });
}
async function acceptConflicts(page: Page, review: number) {
    for (const name of ["conflict", "edited", "deferred"]) await choose(page, review, name, "incoming");
}
async function programs(page: Page) {
    return page.evaluate(() =>
        window.changes.evaluate(
            "return {#9500:upstream(), #9500:local(), #9500:conflict(), #9500:edited(), #9500:deferred()};",
        )
    );
}

test("Git review preserves choices and edited drafts across restart, then applies with deferrals", async ({ page, context }) => {
    const browserErrors: string[] = [];
    page.on("pageerror", error => browserErrors.push(error.message));
    await login(page);
    await configure(page, "git", true);
    const review = await stage(page);
    const overview = await page.evaluate(id => window.changes.client(id).page(1), review);
    expect(Object.fromEntries(overview.rows.map(row => [row.memberLabel.slice(1), row.classification]))).toEqual({
        unchanged: "unchanged",
        upstream: "upstream",
        local: "local",
        converged: "converged",
        conflict: "conflict",
        edited: "conflict",
        deferred: "conflict",
        unbased: "unbased",
    });
    const inspection = await page.evaluate(id => window.changes.client(id).inspection(1), review);
    expect(inspection.rows.filter(row => row.field === "property" && row.classification !== "unchanged")).toEqual([]);
    const provenance = await page.evaluate(id => window.changes.invoke("status", [id]), review);
    expect(provenance).toMatchObject({
        provenance: { transport: "git", commit: `sha1:${fixture.revisionB}`, path: "cores/sample/src" },
    });

    const second = await context.newPage();
    await login(second);
    const row = overview.rows.find(row => row.memberLabel === ":conflict")!;
    await choose(page, review, "conflict", "local");
    const stale = await second.evaluate(async ({ review, row }) => {
        try {
            await window.changes.client(review).resolve(1, row, "incoming");
            return false;
        } catch {
            return true;
        }
    }, { review, row: row.id });
    expect(stale).toBe(true);
    await second.close();
    await choose(page, review, "deferred", "defer");
    await choose(page, review, "unbased", "defer");
    const status = await page.evaluate(id => window.changes.client(id).status(), review);
    await page.evaluate(({ review, generation }) => window.changes.open(review, generation), {
        review,
        generation: status.generation,
    });
    await page.getByText(/^Source and checks/).click();
    await expect(page.locator(".change-review-source")).toContainText(`sha1:${fixture.revisionB}`);
    await expect(page.locator(".change-review-source")).toContainText("does not establish compatibility");
    await page.getByText(/^Source and checks/).click();
    await page.getByRole("button", { name: /:edited, Verb/ }).click();
    await page.getByRole("button", { name: "Edit resolution", exact: true }).click();
    const editor = page.locator(".change-review-draft").getByRole("textbox", { name: "Editor content", exact: true });
    await editor.focus();
    await editor.press("ControlOrMeta+A");
    await editor.pressSequentially("return (;");
    await page.getByRole("button", { name: "Save proposed program" }).click();
    await expect(page.getByText(/Line \d+:/)).toBeVisible();
    const invalidGeneration = (await page.evaluate(id => window.changes.client(id).status(), review)).generation;
    await page.evaluate(({ review, generation }) => window.changes.open(review, generation), {
        review,
        generation: invalidGeneration,
    });
    await page.getByRole("button", { name: /:edited, Verb/ }).click();

    await page.getByRole("button", { name: "Edit resolution", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.changes.markers())).toHaveLength(1);
    await editor.focus();
    await editor.press("ControlOrMeta+A");
    await editor.pressSequentially("return 9;");
    await expect.poll(() => page.evaluate(() => window.changes.markers())).toHaveLength(0);
    await page.getByRole("button", { name: "Save proposed program" }).click();
    await expect(page.getByText("Choice saved. Live code has not changed.")).toBeVisible();
    expect(await programs(page)).toEqual([1, 3, 3, 3, 3]);

    const saved = await page.evaluate(async id => {
        const client = window.changes.client(id);
        const status = await client.status();
        return { status, page: await client.page(status.generation) };
    }, review);
    await fixture.restart();
    await login(page);
    const reopened = await page.evaluate(async id => {
        const client = window.changes.client(id);
        const status = await client.status();
        return { status, page: await client.page(status.generation) };
    }, review);
    expect(reopened).toEqual(saved);
    const editedRow = reopened.page.rows.find(row => row.memberLabel === ":edited")!;
    const restoredDraft = await page.evaluate(
        ({ review, generation, row }) => window.changes.client(review).details(generation, row),
        { review, generation: reopened.status.generation, row: editedRow.id },
    );
    expect(restoredDraft.choice).toMatchObject({ choice: "edited", program: "return 9;", validated: true });
    await page.evaluate(({ review, generation }) => window.changes.open(review, generation), {
        review,
        generation: saved.status.generation,
    });
    await page.getByRole("button", { name: /^Apply \d+ choices?$/ }).click();
    await page.getByRole("button", { name: "Confirm apply" }).click();
    await expect.poll(() => page.evaluate(id => window.changes.client(id).status(), review)).toMatchObject({
        status: "partial",
    });
    expect(await programs(page)).toEqual([2, 3, 3, 9, 3]);
    const finalRows = await page.evaluate(async id => {
        const client = window.changes.client(id);
        return (await client.page((await client.status()).generation)).rows;
    }, review);
    expect(finalRows.find(row => row.memberLabel === ":edited")).toMatchObject({ classification: "local" });
    expect(finalRows.find(row => row.memberLabel === ":deferred")).toMatchObject({ classification: "conflict" });
    expect(browserErrors).toEqual([]);
});

test("HTTP apply with a lost response recovers the same committed receipt", async ({ page }) => {
    await login(page);
    await configure(page, "http");
    const review = await stage(page);
    await acceptConflicts(page, review);
    const status = await page.evaluate(id => window.changes.client(id).status(), review);
    await page.route("**/v1/verbs/oid:2000/apply/invoke", async route => {
        await route.fetch(); // The request reached the real server; drop only its response.
        await route.abort("connectionreset");
    });
    await page.evaluate(async ({ review, generation }) => {
        try {
            await window.changes.client(review).apply(generation);
        } catch {
            return;
        }
        throw new Error("Expected the transport response to be lost");
    }, { review, generation: status.generation });
    await page.unroute("**/v1/verbs/oid:2000/apply/invoke");
    await expect.poll(() => page.evaluate(id => window.changes.client(id).status(), review)).toMatchObject({
        status: "complete",
    });
    const receipt = await page.evaluate(id => window.changes.invoke("status", [id]), review);
    expect(receipt).toMatchObject({ provenance: { transport: "http", url: fixture.httpUrl } });
    const retried = await page.evaluate(
        ({ review, generation }) => window.changes.invoke("apply", [review, generation]),
        { review, generation: status.generation },
    );
    expect(retried).toEqual(receipt);
    expect(await programs(page)).toEqual([2, 3, 2, 2, 2]);
    await fixture.restart();
    await login(page);
    expect(await page.evaluate(id => window.changes.invoke("status", [id]), review)).toEqual(receipt);
});

test("a live edit rejects apply, and refresh clears choices only after confirmation", async ({ page }) => {
    await login(page);
    await configure(page, "git");
    const review = await stage(page);
    await acceptConflicts(page, review);
    const before = await page.evaluate(async id => {
        const client = window.changes.client(id);
        const status = await client.status();
        return { status, page: await client.page(status.generation) };
    }, review);
    await page.evaluate(() =>
        window.changes.evaluate("set_verb_code(#9500, \"upstream\", {\"return 99;\"}); return 1;")
    );
    await page.evaluate(({ review, generation }) => window.changes.client(review).apply(generation), {
        review,
        generation: before.status.generation,
    });
    await expect.poll(() => page.evaluate(id => window.changes.client(id).status(), review)).toMatchObject({
        status: "rejected",
    });
    expect(await programs(page)).toEqual([99, 3, 3, 3, 3]);
    const rejected = await page.evaluate(id => window.changes.client(id).status(), review);
    await page.evaluate(({ review, generation }) => window.changes.open(review, generation), {
        review,
        generation: rejected.generation,
    });
    await page.getByRole("button", { name: "Refresh comparison" }).click();
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    expect(await page.evaluate(id => window.changes.client(id).status(), review)).toEqual(rejected);
    await page.getByRole("button", { name: "Refresh comparison" }).click();
    await page.getByRole("button", { name: "Confirm refresh" }).click();
    await expect.poll(() => page.evaluate(id => window.changes.client(id).status(), review)).toMatchObject({
        status: "ready",
        generation: rejected.generation + 1,
    });
    const refreshed = await page.evaluate(async id => {
        const client = window.changes.client(id);
        return client.page((await client.status()).generation);
    }, review);
    expect(refreshed.rows.every(row => row.choice === undefined)).toBe(true);
    expect(refreshed.rows.find(row => row.memberLabel === ":upstream")?.classification).toBe("conflict");
});

test("revoking authority while the HTTP worker is suspended prevents staging", async ({ page, context }) => {
    await login(page, "AcceptanceAdmin");
    await configure(page, "http");
    const held = fixture.holdHttp();
    const pending = await page.evaluate(() => window.changes.invoke("stage", ["acceptance"])) as { review_id: number };
    await held.arrived;
    const administrator = await context.newPage();
    await login(administrator);
    await administrator.evaluate(() => window.changes.evaluate("#9502.wizard = false; return 1;"));
    held.release();
    await expect.poll(() => administrator.evaluate(id => window.changes.client(id).status(), pending.review_id))
        .toMatchObject({ status: "failed" });
    expect(
        await administrator.evaluate(
            id => window.changes.evaluate(`return length($change_manager.pending[${id}]["sources"]);`),
            pending.review_id,
        ),
    ).toBe(0);
    expect(await programs(administrator)).toEqual([1, 3, 3, 3, 3]);
    await administrator.close();
});

test("a failure after native writes rolls back programs, baselines, and receipts", async ({ page }) => {
    await login(page);
    await configure(page, "git");
    const review = await stage(page);
    await acceptConflicts(page, review);
    const baseline = await page.evaluate(() =>
        window.changes.evaluate("return verb_metadata(#9500, \"upstream\", \"objdef_base\");")
    );
    const status = await page.evaluate(id => window.changes.client(id).status(), review);
    const task = await page.evaluate(
        ({ review, generation }) =>
            window.changes.evaluate(`
        const started = $change_manager:apply(${review}, ${generation});
        $change_manager.receipts = 0;
        return started["task"];
    `),
        { review, generation: status.generation },
    );
    await expect.poll(() => page.evaluate(task => window.changes.evaluate(`return valid_task(${task});`), task)).toBe(
        false,
    );
    await page.evaluate(() => window.changes.evaluate("$change_manager.receipts = []; return 1;"));
    expect(await page.evaluate(id => window.changes.client(id).status(), review)).toMatchObject({
        status: "interrupted",
    });
    expect(await programs(page)).toEqual([1, 3, 3, 3, 3]);
    expect(
        await page.evaluate(() =>
            window.changes.evaluate("return verb_metadata(#9500, \"upstream\", \"objdef_base\");")
        ),
    ).toEqual(baseline);
    expect(await page.evaluate(() => window.changes.evaluate("return length($change_manager.receipts);"))).toBe(0);
});

test("workspace configures bindings, fetches, discards, and adopts uploaded source", async ({ page }) => {
    await login(page);
    await page.evaluate(() => window.changes.workspace());
    await page.getByRole("button", { name: "New package", exact: true }).click();
    await page.getByLabel("Package name", { exact: true }).fill("workspace");
    await page.getByLabel("Managed objects").fill("#9500 #9501");
    await page.getByLabel("Object constant bindings").fill("PROBE = #9500");
    await page.getByLabel("Trusted owners").fill("#2");
    await page.getByRole("combobox", { name: "Source", exact: true }).selectOption("git");
    await page.getByLabel("Repository URL").fill(fixture.gitUrl);
    await page.getByLabel("Repository subdirectory").fill("cores/sample/src");
    await page.getByRole("button", { name: "Save configuration" }).click();
    await expect(page.getByLabel("Package", { exact: true })).toHaveValue("workspace");
    await expect(page.getByText("2 managed objects", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Fetch source", exact: true }).click();
    await expect(page.getByRole("button", { name: "Open review", exact: true })).toBeEnabled();
    await page.getByRole("button", { name: "Open review", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "workspace · Review 1", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Close", exact: true }).click();
    await page.getByRole("button", { name: "Discard review", exact: true }).click();
    await page.getByRole("button", { name: "Discard saved review", exact: true }).click();
    await expect(page.getByRole("region", { name: "Active review" })).toHaveCount(0);

    const { source } = await import("./changes-fixture");
    await page.getByLabel("Upload objdef files").setInputFiles({
        name: "base.moo",
        mimeType: "text/plain",
        buffer: Buffer.from(source("base")),
    });
    await page.getByLabel("Adopt supplied source as the baseline").check();
    await page.getByRole("button", { name: "Review uploaded source" }).click();
    await page.getByRole("button", { name: "Open review", exact: true }).click();
    await page.getByRole("button", { name: "Accept 8 baselines", exact: true }).click();
    await page.getByRole("button", { name: "Confirm apply", exact: true }).click();
    await expect(page.getByText("Updates applied.", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("region", { name: "Completed reviews" })).toContainText("Review 2");
    await page.getByRole("region", { name: "Completed reviews" }).getByRole("button", { name: "Review 2", exact: true })
        .click();
    await expect(page.getByText("Updates applied.", { exact: true })).toBeVisible();
    expect(await programs(page)).toEqual([1, 3, 3, 3, 3]);
    await page.getByText(/^Source and checks/).click();
    await expect(page.getByRole("region", { name: "Apply receipt" })).toContainText("#9500:edited");
    await expect(page.getByRole("region", { name: "Apply receipt" })).toContainText("8 decisions committed");
    await page.getByRole("button", { name: "Close", exact: true }).click();
    await page.getByRole("button", { name: "Configure upstream" }).click();
    await page.getByRole("combobox", { name: "Source", exact: true }).selectOption("http");
    await page.getByLabel("Bundle URL").fill(fixture.httpUrl);
    await page.evaluate(async () => {
        const response = await window.changes.invoke("packages", []) as {
            packages: Record<string, { generation: number }>;
        };
        await window.changes.invoke("upstream", [
            "workspace",
            "https://example.org/newer.moo",
            response.packages.workspace.generation,
        ]);
    });
    await page.getByRole("button", { name: "Reload packages" }).click();
    await page.getByRole("button", { name: "Save configuration" }).click();
    await expect(page.locator(".change-workspace").getByRole("alert")).toContainText("Package settings changed");
    await expect(page.getByLabel("Bundle URL")).toHaveValue(fixture.httpUrl);
    const current = await page.evaluate(() => window.changes.invoke("packages", []));
    expect(current).toMatchObject({ packages: { workspace: { upstream: "https://example.org/newer.moo" } } });
});

async function historicalReview(page: Page) {
    await configure(page, "git");
    await page.evaluate(({ repository, commit }) =>
        window.changes.evaluate(
            `return $change_manager:upstream("acceptance", ["transport" -> "git", "repository" -> "${repository}", "revision" -> ["commit" -> "sha1:${commit}"], "path" -> "cores/sample/src"], $change_manager.packages["acceptance"]["generation"]);`,
        ), { repository: fixture.gitUrl, commit: fixture.revisionA });
    const adoption = await page.evaluate(async () =>
        (await window.changes.invoke("stage", ["acceptance", [], "adopt"]) as { review_id: number }).review_id
    );
    await expect.poll(() => page.evaluate(id => window.changes.client(id).status(), adoption)).toMatchObject({
        status: "ready",
    });
    await page.evaluate(id => window.changes.client(id).apply(1), adoption);
    await expect.poll(() => page.evaluate(id => window.changes.client(id).status(), adoption)).toMatchObject({
        status: "complete",
    });
    await page.evaluate(repository =>
        window.changes.evaluate(
            `return $change_manager:upstream("acceptance", ["transport" -> "git", "repository" -> "${repository}", "revision" -> ["ref" -> "refs/heads/main"], "path" -> "cores/sample/src"], $change_manager.packages["acceptance"]["generation"]);`,
        ), fixture.gitUrl);
    return stage(page);
}

test("historical source verifies uploads and pinned Git revisions, survives restart, and supports mixed baselines", async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", error => errors.push(error.message));
    await login(page);
    const review = await historicalReview(page);
    await choose(page, review, "edited", "edited", "return 9;");
    const generation = (await page.evaluate(id => window.changes.client(id).status(), review)).generation;
    await page.evaluate(({ review, generation }) => window.changes.open(review, generation), { review, generation });
    await page.getByRole("button", { name: /:edited, Verb/ }).click();
    await page.getByLabel("Upload accepted objdefs").setInputFiles({
        name: "wrong.moo",
        mimeType: "text/plain",
        buffer: Buffer.from(source("incoming")),
    });
    await expect(page.locator(".change-review-shell")).toContainText("does not match the accepted baseline");
    expect((await page.evaluate(id => window.changes.client(id).status(), review)).generation).toBe(generation);
    await page.getByRole("button", { name: "Fetch accepted source" }).click();
    await expect(page.getByText("Accepted source verified against this verb's baseline.")).toBeVisible();
    await expect(page.locator(".change-review-three-way")).toBeVisible();
    for (const name of ["Accepted baseline", "Local", "Upstream"]) {
        await expect(page.locator(".change-review-three-way").getByRole("region", { name, exact: true })).toBeVisible();
    }
    await page.getByRole("button", { name: "Edit resolution", exact: true }).click();
    const editor = page.locator(".change-review-draft").getByRole("textbox", { name: "Editor content", exact: true });
    await editor.focus();
    await editor.press("End");
    await editor.pressSequentially(" ");
    await expect(page.getByRole("button", { name: "Fetch accepted source" })).toBeDisabled();
    await page.getByRole("button", { name: "Save proposed program" }).click();
    await expect(page.getByText("Choice saved. Live code has not changed.")).toBeVisible();
    await page.getByRole("button", { name: /:deferred, Verb/ }).click();
    await page.getByLabel("Upload accepted objdefs").setInputFiles({
        name: "ancestor.moo",
        mimeType: "text/plain",
        buffer: Buffer.from(source("base")),
    });
    await expect(page.getByText("Accepted source verified against this verb's baseline.")).toBeVisible();
    await expect(
        page.locator(".change-review-three-way").getByRole("region", { name: "Accepted baseline", exact: true })
            .locator(".view-lines"),
    ).toContainText("return 1;");
    await fixture.restart();
    await login(page);
    const details = await page.evaluate(async id => {
        const client = window.changes.client(id);
        const status = await client.status();
        const row = (await client.page(status.generation)).rows.find(row => row.memberLabel === ":edited")!;
        return client.details(status.generation, row.id);
    }, review);
    expect(details).toMatchObject({
        base_text: "return 1;",
        live_text: "return 3;",
        incoming_text: "return 2;",
        choice: { choice: "edited", validated: true },
    });
    await choose(page, review, "conflict", "local");
    await choose(page, review, "deferred", "defer");
    await page.evaluate(async id => {
        const client = window.changes.client(id);
        await client.apply((await client.status()).generation);
    }, review);
    await expect.poll(() => page.evaluate(id => window.changes.client(id).status(), review)).toMatchObject({
        status: "partial",
    });
    const mixed = await page.evaluate(async id => {
        const client = window.changes.client(id);
        const values = [];
        for (const name of [":edited", ":deferred"]) {
            const status = await client.status();
            const row = (await client.page(status.generation)).rows.find(row => row.memberLabel === name)!;
            values.push((await client.baseSource(status.generation, row.id)).row.base_text);
        }
        return values;
    }, review);
    expect(mixed).toEqual(["return 2;", "return 1;"]);
    expect(await programs(page)).toEqual([2, 3, 3, 9, 3]);
    expect(errors).toEqual([]);
});

test("historical Git retrieval rechecks authority after worker suspension", async ({ page, context }) => {
    await login(page, "AcceptanceAdmin");
    const review = await historicalReview(page);
    const held = fixture.holdGit();
    const fetching = page.evaluate(async id => {
        const client = window.changes.client(id);
        const row = (await client.page(1)).rows.find(row => row.memberLabel === ":conflict")!;
        try {
            await client.baseSource(1, row.id);
            return "unexpected success";
        } catch {
            return "rejected";
        }
    }, review);
    await held.arrived;
    const administrator = await context.newPage();
    await login(administrator);
    await administrator.evaluate(() => window.changes.evaluate("#9502.wizard = false; return 1;"));
    held.release();
    expect(await fetching).toBe("rejected");
    expect(await administrator.evaluate(id => window.changes.client(id).status(), review)).toMatchObject({
        generation: 1,
        status: "ready",
    });
    expect(
        await administrator.evaluate(id =>
            window.changes.evaluate(
                `return maphaskey($change_manager.pending[${id}], "base_sources");`,
            ), review),
    ).toBe(false);
    expect(await programs(administrator)).toEqual([1, 3, 3, 3, 3]);
    await administrator.close();
});

test("a declared group installs a helper and property with its caller through Meadow", async ({ page }) => {
    const browserErrors: string[] = [];
    page.on("pageerror", error => browserErrors.push(error.message));
    await login(page);
    await page.evaluate(() => window.changes.evaluate("return $change_manager:configure(\"addition\", {#9500}, []);"));
    const input = "object #9500 [import_export_id -> \"changes_acceptance\"]\nowner: #2\n"
        + "verb upstream (this none this) owner: #2 flags: \"rxd\"\nreturn this:added_helper();\nendverb\n"
        + "verb added_helper (this none this) owner: #2 flags: \"rxd\"\nreturn this.item_schema;\nendverb\n"
        + "property item_schema (owner: #2, flags: \"r\") = 42;\nendobject";
    const review = await page.evaluate(
        async text =>
            (await window.changes.invoke("stage", ["addition", [{ label: "addition.moo", text }]]) as {
                review_id: number;
            }).review_id,
        input,
    );
    await page.evaluate(id => window.changes.open(id, 1), review);
    await page.getByRole("button", { name: /\.item_schema, Property/ }).click();
    await expect(page.getByRole("region", { name: "Property initialization" })).toContainText(
        "Existing data stays unchanged",
    );
    await page.getByRole("button", { name: "Permit initialization and clear choices", exact: true }).click();
    await expect(page.getByText("Absent-only initialization is already permitted.")).toBeVisible();
    await expect(page.getByRole("button", { name: /:added_helper, Verb/ })).toHaveCount(1);
    await page.getByRole("button", { name: /:added_helper, Verb/ }).click();
    await expect(page.getByRole("region", { name: "New verb definition" })).toContainText("Owner: #2");
    await expect(page.getByRole("region", { name: "New verb definition" })).toContainText("Flags: rxd");
    await expect(page.getByText("Declare which changes must be installed together before selecting this program."))
        .toBeVisible();
    await page.getByRole("button", { name: "Create a change group", exact: true }).click();
    const editor = page.getByRole("dialog", { name: "Create a change group", exact: true });
    await editor.getByLabel("Group name").fill("helper-update");
    await editor.getByRole("checkbox", { name: "#9500:added_helper — new verb", exact: true }).check();
    await editor.getByRole("checkbox", { name: "#9500:upstream — existing program", exact: true }).check();
    await editor.getByRole("checkbox", { name: "#9500 (Acceptance package).item_schema — new property", exact: true })
        .check();
    await editor.getByRole("button", { name: "Save group and clear choices", exact: true }).click();
    await expect(editor).toHaveCount(0);
    await page.getByRole("button", { name: "Select helper-update", exact: true }).click();
    await expect(page.getByRole("button", { name: "Apply 3 choices", exact: true })).toBeEnabled();
    expect(
        await page.evaluate(() =>
            window.changes.evaluate("return {#9500:upstream(), \"added_helper\" in verbs(#9500)};")
        ),
    ).toEqual([1, 0]);
    await page.getByRole("button", { name: "Apply 3 choices", exact: true }).click();
    await page.getByRole("button", { name: "Confirm apply", exact: true }).click();
    await expect.poll(() => page.evaluate(id => window.changes.client(id).status(), review)).toMatchObject({
        status: "complete",
    });
    expect(
        await page.evaluate(() =>
            window.changes.evaluate(
                "return {#9500:upstream(), #9500:added_helper(), length($change_manager.packages[\"addition\"][\"members\"])};",
            )
        ),
    ).toEqual([42, 42, 2]);
    expect(browserErrors).toEqual([]);
});
