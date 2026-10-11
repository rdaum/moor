import assert from "node:assert/strict";
import { once } from "node:events";
import { createConnection, type Socket } from "node:net";
import { ChangesFixture } from "../../../../clients/meadow/e2e/changes-fixture.ts";
import { MoorWebClient } from "../../../../clients/moor-web-mcp/src/moor-service.ts";

class Telnet {
    private buffer = "";
    private closed = false;
    private socket: Socket;
    private constructor(port: number) {
        this.socket = createConnection({ host: "127.0.0.1", port });
        this.socket.on("data", bytes => {
            // This test does not negotiate GMCP, HTML, or any browser features.
            this.buffer += bytes.toString("utf8").replace(/\x1b\[[0-9;]*[a-zA-Z]/g, "").replaceAll("\r", "");
        });
        this.socket.on("close", () => this.closed = true);
    }
    static async connect(port: number) {
        const connection = new Telnet(port);
        await once(connection.socket, "connect");
        connection.send("connect ArchWizard test");
        await connection.expect("*** Connected ***");
        return connection;
    }
    send(line: string) {
        this.socket.write(line + "\r\n");
    }
    async expect(text: string, timeout = 20000) {
        const deadline = Date.now() + timeout;
        while (true) {
            const end = this.buffer.indexOf(text);
            if (end >= 0) {
                const output = this.buffer.slice(0, end + text.length);
                this.buffer = this.buffer.slice(end + text.length);
                assert.ok(!output.includes("Traceback"), output);
                assert.ok(!output.includes("<table") && !output.includes("<a "), output);
                return output;
            }
            assert.ok(!this.closed && Date.now() < deadline, `Waiting for ${text}:\n${this.buffer}`);
            await new Promise(done => setTimeout(done, 20));
        }
    }
    async command(line: string, expected: string) {
        this.send(line);
        return this.expect(expected);
    }
    close() {
        this.socket.destroy();
    }
}

const fixture = new ChangesFixture();
const connections: Telnet[] = [];
let assertions: MoorWebClient | undefined;
try {
    await fixture.start({ distributed: true, browser: false, extraMembers: 65 });
    assertions = new MoorWebClient({
        moos: [{
            id: "test",
            description: "Telnet acceptance world",
            connectAddress: `http://127.0.0.1:${fixture.apiPort}`,
            players: [{ id: "wizard", username: "ArchWizard", password: "test", isWizard: true }],
        }],
    });
    const actor = { mooId: "test", playerId: "wizard" };
    let terminal = await Telnet.connect(fixture.telnetPort);
    connections.push(terminal);
    await terminal.command("@changes package acceptance #9500 #9501", "configured at generation");
    const git = process.argv.includes("--git");
    const review = git ? 2 : 1;
    if (git) {
        // Establish the known historical source through the terminal adoption flow.
        await terminal.command(
            `@changes upstream acceptance git ${fixture.gitUrl} sha1:${fixture.revisionA} cores/sample/src`,
            "Git upstream configured.",
        );
        await terminal.command("@changes adopt acceptance", "Checking acceptance for updates");
        await terminal.expect("Review changes");
        await terminal.command("@changes diff", "Discard review");
        await terminal.command("@changes preview", "Apply these choices");
        await terminal.command("@changes apply", "Applying changes to acceptance");
        await terminal.expect("Changes applied.");
    }
    await terminal.command(
        git
            ? `@changes upstream acceptance git ${fixture.gitUrl} refs/heads/main cores/sample/src`
            : `@changes upstream acceptance ${fixture.httpUrl}`,
        git ? "Git upstream configured." : "HTTP upstream configured.",
    );
    await terminal.command("@changes fetch acceptance", "Checking acceptance for updates");
    await terminal.expect("Review changes");
    await terminal.command(`@changes diff ${review}`, "Conflict");
    await terminal.expect("Discard review");
    for (let page = 0; page < 3; page++) await terminal.command("@changes next", "Discard review");
    if (git) {
        await terminal.command("@changes base 5", "Review changes");
        await terminal.command("@changes source 5 base", "1: return 1;");
    }
    const comparison = await terminal.command("@changes compare 5", "leave both code and baseline unchanged.");
    assert.match(comparison, /- local 1: return 3;/);
    assert.match(comparison, /\+ upstream 1: return 2;/);
    await terminal.command("@changes keep 5", "Keep my code saved for");
    await terminal.command("@changes later 7", "Decide later saved for");
    await terminal.command("@changes edit 6", "Prefix a literal dot with another dot.");
    terminal.send("const value = \"#9500\";");
    await terminal.command("@abort", "Live code and the review choice are unchanged.");
    await terminal.command("@changes draft 1", "1: const value = \"#9500\";");
    await terminal.command("@changes edit 6", "Prefix a literal dot with another dot.");
    terminal.send("return {value, 42};");
    await terminal.command(".show", "2: return {value, 42};");
    terminal.close();
    terminal = await Telnet.connect(fixture.telnetPort);
    connections.push(terminal);
    await terminal.command("@changes drafts", "2 lines.");
    await terminal.command(`@changes diff ${review}`, "Discard review");
    await terminal.command("@changes edit 6", "Prefix a literal dot with another dot.");
    await terminal.command(".", "The edited program compiles.");
    const before = await assertions.evalExpression(actor, "return #9500:edited();");
    assert.equal(before.js, 3, "Saving a draft must not program the verb");

    // Another terminal changes the review while the first terminal is editing.
    const other = await Telnet.connect(fixture.telnetPort);
    connections.push(other);
    await terminal.command("@changes edit 6", "Prefix a literal dot with another dot.");
    terminal.send(".clear");
    terminal.send("return (;");
    await other.command(`@changes diff ${review}`, "Discard review");
    await other.command("@changes incoming 2", "Use upstream saved for");
    await terminal.command(".", "The choice was not saved:");
    await terminal.expect("remains available.");
    await terminal.command(`@changes diff ${review}`, "Discard review");
    await terminal.command("@changes edit 6", "Prefix a literal dot with another dot.");
    await terminal.command(".", "Program error at line");
    await terminal.command("@changes edit 6", "Prefix a literal dot with another dot.");
    terminal.send(".clear");
    terminal.send("const value = \"#9500\";");
    terminal.send("return {value, 42};");
    await terminal.command(".", "The edited program compiles.");
    await terminal.command("@changes preview", "Apply these choices");
    await terminal.command("@changes apply", "Applying changes to acceptance");
    await terminal.expect("Changes applied. Some were skipped.");
    await terminal.command("@changes status", "Review remaining changes");
    await terminal.command("@changes result", "recorded decisions.");
    const remaining = await terminal.command(`@changes result ${review} 51`, "extra_65");
    assert.ok(remaining.includes("Replace program"), remaining);
    await terminal.command("@changes finish", "Review closed.");
    await terminal.command("@changes draft 1", "2: return {value, 42};");
    await terminal.command("@changes forget-draft 1", "Saved review choices and live code are unchanged.");
    await terminal.command("@changes draft 1", "No saved draft with that ID belongs to you.");
    await terminal.command("@changes history acceptance", "There is no general rollback command.");
    const after = await assertions.evalExpression(
        actor,
        "return {#9500:upstream(), #9500:conflict(), #9500:edited(), #9500:deferred()};",
    );
    assert.deepEqual(after.js, [2, 3, ["#9500", 42], 3]);
    if (!git) {
        fixture.httpFailure = true;
        await terminal.command("@changes fetch acceptance", "Checking acceptance for updates");
        await terminal.expect("Fetch failed:");
        await terminal.command("@changes status", "Discard failed review");
        fixture.httpFailure = false;
        await terminal.command("@changes retry", "Checking acceptance for updates");
        await terminal.expect("Review changes");
    } else {
        await terminal.command("@changes fetch acceptance", "Checking acceptance for updates");
        await terminal.expect("Review changes");
    }
    await terminal.command("@changes diff", "Discard review");
    console.log(
        "PASS plain telnet: review, comparison, saved draft, reconnect, stale editor, compiler repair, apply, finish, history",
    );
} finally {
    for (const connection of connections) connection.close();
    await assertions?.close();
    await fixture.close();
}
