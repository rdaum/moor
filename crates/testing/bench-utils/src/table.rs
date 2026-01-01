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

/// Generic table formatter with proper Unicode-aware column alignment
pub struct TableFormatter {
    headers: Vec<String>,
    column_widths: Vec<usize>,
    rows: Vec<Vec<String>>,
}

impl TableFormatter {
    pub fn new(headers: Vec<&str>, column_widths: Vec<usize>) -> Self {
        Self {
            headers: headers.iter().map(|s| s.to_string()).collect(),
            column_widths,
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, row: Vec<&str>) {
        self.rows.push(row.iter().map(|s| s.to_string()).collect());
    }

    /// Calculate the display width of a string, accounting for wide characters like emojis
    fn display_width(&self, text: &str) -> usize {
        text.chars()
            .map(|c| {
                match c {
                    // Most emojis and symbols have width 2
                    '\u{1F300}'..='\u{1F9FF}' => 2, // Miscellaneous Symbols and Pictographs, Emoticons, etc.
                    '\u{2600}'..='\u{26FF}' => 2,   // Miscellaneous Symbols
                    '\u{2700}'..='\u{27BF}' => 2,   // Dingbats
                    _ => 1,                         // Regular characters
                }
            })
            .sum()
    }

    fn format_cell(&self, text: &str, width: usize, align: &str) -> String {
        let display_width = self.display_width(text);
        if display_width > width {
            // Truncate Unicode-aware - this is complex with variable-width chars, so just truncate simply
            let truncated: String = text.chars().take(width.saturating_sub(3)).collect();
            format!("{truncated}...")
        } else {
            let padding = width.saturating_sub(display_width);
            match align {
                "left" => format!("{}{}", text, " ".repeat(padding)),
                "right" => format!("{}{}", " ".repeat(padding), text),
                "center" => {
                    let left_pad = padding / 2;
                    let right_pad = padding - left_pad;
                    format!("{}{}{}", " ".repeat(left_pad), text, " ".repeat(right_pad))
                }
                _ => format!("{}{}", text, " ".repeat(padding)),
            }
        }
    }

    pub fn print(&self) {
        let has_headers = !self.headers.is_empty();

        // Top border
        print!("┌");
        for (i, &width) in self.column_widths.iter().enumerate() {
            print!("{}", "─".repeat(width));
            if i < self.column_widths.len() - 1 {
                print!("┬");
            }
        }
        println!("┐");

        // Header row (only if headers exist)
        if has_headers {
            print!("│");
            for (header, &width) in self.headers.iter().zip(self.column_widths.iter()) {
                let formatted = self.format_cell(header, width, "center");
                print!("{formatted}");
                print!("│");
            }
            println!();

            // Header separator
            print!("├");
            for (i, &width) in self.column_widths.iter().enumerate() {
                print!("{}", "─".repeat(width));
                if i < self.column_widths.len() - 1 {
                    print!("┼");
                }
            }
            println!("┤");
        }

        // Data rows with separators between them
        for (row_idx, row) in self.rows.iter().enumerate() {
            print!("│");
            for (i, (cell, &width)) in row.iter().zip(self.column_widths.iter()).enumerate() {
                let align = if i == 0 { "left" } else { "center" };
                let formatted = self.format_cell(cell, width, align);
                print!("{formatted}");
                print!("│");
            }
            println!();

            // Add row separator (except after the last row)
            if row_idx < self.rows.len() - 1 {
                print!("├");
                for (i, &width) in self.column_widths.iter().enumerate() {
                    print!("{}", "─".repeat(width));
                    if i < self.column_widths.len() - 1 {
                        print!("┼");
                    }
                }
                println!("┤");
            }
        }

        // Bottom border
        print!("└");
        for (i, &width) in self.column_widths.iter().enumerate() {
            print!("{}", "─".repeat(width));
            if i < self.column_widths.len() - 1 {
                print!("┴");
            }
        }
        println!("┘");
    }
}
