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

/**
 * Model Layer
 *
 * This module contains the core data models and types for the Moor client application.
 * It's organized into several distinct areas of concern:
 *
 * 1. Domain Models - Core business entities (Player, Spool, etc.)
 * 2. UI State Models - Components for managing UI state (Notice, PresentationManager)
 * 3. Event Types - Types for server-client communication
 * 4. Application Context - Global state container
 *
 * The models here are designed to be reactive using VanJS State objects where
 * appropriate, and provide a clean separation between data and presentation logic.
 */

import van, { State } from "vanjs-core";
import { ObjectRef } from "./var";

// ============================================================================
// Domain Models
// ============================================================================

/**
 * Represents a player/user session in the Moor system.
 *
 * Contains the essential identity and authentication information needed
 * to interact with the server and maintain session state.
 */
export class Player {
    /** Whether the player is currently connected to the server */
    readonly connected: boolean;

    /** The unique object identifier for this player in the Moor system */
    readonly oid: string;

    /** Authentication token used for server requests */
    readonly auth_token: string;

    constructor(oid: string, auth_token: string, connected: boolean) {
        this.oid = oid;
        this.auth_token = auth_token;
        this.connected = connected;
    }

    /**
     * Creates a new Player instance with updated connection status
     */
    withConnection(connected: boolean): Player {
        return new Player(this.oid, this.auth_token, connected);
    }
}

/**
 * Types of content that can be stored in a Spool for editing operations
 */
export enum SpoolType {
    /** Verb/method code editing */
    Verb = "verb",
    /** Property value editing */
    Property = "property",
}

/**
 * A Spool represents a temporary buffer for collecting and editing content
 * sent from the server, typically used for MCP-style editing operations.
 *
 * The spool accumulates lines of text that represent code or data being
 * edited, and provides mechanisms to upload the changes back to the server.
 */
export class Spool {
    /** The type of content being edited */
    readonly type: SpoolType;

    /** Human-readable name for this editing session */
    readonly name: string;

    /** The object reference this spool is associated with */
    readonly object: ObjectRef;

    /** The specific entity (verb name, property name) being edited */
    readonly entity: string;

    /** Command template for uploading the edited content */
    readonly uploadAction: string;

    /** Buffer containing the accumulated lines of content */
    private content: Array<string>;

    constructor(type: SpoolType, name: string, object: ObjectRef, entity: string, uploadAction: string) {
        this.type = type;
        this.name = name;
        this.object = object;
        this.entity = entity;
        this.uploadAction = uploadAction;
        this.content = [];
    }

    /**
     * Appends a line of content to the spool buffer
     */
    append(line: string): void {
        this.content.push(line);
    }

    /**
     * Retrieves all content from the spool and clears the buffer
     * @returns Array of content lines that were in the spool
     */
    take(): Array<string> {
        const content = [...this.content];
        this.content = [];
        return content;
    }

    /**
     * Gets a copy of the current content without clearing the buffer
     */
    peek(): Array<string> {
        return [...this.content];
    }

    /**
     * Returns the number of lines currently in the spool
     */
    size(): number {
        return this.content.length;
    }
}

// ============================================================================
// UI State Models
// ============================================================================

/**
 * Manages temporary notification messages in the UI.
 *
 * Provides a reactive notification system that can display messages
 * and automatically hide them after a specified duration.
 */
export class Notice {
    /** The current message content */
    readonly message: State<string>;

    /** Whether the notice is currently visible */
    readonly visible: State<boolean>;

    constructor() {
        this.message = van.state("");
        this.visible = van.state(false);
    }

    /**
     * Displays a message for a specified duration
     * @param message - Text content to display
     * @param duration - How long to show the message (in seconds)
     */
    show(message: string, duration: number): void {
        this.message.val = message;
        this.visible.val = true;

        setTimeout(() => {
            this.visible.val = false;
        }, duration * 1000);
    }

    /**
     * Immediately hides the current notice
     */
    hide(): void {
        this.visible.val = false;
    }
}

/**
 * Represents a UI presentation that can be displayed in various targets
 * (windows, panels, etc.).
 */
export interface PresentationModel {
    /** Unique identifier for this presentation */
    readonly id: string;

    /** Where the presentation should be displayed (window, right-dock, etc.) */
    readonly target: string;

    /** Whether the presentation is currently closed/hidden */
    readonly closed: State<boolean>;

    /** The rendered DOM content for this presentation */
    readonly content: HTMLElement;

    /** Additional attributes controlling presentation behavior */
    readonly attrs: Readonly<{ [key: string]: string }>;
}

/**
 * Manages a collection of UI presentations with support for different display targets.
 *
 * Provides methods to add, remove, and query presentations based on their
 * display target (e.g., floating windows, right dock panels, etc.).
 */
export class PresentationManager {
    private readonly presentations: Readonly<{ [key: string]: State<PresentationModel> }>;

    constructor(presentations: { [key: string]: State<PresentationModel> } = {}) {
        this.presentations = Object.freeze({ ...presentations });
    }

    /**
     * Creates a new manager with an additional presentation
     */
    withAdded(id: string, model: State<PresentationModel>): PresentationManager {
        return new PresentationManager({
            ...this.presentations,
            [id]: model,
        });
    }

