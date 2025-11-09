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

import { parse, renderHTML } from "@djot/djot";
import { AnsiUp } from "ansi_up";
import DOMPurify from "dompurify";
import Prism from "prismjs";
import React, { useCallback, useState } from "react";
import { InputMetadata } from "../types/input";
import "../lib/prism-moo";

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

    // Render prompt text as djot with ANSI escape codes
    const renderPrompt = useCallback((text: string) => {
        try {
            // Parse djot markdown and render to HTML
            const djotAst = parse(text);
            const djotHtml = renderHTML(djotAst, {
                overrides: {
                    link: (node: any, _context: any) => {
                        const href = node.destination || "";

                        // Extract link text from djot AST
                        let linkText = "";
                        if (node.children && node.children.length > 0) {
                            linkText = node.children.map((child: any) => {
                                if (child.tag === "str") {
                                    return child.text || "";
                                }
                                return "";
                            }).join("");
                        }

                        if (!linkText.trim()) {
                            linkText = href;
                        }

                        // For prompts, just render links as plain text with basic styling
                        return `<span class="prompt-link" style="text-decoration: underline;">${linkText}</span>`;
                    },
                } as any,
            });

            // Sanitize the rendered HTML
            const sanitizedDjotHtml = DOMPurify.sanitize(djotHtml, {
                ALLOWED_TAGS: [
                    "p",
                    "br",
                    "div",
                    "span",
                    "strong",
                    "b",
                    "em",
                    "i",
                    "u",
                    "s",
                    "ul",
                    "ol",
                    "li",
                    "pre",
                    "code",
                    "small",
                    "sup",
                    "sub",
                ],
                ALLOWED_ATTR: [
                    "style",
                    "class",
                ],
            });

            // Process ANSI codes
            const tempDiv = document.createElement("div");
            tempDiv.innerHTML = sanitizedDjotHtml;
            const ansi_up = new AnsiUp();

            // Process <pre> blocks for syntax highlighting
            const preElements = tempDiv.querySelectorAll("pre");
            preElements.forEach((pre) => {
                const codeElement = pre.querySelector("code");

                if (codeElement) {
                    const classes = codeElement.className.split(/\s+/);
                    const languageClass = classes.find(cls => cls.startsWith("language-"));

                    if (languageClass) {
                        const language = languageClass.replace("language-", "");

                        if (Prism.languages[language]) {
                            const code = codeElement.textContent || "";
                            const highlightedHtml = Prism.highlight(
                                code,
                                Prism.languages[language],
                                language,
                            );
                            codeElement.innerHTML = highlightedHtml;
                            codeElement.classList.add(`language-${language}`);
                        }
                        return;
                    }
                }

                // ANSI code processing for non-syntax-highlighted pre blocks
                const preText = pre.textContent || "";
                if (preText.includes("\x1b[")) {
                    const ansiHtml = ansi_up.ansi_to_html(preText).replace(/\n/g, "<br>");
                    pre.innerHTML = ansiHtml;
                }
            });

            // Process ANSI codes in all other text nodes
            function processTextNodesForAnsi(node: Node): void {
                if (node.nodeType === Node.TEXT_NODE) {
                    const nodeText = node.textContent || "";
                    if (nodeText.includes("\x1b[")) {
                        const ansiHtml = ansi_up.ansi_to_html(nodeText);
                        const span = document.createElement("span");
                        span.innerHTML = ansiHtml;
                        node.parentNode?.replaceChild(span, node);
                    }
                } else if (node.nodeType === Node.ELEMENT_NODE) {
                    const element = node as Element;
                    if (element.tagName !== "PRE" && element.tagName !== "CODE") {
                        Array.from(node.childNodes).forEach(processTextNodesForAnsi);
                    }
                }
            }

            processTextNodesForAnsi(tempDiv);

            const processedHtml = tempDiv.innerHTML;

            return <div dangerouslySetInnerHTML={{ __html: processedHtml }} />;
        } catch (error) {
            // Fallback to plain text with ANSI if djot parsing fails
            console.warn("Failed to parse djot content in prompt:", error);
            const ansi_up = new AnsiUp();
            const htmlFromAnsi = ansi_up.ansi_to_html(text);
            const sanitizedHtml = DOMPurify.sanitize(htmlFromAnsi, {
                ALLOWED_TAGS: ["span", "div", "br"],
                ALLOWED_ATTR: ["style", "class"],
            });
            return <div dangerouslySetInnerHTML={{ __html: sanitizedHtml }} />;
        }
    }, []);

    const handleSubmit = useCallback((submitValue: string) => {
        onSubmit(submitValue);
        setValue("");
    }, [onSubmit]);

    const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
        if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            if (value.trim()) {
                handleSubmit(value.trim());
            }
        }
    }, [value, handleSubmit]);

    // Yes/No/Alternative input type (for coding agents, etc.)
    if (metadata.input_type === "yes_no_alternative") {
        if (showAlternative) {
            return (
                <div className="rich_input_prompt" role="group" aria-label={metadata.prompt || "Respond"}>
                    {metadata.prompt && (
                        <div className="rich_input_prompt_text" role="status">
                            {renderPrompt(metadata.prompt)}
                        </div>
                    )}
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
                        <label htmlFor="alternative-input" className="rich_input_prompt_text">
                            {renderPrompt(metadata.alternative_label || "Describe your alternative:")}
                        </label>
                        <div className="rich_input_text_container">
                            <textarea
                                id="alternative-input"
                                className="rich_input_textarea"
                                value={value}
                                onChange={(e) => setValue(e.target.value)}
                                onKeyDown={(e) => {
                                    if (e.key === "Enter" && e.ctrlKey) {
                                        e.preventDefault();
                                        if (value.trim()) {
                                            handleSubmit(`alternative: ${value.trim()}`);
                                        }
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
                            onClick={() => value.trim() && handleSubmit(`alternative: ${value.trim()}`)}
                            disabled={disabled || !value.trim()}
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
                {metadata.prompt && (
                    <div className="rich_input_prompt_text" role="status">
                        {renderPrompt(metadata.prompt)}
                    </div>
                )}
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
                {metadata.prompt && (
                    <div className="rich_input_prompt_text" role="status">
                        {renderPrompt(metadata.prompt)}
                    </div>
                )}
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
                    {metadata.prompt && (
                        <div className="rich_input_prompt_text" role="status">
                            {renderPrompt(metadata.prompt)}
                        </div>
                    )}
                    <div className="rich_input_buttons">
                        {metadata.choices.map((choice, index) => (
                            <button
                                key={index}
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
                {metadata.prompt && (
                    <label htmlFor="choice-select" className="rich_input_prompt_text">
                        {renderPrompt(metadata.prompt)}
                    </label>
                )}
                <div className="rich_input_select_container">
                    <select
                        id="choice-select"
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
                        onClick={() => value.trim() && handleSubmit(value.trim())}
                        disabled={disabled || !value.trim()}
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
                {metadata.prompt && (
                    <label htmlFor="number-input" className="rich_input_prompt_text">
                        {renderPrompt(metadata.prompt)}
                    </label>
                )}
                <div className="rich_input_number_container">
                    <input
                        id="number-input"
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
                        onClick={() => value.trim() && handleSubmit(value.trim())}
                        disabled={disabled || !value.trim()}
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
                {metadata.prompt && (
                    <div className="rich_input_prompt_text" role="status">
                        {renderPrompt(metadata.prompt)}
                    </div>
                )}
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
            {metadata.prompt && (
                <label htmlFor="text-input" className="rich_input_prompt_text">
                    {renderPrompt(metadata.prompt)}
                </label>
            )}
            <div className="rich_input_text_container">
                <input
                    id="text-input"
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
                    onClick={() => value.trim() && handleSubmit(value.trim())}
                    disabled={disabled || !value.trim()}
                    aria-label="Submit text"
                >
                    Submit
                </button>
            </div>
        </div>
    );
};
