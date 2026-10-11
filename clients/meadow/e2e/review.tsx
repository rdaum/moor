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

import "@vitejs/plugin-react/preamble";
import React from "react";
import "../src/styles/main.css";
import { buildStructuredArgs, type StructuredArgument } from "@moor/web-sdk";
import { createRoot } from "react-dom/client";
import { ChangeReview } from "../src/components/ChangeReview";
import { ChangeWorkspace } from "../src/components/ChangeWorkspace";
import { ThemeProvider } from "../src/components/ThemeProvider";
import { ChangeReviewClient, type ChangeTarget } from "../src/lib/change-review";
import { discoverChangeService } from "../src/lib/change-service";
import { invokeVerbFlatBuffer, performEvalFlatBuffer } from "../src/lib/rpc-fb";

let token = "";
const root = createRoot(document.getElementById("root")!);
const target = (review: number, generation = 1): ChangeTarget => ({
    kind: "change",
    provider: "oid:2000",
    review,
    generation,
});
const harness = {
    async login(player = "ArchWizard") {
        const response = await fetch("/auth/connect", {
            method: "POST",
            body: new URLSearchParams({ player, password: "test" }),
        });
        if (!response.ok) throw new Error(`Login failed: ${response.status}`);
        token = response.headers.get("X-Moor-Auth-Token")!;
    },
    async invoke(method: string, args: StructuredArgument[]) {
        return (await invokeVerbFlatBuffer(token, "oid:2000", method, buildStructuredArgs(args))).result;
    },
    evaluate(source: string) {
        return performEvalFlatBuffer(token, source);
    },
    client(review: number) {
        return new ChangeReviewClient(token, target(review));
    },
    async markers() {
        const monaco = await import("../src/lib/monaco");
        return monaco.editor.getModelMarkers({ owner: "change-review" }).map(marker => ({
            message: marker.message,
            line: marker.startLineNumber,
        }));
    },
    async workspace() {
        const service = await discoverChangeService(token);
        if (!service) throw new Error("No change service");
        root.render(
            <ThemeProvider>
                <ChangeWorkspace
                    visible
                    service={service}
                    authToken={token}
                    isWizard
                    onClose={() => root.render(null)}
                />
            </ThemeProvider>,
        );
    },
    open(review: number, generation = 1) {
        root.render(
            <ThemeProvider>
                <ChangeReview
                    key={`${review}:${generation}`}
                    visible
                    target={target(review, generation)}
                    authToken={token}
                    onClose={() => root.render(null)}
                />
            </ThemeProvider>,
        );
    },
};
declare global {
    interface Window {
        changes: typeof harness;
    }
}
window.changes = harness;
