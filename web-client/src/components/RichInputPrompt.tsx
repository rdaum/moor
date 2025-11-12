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

import React, { useCallback, useId, useMemo, useState } from "react";
import { renderDjot, renderPlainText } from "../lib/djot-renderer";
import { InputMetadata } from "../types/input";

interface RichInputPromptProps {
    metadata: InputMetadata;
    onSubmit: (value: string) => void;
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
    const baseId = useId();
    const textInputId = `${baseId}-text`;
    const numberInputId = `${baseId}-number`;
    const choiceSelectId = `${baseId}-choice`;
    const alternativeInputId = `${baseId}-alternative`;
    const trimmedValue = value.trim();

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

    const promptStatus = useMemo(() => {
        if (!metadata.prompt) {
            return null;
        }

        return (
            <div className="rich_input_prompt_text" role="status">
                {renderPrompt(metadata.prompt)}
            </div>
        );
    }, [metadata.prompt, renderPrompt]);

    const renderPromptLabel = useCallback((targetId: string) => {
        if (!metadata.prompt) {
            return null;
        }

        return (
            <label htmlFor={targetId} className="rich_input_prompt_text">
                {renderPrompt(metadata.prompt)}
            </label>
        );
    }, [metadata.prompt, renderPrompt]);

    // Yes/No/Alternative input type (for coding agents, etc.)
    if (metadata.input_type === "yes_no_alternative") {
        if (showAlternative) {
            return (
                <div className="rich_input_prompt" role="group" aria-label={metadata.prompt || "Respond"}>
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
                    <div className="rich_input_alternative_container">
                        <label htmlFor={alternativeInputId} className="rich_input_prompt_text">
                            {renderPrompt(metadata.alternative_label || "Describe your alternative:")}
                        </label>
                        <div className="rich_input_text_container">
                            <textarea
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
            <div className="rich_input_prompt" role="group" aria-label={metadata.prompt || "Respond"}>
                {promptStatus}
                <div className="rich_input_buttons">
                    <button
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
                        onClick={() => setShowAlternative(true)}
                        disabled={disabled}
                        aria-label="Suggest alternative"
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
            <div className="rich_input_prompt" role="group" aria-label={metadata.prompt || "Respond"}>
                {promptStatus}
                <div className="rich_input_buttons">
                    <button
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
                <div className="rich_input_prompt" role="group" aria-label={metadata.prompt || "Choose an option"}>
                    {promptStatus}
                    <div className="rich_input_buttons">
                        {metadata.choices.map((choice, index) => (
                            <button
                                key={`${choice}-${index}`}
                                type="button"
                                className={`rich_input_button ${index === 0 ? "rich_input_button_primary" : ""}`}
                                onClick={() => handleSubmit(choice)}
                                disabled={disabled}
                                aria-label={choice}
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
                        aria-label={metadata.prompt || "Choose an option"}
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
                        aria-label="Submit choice"
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
                        aria-label={metadata.prompt || "Enter a number"}
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

    // Confirmation input type
    if (metadata.input_type === "confirmation") {
        return (
            <div className="rich_input_prompt" role="group" aria-label={metadata.prompt || "Confirm"}>
                {promptStatus}
                <div className="rich_input_buttons">
                    <button
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
                    aria-label={metadata.prompt || "Enter text"}
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
