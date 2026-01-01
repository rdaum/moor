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

import React, { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { renderDjot, renderPlainText } from "../lib/djot-renderer";
import { MoorVar } from "../lib/MoorVar";
import { InputMetadata } from "../types/input";

interface RichInputPromptProps {
    metadata: InputMetadata;
    onSubmit: (value: string | Uint8Array) => void;
    disabled?: boolean;
}

export const RichInputPrompt: React.FC<RichInputPromptProps> = ({
    metadata,
    onSubmit,
    disabled = false,
}) => {
    const [value, setValue] = useState<string>(() => {
        if (metadata.default !== undefined) {
            return String(metadata.default);
        }
        return "";
    });

    const [showAlternative, setShowAlternative] = useState(false);
    const [selectedFile, setSelectedFile] = useState<File | null>(null);
    const [previewUrl, setPreviewUrl] = useState<string | null>(null);
    const [fileError, setFileError] = useState<string | null>(null);
    const baseId = useId();
    const textInputId = `${baseId}-text`;
    const textAreaId = `${baseId}-textarea`;
    const numberInputId = `${baseId}-number`;
    const choiceSelectId = `${baseId}-choice`;
    const alternativeInputId = `${baseId}-alternative`;
    const alternativeDescriptionId = `${baseId}-alternative-description`;
    const promptStatusId = `${baseId}-prompt-status`;
    const fileInputId = `${baseId}-file`;
    const trimmedValue = value.trim();
    const primaryButtonRef = useRef<HTMLButtonElement>(null);
    const alternativeButtonRef = useRef<HTMLButtonElement>(null);
    const alternativeTextareaRef = useRef<HTMLTextAreaElement>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);

    // Auto-focus the primary button when the component mounts or when returning from alternative view
    useEffect(() => {
        if (!disabled && primaryButtonRef.current) {
            // Small delay to ensure DOM is ready and screen reader announcements complete
            const timer = setTimeout(() => {
                primaryButtonRef.current?.focus();
            }, 100);
            return () => clearTimeout(timer);
        }
    }, [disabled, showAlternative, metadata.input_type]);

    // Render prompt text as djot with ANSI escape codes
    const renderPrompt = useCallback((text: string) => {
        try {
            // Use centralized djot rendering utility
            // For prompts, we don't need clickable links, so no linkHandler
            const processedHtml = renderDjot(text);

            return <div dangerouslySetInnerHTML={{ __html: processedHtml }} />;
        } catch (error) {
            // Fallback to plain text with ANSI if djot parsing fails
            console.warn("Failed to parse djot content in prompt:", error);
            const plainHtml = renderPlainText(text);
            return <div dangerouslySetInnerHTML={{ __html: plainHtml }} />;
        }
    }, []);

    const handleSubmit = useCallback((submitValue: string) => {
        onSubmit(submitValue);
        setValue("");
    }, [onSubmit]);

    const submitCurrentValue = useCallback(() => {
        if (!trimmedValue) {
            return;
        }
        handleSubmit(trimmedValue);
    }, [handleSubmit, trimmedValue]);

    const submitAlternativeValue = useCallback(() => {
        if (!trimmedValue) {
            return;
        }
        handleSubmit(`alternative: ${trimmedValue}`);
    }, [handleSubmit, trimmedValue]);

    const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
        if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            submitCurrentValue();
        }
    }, [submitCurrentValue]);

    // Handle file selection for image/file input types
    const handleFileSelect = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        setFileError(null);

        if (!file) {
            setSelectedFile(null);
            setPreviewUrl(null);
            return;
        }

        // Validate content type if restrictions are specified
        if (metadata.accept_content_types && metadata.accept_content_types.length > 0) {
            const isAccepted = metadata.accept_content_types.some(type => {
                if (type.endsWith("/*")) {
                    // Handle wildcards like "image/*"
                    const prefix = type.slice(0, -1);
                    return file.type.startsWith(prefix);
                }
                return file.type === type;
            });
            if (!isAccepted) {
                setFileError(
                    `File type ${file.type} is not accepted. Allowed: ${metadata.accept_content_types.join(", ")}`,
                );
                return;
            }
        }

        // Validate file size
        if (metadata.max_file_size && file.size > metadata.max_file_size) {
            const maxSizeMB = (metadata.max_file_size / (1024 * 1024)).toFixed(1);
            setFileError(`File is too large. Maximum size: ${maxSizeMB} MB`);
            return;
        }

        setSelectedFile(file);

        // Create preview URL for images
        if (file.type.startsWith("image/")) {
            const url = URL.createObjectURL(file);
            setPreviewUrl(url);
        } else {
            setPreviewUrl(null);
        }
    }, [metadata.accept_content_types, metadata.max_file_size]);

    // Submit the selected file
    const submitFile = useCallback(async () => {
        if (!selectedFile) {
            return;
        }

        try {
            const arrayBuffer = await selectedFile.arrayBuffer();
            const data = new Uint8Array(arrayBuffer);
            const varBytes = MoorVar.buildFileVar(selectedFile.type, data);
            onSubmit(varBytes);

            // Clean up
            setSelectedFile(null);
            if (previewUrl) {
                URL.revokeObjectURL(previewUrl);
                setPreviewUrl(null);
            }
        } catch (error) {
            console.error("Failed to read file:", error);
            setFileError("Failed to read file");
        }
    }, [selectedFile, previewUrl, onSubmit]);

    // Clean up preview URL when component unmounts
    useEffect(() => {
        return () => {
            if (previewUrl) {
                URL.revokeObjectURL(previewUrl);
            }
        };
    }, [previewUrl]);

    const promptStatus = useMemo(() => {
        if (!metadata.prompt) {
            return null;
        }

        // If tts_prompt is provided, use sr-only for screen readers and aria-hidden for visual
        if (metadata.tts_prompt) {
            return (
                <div
                    className="rich_input_prompt_text"
                    role="status"
                    aria-live="polite"
                    id={promptStatusId}
                    tabIndex={-1}
                >
                    <span className="sr-only">{metadata.tts_prompt}</span>
                    <span aria-hidden="true">{renderPrompt(metadata.prompt)}</span>
                </div>
            );
        }

        return (
            <div className="rich_input_prompt_text" role="status" aria-live="polite" id={promptStatusId} tabIndex={-1}>
                {renderPrompt(metadata.prompt)}
            </div>
        );
    }, [metadata.prompt, metadata.tts_prompt, renderPrompt, promptStatusId]);

    const renderPromptLabel = useCallback((targetId: string) => {
        if (!metadata.prompt) {
            return null;
        }

        // If tts_prompt is provided, use sr-only for screen readers and aria-hidden for visual
        if (metadata.tts_prompt) {
            return (
                <label htmlFor={targetId} className="rich_input_prompt_text">
                    <span className="sr-only">{metadata.tts_prompt}</span>
                    <span aria-hidden="true">{renderPrompt(metadata.prompt)}</span>
                </label>
            );
        }

        return (
            <label htmlFor={targetId} className="rich_input_prompt_text">
                {renderPrompt(metadata.prompt)}
            </label>
        );
    }, [metadata.prompt, metadata.tts_prompt, renderPrompt]);

    // Yes/No/Alternative input type (for coding agents, etc.)
    if (metadata.input_type === "yes_no_alternative") {
        if (showAlternative) {
            return (
                <div className="rich_input_prompt" role="form" aria-label="Provide alternative response">
                    <div className="sr-only" role="status" aria-live="polite">
                        Alternative response form is now active
                    </div>
                    {promptStatus}
                    <div className="rich_input_buttons">
                        <button
                            type="button"
                            className="rich_input_button"
                            onClick={() => handleSubmit("yes")}
                            disabled={disabled}
                            aria-label="Yes"
                        >
                            Yes
                        </button>
                        <button
                            type="button"
                            className="rich_input_button"
                            onClick={() => handleSubmit("no")}
                            disabled={disabled}
                            aria-label="No"
                        >
                            No
                        </button>
                        <button
                            type="button"
                            className="rich_input_button"
                            onClick={() => setShowAlternative(false)}
                            disabled={disabled}
                            aria-label="Cancel alternative"
                        >
                            Cancel
                        </button>
                    </div>
                    <div
                        className="rich_input_alternative_container"
                        role="group"
                        aria-labelledby={`${baseId}-alternative-label`}
                    >
                        <label
                            htmlFor={alternativeInputId}
                            className="rich_input_prompt_text"
                            id={`${baseId}-alternative-label`}
                        >
                            {renderPrompt(metadata.alternative_label || "Describe your alternative:")}
                        </label>
                        <div id={alternativeDescriptionId} className="sr-only">
                            {metadata.alternative_placeholder
                                || "Enter your alternative suggestion in the text area below. Press Ctrl+Enter to submit."}
                        </div>
                        <div className="rich_input_text_container">
                            <textarea
                                ref={alternativeTextareaRef}
                                id={alternativeInputId}
                                className="rich_input_textarea"
                                value={value}
                                onChange={(e) => setValue(e.target.value)}
                                onKeyDown={(e) => {
                                    if (e.key === "Enter" && e.ctrlKey) {
                                        e.preventDefault();
                                        submitAlternativeValue();
                                    }
                                }}
                                placeholder={metadata.alternative_placeholder || "Enter your alternative..."}
                                disabled={disabled}
                                aria-label={metadata.alternative_label || "Alternative suggestion"}
                                aria-describedby={alternativeDescriptionId}
                                rows={4}
                                autoFocus
                            />
                        </div>
                        <button
                            type="button"
                            className="rich_input_button rich_input_button_primary"
                            onClick={submitAlternativeValue}
                            disabled={disabled || !trimmedValue}
                            aria-label="Submit alternative"
                        >
                            Submit Alternative
                        </button>
                    </div>
                </div>
            );
        }

        return (
            <div className="rich_input_prompt" role="form" aria-label="Respond with yes, no, or alternative">
                <div className="sr-only" role="status" aria-live="polite">
                    Response required: choose yes, no, or provide an alternative
                </div>
                {promptStatus}
                <div className="rich_input_buttons">
                    <button
                        ref={primaryButtonRef}
                        type="button"
                        className="rich_input_button rich_input_button_primary"
                        onClick={() => handleSubmit("yes")}
                        disabled={disabled}
                        aria-label="Yes"
                    >
                        Yes
                    </button>
                    <button
                        type="button"
                        className="rich_input_button"
                        onClick={() => handleSubmit("no")}
                        disabled={disabled}
                        aria-label="No"
                    >
                        No
                    </button>
                    <button
                        type="button"
                        className="rich_input_button"
                        onClick={() => {
                            setShowAlternative(true);
                        }}
                        disabled={disabled}
                        aria-label="Suggest alternative"
                        aria-expanded={false}
                        ref={alternativeButtonRef}
                    >
                        Alternative...
                    </button>
                </div>
            </div>
        );
    }

    // Yes/No/Alternative/All input type (for LLM wearable confirmations with "accept all" option)
    if (metadata.input_type === "yes_no_alternative_all") {
        if (showAlternative) {
            return (
                <div className="rich_input_prompt" role="form" aria-label="Provide alternative response">
                    <div className="sr-only" role="status" aria-live="polite">
                        Alternative response form is now active
                    </div>
                    {promptStatus}
                    <div className="rich_input_buttons">
                        <button
                            type="button"
                            className="rich_input_button"
                            onClick={() => handleSubmit("yes")}
                            disabled={disabled}
                            aria-label="Yes"
                        >
                            Yes
                        </button>
                        <button
                            type="button"
                            className="rich_input_button"
                            onClick={() => handleSubmit("no")}
                            disabled={disabled}
                            aria-label="No"
                        >
                            No
                        </button>
                        <button
                            type="button"
                            className="rich_input_button"
                            onClick={() => setShowAlternative(false)}
                            disabled={disabled}
                            aria-label="Cancel alternative"
                        >
                            Cancel
                        </button>
                    </div>
                    <div
                        className="rich_input_alternative_container"
                        role="group"
                        aria-labelledby={`${baseId}-alternative-label`}
                    >
                        <label
                            htmlFor={alternativeInputId}
                            className="rich_input_prompt_text"
                            id={`${baseId}-alternative-label`}
                        >
                            {renderPrompt(metadata.alternative_label || "Describe your alternative:")}
                        </label>
                        <div id={alternativeDescriptionId} className="sr-only">
                            {metadata.alternative_placeholder
                                || "Enter your alternative suggestion in the text area below. Press Ctrl+Enter to submit."}
                        </div>
                        <div className="rich_input_text_container">
                            <textarea
                                ref={alternativeTextareaRef}
                                id={alternativeInputId}
                                className="rich_input_textarea"
                                value={value}
                                onChange={(e) => setValue(e.target.value)}
                                onKeyDown={(e) => {
                                    if (e.key === "Enter" && e.ctrlKey) {
                                        e.preventDefault();
                                        submitAlternativeValue();
                                    }
                                }}
                                placeholder={metadata.alternative_placeholder || "Enter your alternative..."}
                                disabled={disabled}
                                aria-label={metadata.alternative_label || "Alternative suggestion"}
                                aria-describedby={alternativeDescriptionId}
                                rows={4}
                                autoFocus
                            />
                        </div>
                        <button
                            type="button"
                            className="rich_input_button rich_input_button_primary"
                            onClick={submitAlternativeValue}
                            disabled={disabled || !trimmedValue}
                            aria-label="Submit alternative"
                        >
                            Submit Alternative
                        </button>
                    </div>
                </div>
            );
        }

        return (
            <div
                className="rich_input_prompt"
                role="form"
                aria-label="Respond with yes, yes to all, no, or alternative"
            >
                <div className="sr-only" role="status" aria-live="polite">
                    Response required: choose yes, yes to all, no, or provide an alternative
                </div>
                {promptStatus}
                <div className="rich_input_buttons">
                    <button
                        ref={primaryButtonRef}
                        type="button"
                        className="rich_input_button rich_input_button_primary"
                        onClick={() => handleSubmit("yes")}
                        disabled={disabled}
                        aria-label="Yes"
                    >
                        Yes
                    </button>
                    <button
                        type="button"
                        className="rich_input_button"
                        onClick={() => handleSubmit("yes_all")}
                        disabled={disabled}
                        aria-label="Yes to all - accept this and all future changes"
                        title="Accept this and all future changes without prompting"
                    >
                        Yes to All
                    </button>
                    <button
                        type="button"
                        className="rich_input_button"
                        onClick={() => handleSubmit("no")}
                        disabled={disabled}
                        aria-label="No"
                    >
                        No
                    </button>
                    <button
                        type="button"
                        className="rich_input_button"
                        onClick={() => {
                            setShowAlternative(true);
                        }}
                        disabled={disabled}
                        aria-label="Suggest alternative"
                        aria-expanded={false}
                        ref={alternativeButtonRef}
                    >
                        Alternative...
                    </button>
                </div>
            </div>
        );
    }

    // Yes/No input type
    if (metadata.input_type === "yes_no") {
        return (
            <div className="rich_input_prompt" role="form" aria-label="Respond with yes or no">
                <div className="sr-only" role="status" aria-live="polite">
                    Response required: choose yes or no
                </div>
                {promptStatus}
                <div className="rich_input_buttons">
                    <button
                        ref={primaryButtonRef}
                        type="button"
                        className="rich_input_button rich_input_button_primary"
                        onClick={() => handleSubmit("yes")}
                        disabled={disabled}
                        aria-label="Yes"
                    >
                        Yes
                    </button>
                    <button
                        type="button"
                        className="rich_input_button"
                        onClick={() => handleSubmit("no")}
                        disabled={disabled}
                        aria-label="No"
                    >
                        No
                    </button>
                </div>
            </div>
        );
    }

    // Choice input type
    if (metadata.input_type === "choice" && metadata.choices && metadata.choices.length > 0) {
        // If there are 4 or fewer choices, show as buttons
        if (metadata.choices.length <= 4) {
            return (
                <div className="rich_input_prompt" role="form" aria-label="Choose an option">
                    <div className="sr-only" role="status" aria-live="polite">
                        Response required: choose one of {metadata.choices.length} options
                    </div>
                    {promptStatus}
                    <div className="rich_input_buttons">
                        {metadata.choices.map((choice, index) => (
                            <button
                                key={`${choice}-${index}`}
                                ref={index === 0 ? primaryButtonRef : undefined}
                                type="button"
                                className={`rich_input_button ${index === 0 ? "rich_input_button_primary" : ""}`}
                                onClick={() => handleSubmit(choice)}
                                disabled={disabled}
                            >
                                {choice}
                            </button>
                        ))}
                    </div>
                </div>
            );
        }

        // More than 4 choices: use a dropdown
        return (
            <div className="rich_input_prompt">
                {renderPromptLabel(choiceSelectId)}
                <div className="rich_input_select_container">
                    <select
                        id={choiceSelectId}
                        className="rich_input_select"
                        value={value}
                        onChange={(e) => setValue(e.target.value)}
                        onKeyDown={handleKeyDown}
                        disabled={disabled}
                        aria-label={metadata.tts_prompt || metadata.prompt || "Choose an option"}
                    >
                        <option value="">-- Select --</option>
                        {metadata.choices.map((choice, index) => (
                            <option key={index} value={choice}>
                                {choice}
                            </option>
                        ))}
                    </select>
                    <button
                        type="button"
                        className="rich_input_button rich_input_button_primary"
                        onClick={submitCurrentValue}
                        disabled={disabled || !trimmedValue}
                    >
                        Submit
                    </button>
                </div>
            </div>
        );
    }

    // Number input type
    if (metadata.input_type === "number") {
        return (
            <div className="rich_input_prompt">
                {renderPromptLabel(numberInputId)}
                <div className="rich_input_number_container">
                    <input
                        id={numberInputId}
                        type="number"
                        className="rich_input_number"
                        value={value}
                        onChange={(e) => setValue(e.target.value)}
                        onKeyDown={handleKeyDown}
                        min={metadata.min}
                        max={metadata.max}
                        placeholder={metadata.placeholder}
                        disabled={disabled}
                        aria-label={metadata.tts_prompt || metadata.prompt || "Enter a number"}
                        autoFocus
                    />
                    <button
                        type="button"
                        className="rich_input_button rich_input_button_primary"
                        onClick={submitCurrentValue}
                        disabled={disabled || !trimmedValue}
                        aria-label="Submit number"
                    >
                        Submit
                    </button>
                </div>
            </div>
        );
    }

    // Text area input type
    if (metadata.input_type === "text_area") {
        return (
            <div className="rich_input_prompt">
                {renderPromptLabel(textAreaId)}
                <div className="rich_input_text_container">
                    <textarea
                        id={textAreaId}
                        className="rich_input_textarea"
                        value={value}
                        onChange={(e) => setValue(e.target.value)}
                        onKeyDown={(e) => {
                            if (e.key === "Enter" && e.ctrlKey) {
                                e.preventDefault();
                                submitCurrentValue();
                            }
                        }}
                        placeholder={metadata.placeholder}
                        disabled={disabled}
                        aria-label={metadata.tts_prompt || metadata.prompt || "Enter text"}
                        rows={metadata.rows || 4}
                        autoFocus
                    />
                    <div className="rich_input_buttons">
                        <button
                            type="button"
                            className="rich_input_button rich_input_button_primary"
                            onClick={submitCurrentValue}
                            disabled={disabled || !trimmedValue}
                            aria-label="Submit text"
                        >
                            Submit
                        </button>
                        <button
                            type="button"
                            className="rich_input_button"
                            onClick={() => handleSubmit("@abort")}
                            disabled={disabled}
                            aria-label="Cancel"
                        >
                            Cancel
                        </button>
                    </div>
                </div>
            </div>
        );
    }

    // Confirmation input type
    if (metadata.input_type === "confirmation") {
        return (
            <div className="rich_input_prompt" role="form" aria-label="Confirmation required">
                <div className="sr-only" role="status" aria-live="polite">
                    Confirmation required: press OK to continue
                </div>
                {promptStatus}
                <div className="rich_input_buttons">
                    <button
                        ref={primaryButtonRef}
                        type="button"
                        className="rich_input_button rich_input_button_primary"
                        onClick={() => handleSubmit("ok")}
                        disabled={disabled}
                        aria-label="OK"
                    >
                        OK
                    </button>
                </div>
            </div>
        );
    }

    // Image/file upload input type
    if (metadata.input_type === "image" || metadata.input_type === "file") {
        const acceptAttr = metadata.accept_content_types?.join(",") || (
            metadata.input_type === "image" ? "image/*" : "*/*"
        );
        const isImage = metadata.input_type === "image";

        return (
            <div className="rich_input_prompt" role="form" aria-label={isImage ? "Upload an image" : "Upload a file"}>
                <div className="sr-only" role="status" aria-live="polite">
                    {isImage ? "Image upload required" : "File upload required"}
                </div>
                {promptStatus}
                <div className="rich_input_file_container">
                    <input
                        ref={fileInputRef}
                        id={fileInputId}
                        type="file"
                        accept={acceptAttr}
                        onChange={handleFileSelect}
                        disabled={disabled}
                        className="rich_input_file"
                        aria-label={metadata.tts_prompt || metadata.prompt
                            || (isImage ? "Choose an image" : "Choose a file")}
                    />
                    <label htmlFor={fileInputId} className="rich_input_file_label">
                        {selectedFile ? selectedFile.name : (isImage ? "Choose image..." : "Choose file...")}
                    </label>
                    {previewUrl && (
                        <div className="rich_input_image_preview">
                            <img src={previewUrl} alt="Preview" />
                        </div>
                    )}
                    {selectedFile && !previewUrl && (
                        <div className="rich_input_file_info">
                            {selectedFile.name} ({(selectedFile.size / 1024).toFixed(1)} KB)
                        </div>
                    )}
                    {fileError && (
                        <div className="rich_input_error" role="alert">
                            {fileError}
                        </div>
                    )}
                    <div className="rich_input_buttons">
                        <button
                            ref={primaryButtonRef}
                            type="button"
                            className="rich_input_button rich_input_button_primary"
                            onClick={submitFile}
                            disabled={disabled || !selectedFile}
                            aria-label={isImage ? "Upload image" : "Upload file"}
                        >
                            Upload
                        </button>
                        <button
                            type="button"
                            className="rich_input_button"
                            onClick={() =>
                                handleSubmit("@abort")}
                            disabled={disabled}
                            aria-label="Cancel"
                        >
                            Cancel
                        </button>
                    </div>
                </div>
            </div>
        );
    }

    // Default text input type
    return (
        <div className="rich_input_prompt">
            {renderPromptLabel(textInputId)}
            <div className="rich_input_text_container">
                <input
                    id={textInputId}
                    type="text"
                    className="rich_input_text"
                    value={value}
                    onChange={(e) => setValue(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder={metadata.placeholder}
                    disabled={disabled}
                    aria-label={metadata.tts_prompt || metadata.prompt || "Enter text"}
                    autoFocus
                />
                <button
                    type="button"
                    className="rich_input_button rich_input_button_primary"
                    onClick={submitCurrentValue}
                    disabled={disabled || !trimmedValue}
                    aria-label="Submit text"
                >
                    Submit
                </button>
            </div>
        </div>
    );
};
