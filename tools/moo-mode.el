;;; moo-mode.el --- Major mode for MOO language files -*- lexical-binding: t -*-

;; Copyright (C) 2025 Ryan Daum
;; Author: Ryan Daum <ryan.daum@gmail.com>
;; Keywords: languages, moo
;; Version: 1.0.0

;;; Commentary:
;; A major mode for editing MOO language files with syntax highlighting
;; and auto-indentation support.

;;; Code:

(defgroup moo nil
  "Major mode for editing MOO language files."
  :group 'languages)

(defcustom moo-indent-offset 2
  "Number of spaces for each indentation level."
  :type 'integer
  :group 'moo)

(defvar moo-mode-syntax-table
  (let ((st (make-syntax-table)))
    ;; Comments
    (modify-syntax-entry ?/ ". 124b" st)
    (modify-syntax-entry ?* ". 23" st)
    (modify-syntax-entry ?\n "> b" st)
    ;; String quotes
    (modify-syntax-entry ?\" "\"" st)
    ;; Single quotes for symbols
    (modify-syntax-entry ?' "'" st)
    ;; Parentheses
    (modify-syntax-entry ?\( "()" st)
    (modify-syntax-entry ?\) ")(" st)
    ;; Brackets
    (modify-syntax-entry ?\[ "(]" st)
    (modify-syntax-entry ?\] ")[" st)
    ;; Braces
    (modify-syntax-entry ?{ "(}" st)
    (modify-syntax-entry ?} "){" st)
    ;; Angle brackets for flyweight
    (modify-syntax-entry ?< "(>" st)
    (modify-syntax-entry ?> ")<" st)
    ;; Operators as punctuation
    (modify-syntax-entry ?+ "." st)
    (modify-syntax-entry ?- "." st)
    (modify-syntax-entry ?* "." st)
    (modify-syntax-entry ?/ "." st)
    (modify-syntax-entry ?% "." st)
    (modify-syntax-entry ?^ "." st)
    (modify-syntax-entry ?= "." st)
    (modify-syntax-entry ?! "." st)
    (modify-syntax-entry ?& "." st)
    (modify-syntax-entry ?| "." st)
    (modify-syntax-entry ?# "." st)
    (modify-syntax-entry ?$ "." st)
    (modify-syntax-entry ?@ "." st)
    (modify-syntax-entry ?: "." st)
    (modify-syntax-entry ?. "." st)
    (modify-syntax-entry ?, "." st)
    (modify-syntax-entry ?\; "." st)
    ;; Underscore is word constituent
    (modify-syntax-entry ?_ "w" st)
    st)
  "Syntax table for `moo-mode'.")

(defvar moo-font-lock-keywords
  `(
    ;; Keywords for control flow
    (,(regexp-opt '("if" "elseif" "else" "endif"
                    "for" "in" "endfor"
                    "while" "endwhile"
                    "fork" "endfork"
                    "try" "except" "finally" "endtry"
                    "begin" "end"
                    "fn" "endfn"
                    "return" "break" "continue"
                    "pass" "any"
                    "let" "const" "global"
                    "object" "endobject"
                    "verb" "endverb"
                    "property" "override"
                    "define"
                    "with" "captured" "self")
                  'words)
     . font-lock-keyword-face)
    
    ;; Type constants
    (,(regexp-opt '("int" "num" "float" "str" "err" "obj" 
                    "list" "map" "bool" "flyweight" "sym")
                  'words)
     . font-lock-type-face)
    
    ;; Boolean constants
    ("\\<\\(true\\|false\\)\\>" . font-lock-constant-face)
    
    ;; Error codes (e_xxx)
    ("\\<e_[a-zA-Z_][a-zA-Z0-9_]*\\>" . font-lock-constant-face)
    
    ;; Object references (#number)
    ("#-?[0-9]+" . font-lock-constant-face)
    
    ;; System properties ($xxx)
    ("\\$[a-zA-Z_][a-zA-Z0-9_]*" . font-lock-variable-name-face)
    
    ;; Symbols ('xxx)
    ("'[a-zA-Z_][a-zA-Z0-9_]*" . font-lock-constant-face)
    
    ;; Binary literals (b"...")
    ("b\"[a-zA-Z0-9+/=_-]*\"" . font-lock-string-face)
    
    ;; Numbers (integers and floats)
    ("\\<[+-]?[0-9]+\\(\\.[0-9]+\\)?\\([eE][+-]?[0-9]+\\)?\\>" . font-lock-constant-face)
    
    ;; Function definitions
    ("\\<fn\\s-+\\([a-zA-Z_][a-zA-Z0-9_]*\\)\\s-*("
     (1 font-lock-function-name-face))
    
    ;; Verb definitions
    ("\\<verb\\s-+\\(\"[^\"]*\"\\|[a-zA-Z_][a-zA-Z0-9_]*\\)\\s-*("
     (1 font-lock-function-name-face))
    
    ;; Property definitions
    ("\\<property\\s-+\\(\"[^\"]*\"\\|[a-zA-Z_][a-zA-Z0-9_]*\\)"
     (1 font-lock-variable-name-face))
    
    ;; Object definitions
    ("\\<object\\s-+\\(#-?[0-9]+\\|[a-zA-Z_][a-zA-Z0-9_]*\\)"
     (1 font-lock-type-face))
    
    ;; Variable declarations (let, const, global)
    ("\\<\\(let\\|const\\|global\\)\\s-+\\([a-zA-Z_][a-zA-Z0-9_]*\\)"
     (2 font-lock-variable-name-face))
    
    ;; Method calls (:method)
    (":\\([a-zA-Z_][a-zA-Z0-9_]*\\)\\s-*(" 
     (1 font-lock-function-name-face))
    
    ;; Property access (.property)
    ("\\.\\([a-zA-Z_][a-zA-Z0-9_]*\\)"
     (1 font-lock-variable-name-face))
    
    ;; Built-in function calls (simple identifier followed by parenthesis)
    ("\\<\\([a-zA-Z_][a-zA-Z0-9_]*\\)\\s-*("
     (1 font-lock-function-name-face))
    
    ;; Assignment operators
    ("\\(=\\|+=\\|-=\\|\\*=\\|/=\\|%=\\)" . font-lock-keyword-face)
    
    ;; Logical operators
    ("\\(&&\\|||\\|!\\)" . font-lock-keyword-face)
    
    ;; Comparison operators
    ("\\(==\\|!=\\|<=\\|>=\\|<\\|>\\)" . font-lock-keyword-face)
    
    ;; Range operator
    ("\\.\\." . font-lock-keyword-face)
    
    ;; Lambda arrow
    ("=>" . font-lock-keyword-face)
    
    ;; Map/property arrow
    ("->" . font-lock-keyword-face)
    )
  "Font lock keywords for `moo-mode'.")

(defvar moo-indent-keywords
  '("if" "elseif" "else" "for" "while" "fork" "try" "except" "finally" 
    "begin" "fn" "object" "verb")
  "Keywords that increase indentation.")

(defvar moo-dedent-keywords
  '("elseif" "else" "except" "finally")
  "Keywords that dedent before themselves.")

(defvar moo-end-keywords
  '("endif" "endfor" "endwhile" "endfork" "endtry" "end" "endfn" 
    "endobject" "endverb")
  "Keywords that end blocks.")

(defun moo-indent-line ()
  "Indent current line as MOO code."
  (interactive)
  (let ((indent 0)
        (pos (- (point-max) (point)))
        (at-end-keyword nil)
        (at-dedent-keyword nil))
    (beginning-of-line)
    (save-excursion
      ;; Check if current line has an end keyword
      (when (looking-at "\\s-*\\(endif\\|endfor\\|endwhile\\|endfork\\|endtry\\|end\\|endfn\\|endobject\\|endverb\\)\\>")
        (setq at-end-keyword t))
      ;; Check if current line has a dedent keyword
      (when (looking-at "\\s-*\\(elseif\\|else\\|except\\|finally\\)\\>")
        (setq at-dedent-keyword t))
      ;; Find previous non-empty line
      (forward-line -1)
      (while (and (not (bobp))
                  (looking-at "^\\s-*$"))
        (forward-line -1))
      ;; Get indentation of previous line
      (setq indent (current-indentation))
      ;; Check if previous line should increase indentation
      (when (looking-at ".*\\<\\(if\\|elseif\\|else\\|for\\|while\\|fork\\|try\\|except\\|finally\\|begin\\|fn\\|object\\|verb\\)\\>")
        (unless (looking-at ".*\\<\\(endif\\|endfor\\|endwhile\\|endfork\\|endtry\\|end\\|endfn\\|endobject\\|endverb\\)\\>")
          (setq indent (+ indent moo-indent-offset))))
      ;; Check for lambda or statement continuation
      (when (looking-at ".*=>\\s-*$")
        (setq indent (+ indent moo-indent-offset)))
      ;; Check for unclosed parentheses, brackets, or braces
      (when (looking-at ".*[({[]\\s-*$")
        (unless (looking-at ".*[])}]\\s-*$")
          (setq indent (+ indent moo-indent-offset)))))
    ;; Adjust for end keywords
    (when at-end-keyword
      (setq indent (max 0 (- indent moo-indent-offset))))
    ;; Adjust for dedent keywords
    (when at-dedent-keyword
      (setq indent (max 0 (- indent moo-indent-offset))))
    ;; Apply indentation
    (indent-line-to indent)
    ;; Move point appropriately
    (when (> (- (point-max) pos) (point))
      (goto-char (- (point-max) pos)))))

(defun moo-mode-electric-brace (arg)
  "Insert a brace and possibly reindent."
  (interactive "P")
  (self-insert-command (prefix-numeric-value arg))
  (when (and (not arg)
             (eolp)
             (save-excursion
               (beginning-of-line)
               (looking-at "\\s-*[})]")))
    (moo-indent-line)))

(defvar moo-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map "}" 'moo-mode-electric-brace)
    (define-key map ")" 'moo-mode-electric-brace)
    (define-key map "]" 'moo-mode-electric-brace)
    (define-key map (kbd "C-c C-c") 'comment-region)
    (define-key map (kbd "C-c C-u") 'uncomment-region)
    map)
  "Keymap for `moo-mode'.")

(defun moo-mode-setup-imenu ()
  "Setup imenu for MOO mode."
  (setq imenu-generic-expression
        '(("Functions" "^\\s-*fn\\s-+\\([a-zA-Z_][a-zA-Z0-9_]*\\)" 1)
          ("Verbs" "^\\s-*verb\\s-+\\(\"[^\"]*\"\\|[a-zA-Z_][a-zA-Z0-9_]*\\)" 1)
          ("Objects" "^\\s-*object\\s-+\\(#-?[0-9]+\\|[a-zA-Z_][a-zA-Z0-9_]*\\)" 1)
          ("Properties" "^\\s-*property\\s-+\\(\"[^\"]*\"\\|[a-zA-Z_][a-zA-Z0-9_]*\\)" 1)
          ("Constants" "^\\s-*define\\s-+\\([a-zA-Z_][a-zA-Z0-9_]*\\)" 1))))

;;;###autoload
(define-derived-mode moo-mode prog-mode "MOO"
  "Major mode for editing MOO language files.
\\{moo-mode-map}"
  :syntax-table moo-mode-syntax-table
  (setq-local font-lock-defaults '(moo-font-lock-keywords))
  (setq-local indent-line-function 'moo-indent-line)
  (setq-local comment-start "// ")
  (setq-local comment-start-skip "\\(//+\\|/\\*+\\)\\s-*")
  (setq-local comment-end "")
  (setq-local comment-multi-line t)
  (setq-local electric-indent-chars (append electric-indent-chars '(?} ?) ?])))
  (moo-mode-setup-imenu))

;;;###autoload
(add-to-list 'auto-mode-alist '("\\.moo\\'" . moo-mode))
;;;###autoload
(add-to-list 'auto-mode-alist '("\\.moor\\'" . moo-mode))

(provide 'moo-mode)

;;; moo-mode.el ends here