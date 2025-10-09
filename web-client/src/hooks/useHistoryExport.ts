// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

// Hook for exporting event history using a Web Worker

import { useCallback, useRef, useState } from "react";
import type {
    CompleteMessage,
    ErrorMessage,
    ProgressMessage,
    StartExportMessage,
} from "../workers/historyExportWorker";

interface ExportProgress {
    processed: number;
    total?: number;
}

interface ExportState {
    isExporting: boolean;
    progress: ExportProgress | null;
    error: string | null;
    readyBlob: Blob | null; // Blob ready for download
    readyFilename: string | null; // Filename for the ready blob
}

export const useHistoryExport = () => {
    const [exportState, setExportState] = useState<ExportState>({
        isExporting: false,
        progress: null,
        error: null,
        readyBlob: null,
        readyFilename: null,
    });

    const workerRef = useRef<Worker | null>(null);

    const startExport = useCallback(async (
        authToken: string,
        ageIdentity: string,
        systemTitle: string,
        playerOid: string,
    ): Promise<void> => {
        // Clean up any existing worker
        if (workerRef.current) {
            workerRef.current.terminate();
            workerRef.current = null;
        }

        return new Promise<void>((resolve, reject) => {
            try {
                // Create worker
                const worker = new Worker(
                    new URL("../workers/historyExportWorker.ts", import.meta.url),
                    { type: "module" },
                );
                workerRef.current = worker;

                setExportState({
                    isExporting: true,
                    progress: { processed: 0 },
                    error: null,
                    readyBlob: null,
                    readyFilename: null,
                });

                // Handle worker messages
                worker.onmessage = (event: MessageEvent<ProgressMessage | ErrorMessage | CompleteMessage>) => {
                    const message = event.data;

                    if (message.type === "progress") {
                        setExportState((prev) => ({
                            ...prev,
                            progress: {
                                processed: message.processed,
                                total: message.total,
                            },
                        }));
                    } else if (message.type === "error") {
                        setExportState({
                            isExporting: false,
                            progress: null,
                            error: message.error,
                            readyBlob: null,
                            readyFilename: null,
                        });
                        worker.terminate();
                        workerRef.current = null;
                        reject(new Error(message.error));
                    } else if (message.type === "complete") {
                        // Store the blob and filename - don't auto-download
                        const sanitizedTitle = systemTitle.toLowerCase().replace(/[^a-z0-9]+/g, "-");
                        const filename = `${sanitizedTitle}-history-${new Date().toISOString().split("T")[0]}.json`;

                        setExportState({
                            isExporting: false,
                            progress: null,
                            error: null,
                            readyBlob: message.jsonBlob,
                            readyFilename: filename,
                        });
                        worker.terminate();
                        workerRef.current = null;
                        resolve();
                    }
                };

                worker.onerror = (error) => {
                    setExportState({
                        isExporting: false,
                        progress: null,
                        error: error.message || "Worker error",
                        readyBlob: null,
                        readyFilename: null,
                    });
                    worker.terminate();
                    workerRef.current = null;
                    reject(error);
                };

                // Start the export
                const message: StartExportMessage = {
                    type: "start",
                    authToken,
                    ageIdentity,
                    systemTitle,
                    playerOid,
                };
                worker.postMessage(message);
            } catch (error) {
                setExportState({
                    isExporting: false,
                    progress: null,
                    error: error instanceof Error ? error.message : "Unknown error",
                    readyBlob: null,
                    readyFilename: null,
                });
                reject(error);
            }
        });
    }, []);

    const cancelExport = useCallback(() => {
        if (workerRef.current) {
            workerRef.current.terminate();
            workerRef.current = null;
        }
        setExportState({
            isExporting: false,
            progress: null,
            error: null,
            readyBlob: null,
            readyFilename: null,
        });
    }, []);

    const downloadReady = useCallback(() => {
        if (!exportState.readyBlob || !exportState.readyFilename) {
            return;
        }

        // Trigger the download
        const url = URL.createObjectURL(exportState.readyBlob);
        const a = document.createElement("a");
        a.href = url;
        a.download = exportState.readyFilename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);

        // Clear the ready state after download
        setExportState((prev) => ({
            ...prev,
            readyBlob: null,
            readyFilename: null,
        }));
    }, [exportState.readyBlob, exportState.readyFilename]);

    const dismissReady = useCallback(() => {
        // Clear the ready state without downloading
        setExportState((prev) => ({
            ...prev,
            readyBlob: null,
            readyFilename: null,
        }));
    }, []);

    return {
        exportState,
        startExport,
        cancelExport,
        downloadReady,
        dismissReady,
    };
};