    /**
     * Creates a new manager with a presentation removed
     */
    withRemoved(id: string): PresentationManager {
        const updated = { ...this.presentations };
        delete updated[id];
        return new PresentationManager(updated);
    }

    /**
     * Gets all presentation IDs for a specific display target
     */
    getPresentationsForTarget(target: string): string[] {
        return Object.entries(this.presentations)
            .filter(([_, presentation]) => presentation.val.target === target)
            .map(([id, _]) => id);
    }

    /**
     * Gets presentations specifically for the right dock panel
     */
    rightDockPresentations(): string[] {
        return this.getPresentationsForTarget("right-dock");
    }

    /**
     * Retrieves a specific presentation by ID
     */
    getPresentation(id: string): State<PresentationModel> | undefined {
        return this.presentations[id];
    }

    /**
     * Gets all presentations as a readonly collection
     */
    getAllPresentations(): Readonly<{ [key: string]: State<PresentationModel> }> {
        return this.presentations;
    }

    /**
     * Checks if a presentation with the given ID exists
     */
    hasPresentation(id: string): boolean {
        return id in this.presentations;
    }
}

// ============================================================================
// Event Types for Server Communication
// ============================================================================

/**
 * Categories of events that can be received from the server
 */
export enum EventKind {
    /** System notifications, errors, and status messages */
    SystemMessage = "system_message",
    /** Game narrative, command output, and interactive content */
    NarrativeMessage = "narrative_message",
}

/**
 * Base interface that all server events must implement
 */
export interface BaseEvent {
    /** The category/type of this event */
    readonly kind: EventKind;

    /** When this event was generated (optional) */
    readonly timestamp?: number;
}

/**
 * Represents content in the main narrative stream (game output, descriptions, etc.)
 */
export interface NarrativeEvent extends BaseEvent {
    readonly kind: EventKind.NarrativeMessage;

    /** The actual content/text of the message */
    readonly message: string;

    /** MIME type of the content (text/plain, text/html, text/djot, etc.) */
    readonly content_type: string | null;

    /** Who or what generated this message */
    readonly author: string;
}

/**
 * Represents system-level messages (errors, notifications, status updates)
 */
export interface SystemEvent extends BaseEvent {
    readonly kind: EventKind.SystemMessage;

    /** The system message content */
    readonly system_message: string;
}

/**
 * Raw presentation data as received from the server before processing
 */
export interface PresentationData {
    /** Unique identifier for the presentation */
    readonly id: string;

    /** MIME type of the presentation content */
    readonly content_type: string;

    /** Raw content string to be rendered */
    readonly content: string;

    /** Where the presentation should be displayed */
    readonly target: string;

    /** Additional attributes as key-value pairs */
    readonly attributes: ReadonlyArray<readonly [string, string]>;
}

/**
 * Error information with detailed traceback for debugging
 */
export interface Traceback {
    /** Human-readable error message */
    readonly msg: string;

    /** Error code or identifier */
    readonly code: string;

    /** Stack trace or execution path leading to the error */
    readonly traceback: ReadonlyArray<string>;
}

// Legacy alias for PresentationData - kept for backward compatibility
export interface Presentation extends PresentationData {}

// ============================================================================
// Application Context
// ============================================================================

/**
 * Global application context that holds all session state.
 *
 * This serves as the central coordination point for the application,
 * containing network connections, user state, UI state, and temporary
 * data like editing spools.
 *
 * The context is designed to be passed to components that need access
 * to global state, avoiding prop drilling while maintaining clear
 * dependencies.
 */
export class Context {
    /** Active WebSocket connection to the server (null when disconnected) */
    ws: WebSocket | null;

    /** Command history for input field navigation */
    readonly history: string[];

    /** Current position when navigating through command history */
    historyOffset: number;

    /** Authentication token for server requests */
    authToken: string | null;

    /** System notification manager for toast messages */
    readonly systemMessage: Notice;

    /** Current player/user information */
    player: Player;

    /** Active editing session (null when not editing) */
    spool: Spool | null;

    /** Manager for UI presentations (windows, panels, etc.) */
    readonly presentations: State<PresentationManager>;

    constructor() {
        this.ws = null;
        this.history = [];
        this.historyOffset = 0;
        this.authToken = null;
        this.systemMessage = new Notice();
        this.player = new Player("", "", false);
        this.spool = null;
        this.presentations = van.state(new PresentationManager());
    }

    /**
     * Updates the player information and triggers any reactive updates
     */
    setPlayer(player: Player): void {
        this.player = player;
    }

    /**
     * Adds a command to the history buffer
     */
    addToHistory(command: string): void {
        this.history.push(command);
        this.historyOffset = 0; // Reset to latest when new command is added
    }

    /**
     * Clears the command history
     */
    clearHistory(): void {
        this.history.length = 0;
        this.historyOffset = 0;
    }

    /**
     * Checks if the client is currently connected to the server
     */
    isConnected(): boolean {
        return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
    }

    /**
     * Checks if there's an active editing session
     */
    isEditing(): boolean {
        return this.spool !== null;
    }
}
