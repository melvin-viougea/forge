use gpui::*;
use std::ops::Range;
use std::path::PathBuf;

use crate::theme;

// ── Markdown AST ──────────────────────────────────────────

#[derive(Clone, Debug)]
enum MdBlock {
    Heading(u8, Vec<MdInline>),
    Paragraph(Vec<MdInline>),
    CodeBlock(String, String),           // language, code
    UnorderedList(Vec<ListItem>),
    OrderedList(Vec<ListItem>),
    HorizontalRule,
    Blockquote(Vec<MdBlock>),            // multi-line: nested blocks
    Table(Vec<Vec<Vec<MdInline>>>),      // rows of cells with inline formatting
}

#[derive(Clone, Debug)]
struct ListItem {
    checkbox: Option<bool>,              // None = normal, Some(false) = [ ], Some(true) = [x]
    content: Vec<MdInline>,
    children: Vec<MdBlock>,              // nested sub-lists
}

#[derive(Clone, Debug)]
enum MdInline {
    Text(String),
    Bold(Vec<MdInline>),
    Italic(Vec<MdInline>),
    BoldItalic(Vec<MdInline>),
    Strikethrough(Vec<MdInline>),
    Code(String),
    Link(String, String), // text, url
}

// ── Parser ────────────────────────────────────────────────

fn parse_markdown(input: &str) -> Vec<MdBlock> {
    let lines: Vec<&str> = input.lines().collect();
    parse_blocks(&lines, 0, lines.len()).0
}

fn parse_blocks(lines: &[&str], start: usize, end: usize) -> (Vec<MdBlock>, usize) {
    let mut blocks = Vec::new();
    let mut i = start;

    while i < end {
        let line = lines[i];

        // Empty line
        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        // Code block (fenced)
        if line.trim_start().starts_with("```") {
            let lang = line.trim_start().trim_start_matches('`').trim().to_string();
            let mut code = String::new();
            i += 1;
            while i < end && !lines[i].trim_start().starts_with("```") {
                if !code.is_empty() { code.push('\n'); }
                code.push_str(lines[i]);
                i += 1;
            }
            if i < end { i += 1; }
            blocks.push(MdBlock::CodeBlock(lang, code));
            continue;
        }

        // Heading (ATX)
        if line.starts_with('#') {
            let level = line.chars().take_while(|c| *c == '#').count().min(6) as u8;
            let text = line[(level as usize)..].trim_start();
            blocks.push(MdBlock::Heading(level, parse_inline(text)));
            i += 1;
            continue;
        }

        // Setext heading: text followed by === (h1) or --- (h2)
        if i + 1 < end && !line.trim().is_empty() {
            let next = lines[i + 1].trim();
            if !next.is_empty() && next.chars().all(|c| c == '=') && next.len() >= 2 {
                blocks.push(MdBlock::Heading(1, parse_inline(line.trim())));
                i += 2;
                continue;
            }
            if !next.is_empty() && next.chars().all(|c| c == '-') && next.len() >= 2 {
                // Avoid conflict with HR or list — only treat as setext if line looks like text
                if !line.trim_start().starts_with("- ") && !line.trim_start().starts_with("* ") {
                    blocks.push(MdBlock::Heading(2, parse_inline(line.trim())));
                    i += 2;
                    continue;
                }
            }
        }

        // Table
        if line.contains('|') && i + 1 < end && is_table_separator(lines[i + 1]) {
            let mut rows: Vec<Vec<Vec<MdInline>>> = Vec::new();
            rows.push(parse_table_row(line).into_iter().map(|c| parse_inline(&c)).collect());
            i += 2; // skip header + separator
            while i < end && lines[i].contains('|') && !lines[i].trim().is_empty() {
                rows.push(parse_table_row(lines[i]).into_iter().map(|c| parse_inline(&c)).collect());
                i += 1;
            }
            blocks.push(MdBlock::Table(rows));
            continue;
        }

        // Horizontal rule
        if line.trim().chars().all(|c| c == '-' || c == ' ') && line.trim().len() >= 3
            && line.trim().chars().filter(|c| *c == '-').count() >= 3
        {
            blocks.push(MdBlock::HorizontalRule);
            i += 1;
            continue;
        }

        // Blockquote (multi-line)
        if line.trim_start().starts_with("> ") || line.trim_start() == ">" {
            let mut quote_lines: Vec<String> = Vec::new();
            while i < end {
                let l = lines[i].trim_start();
                if l.starts_with("> ") {
                    quote_lines.push(l[2..].to_string());
                    i += 1;
                } else if l == ">" {
                    quote_lines.push(String::new());
                    i += 1;
                } else {
                    break;
                }
            }
            let joined: Vec<&str> = quote_lines.iter().map(|s| s.as_str()).collect();
            let (inner_blocks, _) = parse_blocks(&joined, 0, joined.len());
            blocks.push(MdBlock::Blockquote(inner_blocks));
            continue;
        }

        // Unordered list (with checkbox support + nested lists)
        if line.trim_start().starts_with("- ") || line.trim_start().starts_with("* ") {
            let base_indent = line.len() - line.trim_start().len();
            let (items, new_i) = parse_unordered_list(lines, i, end, base_indent);
            i = new_i;
            blocks.push(MdBlock::UnorderedList(items));
            continue;
        }

        // Ordered list (with nested lists)
        if let Some(dot_pos) = line.trim_start().find(". ") {
            if dot_pos > 0 && line.trim_start()[..dot_pos].chars().all(|c| c.is_ascii_digit()) {
                let base_indent = line.len() - line.trim_start().len();
                let (items, new_i) = parse_ordered_list(lines, i, end, base_indent);
                if new_i > i {
                    i = new_i;
                    blocks.push(MdBlock::OrderedList(items));
                    continue;
                }
            }
        }

        // Paragraph (fallback — always advances i to prevent infinite loop)
        let mut para = String::new();
        let mut prev_hard_break = false;
        while i < end && !lines[i].trim().is_empty()
            && !lines[i].starts_with('#')
            && !lines[i].trim_start().starts_with("```")
            && !lines[i].trim_start().starts_with("- ")
            && !lines[i].trim_start().starts_with("* ")
            && !lines[i].trim_start().starts_with("> ")
            // Stop when a table starts (line with | followed by separator)
            && !(lines[i].contains('|') && i + 1 < end && is_table_separator(lines[i + 1]))
        {
            if !para.is_empty() {
                if prev_hard_break {
                    para.push('\n');
                } else {
                    para.push(' ');
                }
            }
            // Markdown hard break: two trailing spaces → line break
            prev_hard_break = lines[i].ends_with("  ");
            para.push_str(lines[i].trim());
            i += 1;
        }
        if !para.is_empty() {
            blocks.push(MdBlock::Paragraph(parse_inline(&para)));
        } else {
            // Safety: skip any line that no parser matched to avoid infinite loop
            i += 1;
        }
    }

    (blocks, i)
}

fn parse_checkbox(text: &str) -> (Option<bool>, &str) {
    if text.starts_with("[x] ") || text.starts_with("[X] ") {
        (Some(true), &text[4..])
    } else if text.starts_with("[ ] ") {
        (Some(false), &text[4..])
    } else {
        (None, text)
    }
}

fn parse_unordered_list(lines: &[&str], start: usize, end: usize, base_indent: usize) -> (Vec<ListItem>, usize) {
    let mut items = Vec::new();
    let mut i = start;

    while i < end {
        let line = lines[i];
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();

        // If less indented or empty, stop this list level
        if trimmed.is_empty() || indent < base_indent {
            break;
        }

        // If at same indent and is a list marker, parse item
        if indent == base_indent && (trimmed.starts_with("- ") || trimmed.starts_with("* ")) {
            let content_str = &trimmed[2..];
            let (checkbox, text) = parse_checkbox(content_str);
            let mut item = ListItem { checkbox, content: parse_inline(text), children: Vec::new() };
            i += 1;

            // Check for nested content (more indented)
            if i < end {
                let next_indent = lines[i].len() - lines[i].trim_start().len();
                let next_trimmed = lines[i].trim_start();
                if next_indent > base_indent && !next_trimmed.is_empty() {
                    // Nested unordered list
                    if next_trimmed.starts_with("- ") || next_trimmed.starts_with("* ") {
                        let (children, new_i) = parse_unordered_list(lines, i, end, next_indent);
                        item.children.push(MdBlock::UnorderedList(children));
                        i = new_i;
                    }
                    // Nested ordered list
                    else if next_trimmed.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && next_trimmed.contains(". ") {
                        let (children, new_i) = parse_ordered_list(lines, i, end, next_indent);
                        item.children.push(MdBlock::OrderedList(children));
                        i = new_i;
                    }
                }
            }
            items.push(item);
        } else if indent > base_indent {
            // Sub-items at greater indent — belongs to last item as nested list
            if !items.is_empty() {
                if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                    let (children, new_i) = parse_unordered_list(lines, i, end, indent);
                    items.last_mut().unwrap().children.push(MdBlock::UnorderedList(children));
                    i = new_i;
                } else if trimmed.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && trimmed.contains(". ") {
                    let (children, new_i) = parse_ordered_list(lines, i, end, indent);
                    items.last_mut().unwrap().children.push(MdBlock::OrderedList(children));
                    i = new_i;
                } else {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }

    (items, i)
}

fn parse_ordered_list(lines: &[&str], start: usize, end: usize, base_indent: usize) -> (Vec<ListItem>, usize) {
    let mut items = Vec::new();
    let mut i = start;

    while i < end {
        let line = lines[i];
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();

        if trimmed.is_empty() || indent < base_indent {
            break;
        }

        if indent == base_indent {
            if let Some(dot_pos) = trimmed.find(". ") {
                if trimmed[..dot_pos].chars().all(|c| c.is_ascii_digit()) {
                    let content_str = &trimmed[dot_pos + 2..];
                    let (checkbox, text) = parse_checkbox(content_str);
                    let mut item = ListItem { checkbox, content: parse_inline(text), children: Vec::new() };
                    i += 1;

                    // Check for nested content
                    if i < end {
                        let next_indent = lines[i].len() - lines[i].trim_start().len();
                        let next_trimmed = lines[i].trim_start();
                        if next_indent > base_indent && !next_trimmed.is_empty() {
                            if next_trimmed.starts_with("- ") || next_trimmed.starts_with("* ") {
                                let (children, new_i) = parse_unordered_list(lines, i, end, next_indent);
                                item.children.push(MdBlock::UnorderedList(children));
                                i = new_i;
                            } else if next_trimmed.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && next_trimmed.contains(". ") {
                                let (children, new_i) = parse_ordered_list(lines, i, end, next_indent);
                                item.children.push(MdBlock::OrderedList(children));
                                i = new_i;
                            }
                        }
                    }
                    items.push(item);
                    continue;
                }
            }
            break;
        } else if indent > base_indent {
            if !items.is_empty() {
                if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                    let (children, new_i) = parse_unordered_list(lines, i, end, indent);
                    items.last_mut().unwrap().children.push(MdBlock::UnorderedList(children));
                    i = new_i;
                } else if trimmed.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && trimmed.contains(". ") {
                    let (children, new_i) = parse_ordered_list(lines, i, end, indent);
                    items.last_mut().unwrap().children.push(MdBlock::OrderedList(children));
                    i = new_i;
                } else {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }

    (items, i)
}

fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|')
        && trimmed.replace('|', "").replace('-', "").replace(':', "").replace(' ', "").is_empty()
}

fn parse_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim().trim_matches('|');
    trimmed.split('|').map(|cell| cell.trim().to_string()).collect()
}

fn parse_inline(input: &str) -> Vec<MdInline> {
    let mut result = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut current = String::new();

    while i < chars.len() {
        // Backslash escaping: \* \_ \~ \` \[ \] \( \) \# \> \- \. \! \\
        if chars[i] == '\\' && i + 1 < chars.len() {
            let next = chars[i + 1];
            if matches!(next, '*' | '_' | '~' | '`' | '[' | ']' | '(' | ')' | '#' | '>' | '-' | '.' | '!' | '\\' | '|') {
                current.push(next);
                i += 2;
                continue;
            }
        }

        // Inline code
        if chars[i] == '`' {
            if !current.is_empty() {
                result.push(MdInline::Text(std::mem::take(&mut current)));
            }
            i += 1;
            let mut code = String::new();
            while i < chars.len() && chars[i] != '`' {
                code.push(chars[i]);
                i += 1;
            }
            if i < chars.len() { i += 1; }
            result.push(MdInline::Code(code));
            continue;
        }

        // Link: [text](url)
        if chars[i] == '[' {
            i += 1;
            let mut link_text = String::new();
            while i < chars.len() && chars[i] != ']' {
                link_text.push(chars[i]);
                i += 1;
            }
            if i + 1 < chars.len() && chars[i] == ']' && chars[i + 1] == '(' {
                i += 2;
                let mut url = String::new();
                while i < chars.len() && chars[i] != ')' {
                    url.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() { i += 1; }
                if !current.is_empty() {
                    result.push(MdInline::Text(std::mem::take(&mut current)));
                }
                result.push(MdInline::Link(link_text, url));
                continue;
            } else {
                current.push('[');
                current.push_str(&link_text);
                if i < chars.len() && chars[i] == ']' {
                    current.push(']');
                    i += 1;
                }
                continue;
            }
        }

        // Strikethrough: ~~text~~
        if chars[i] == '~' && i + 1 < chars.len() && chars[i + 1] == '~' {
            let after = i + 2;
            if let Some(end) = find_closing(&chars, after, "~~") {
                if !current.is_empty() {
                    result.push(MdInline::Text(std::mem::take(&mut current)));
                }
                let inner: String = chars[after..end].iter().collect();
                result.push(MdInline::Strikethrough(parse_inline(&inner)));
                i = end + 2;
                continue;
            }
        }

        // Bold+Italic / Bold / Italic
        if chars[i] == '*' {
            let star_count = chars[i..].iter().take_while(|c| **c == '*').count();

            if star_count >= 3 {
                let after = i + 3;
                if let Some(end) = find_closing(&chars, after, "***") {
                    if !current.is_empty() {
                        result.push(MdInline::Text(std::mem::take(&mut current)));
                    }
                    let inner: String = chars[after..end].iter().collect();
                    result.push(MdInline::BoldItalic(parse_inline(&inner)));
                    i = end + 3;
                    continue;
                }
            }

            if star_count >= 2 {
                let after = i + 2;
                if let Some(end) = find_closing(&chars, after, "**") {
                    if !current.is_empty() {
                        result.push(MdInline::Text(std::mem::take(&mut current)));
                    }
                    let inner: String = chars[after..end].iter().collect();
                    result.push(MdInline::Bold(parse_inline(&inner)));
                    i = end + 2;
                    continue;
                }
            }

            if star_count >= 1 {
                let after = i + 1;
                if let Some(end) = find_closing(&chars, after, "*") {
                    if !current.is_empty() {
                        result.push(MdInline::Text(std::mem::take(&mut current)));
                    }
                    let inner: String = chars[after..end].iter().collect();
                    result.push(MdInline::Italic(parse_inline(&inner)));
                    i = end + 1;
                    continue;
                }
            }

            current.push(chars[i]);
            i += 1;
            continue;
        }

        current.push(chars[i]);
        i += 1;
    }

    if !current.is_empty() {
        result.push(MdInline::Text(current));
    }

    result
}

fn find_closing(chars: &[char], from: usize, pattern: &str) -> Option<usize> {
    let pat: Vec<char> = pattern.chars().collect();
    let pat_len = pat.len();
    if from + pat_len > chars.len() { return None; }

    for i in from..(chars.len() - pat_len + 1) {
        if chars[i..i + pat_len].iter().zip(pat.iter()).all(|(a, b)| a == b) {
            if i > from {
                return Some(i);
            }
        }
    }
    None
}

// ── Inline flattening ────────────────────────────────────

#[derive(Clone)]
struct InlineSpan {
    text: String,
    color: Option<Rgba>,
    weight: Option<FontWeight>,
    monospace: bool,
    strikethrough: bool,
    link_url: Option<String>,
}

fn flatten_inlines(inlines: &[MdInline], bold: bool, italic: bool, strike: bool) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    for inline in inlines {
        match inline {
            MdInline::Text(text) => {
                let color = if italic { Some(theme::subtext()) } else { None };
                let weight = if bold { Some(FontWeight::BOLD) } else { None };
                spans.push(InlineSpan { text: text.clone(), color, weight, monospace: false, strikethrough: strike, link_url: None });
            }
            MdInline::Bold(children) => {
                spans.extend(flatten_inlines(children, true, italic, strike));
            }
            MdInline::Italic(children) => {
                spans.extend(flatten_inlines(children, bold, true, strike));
            }
            MdInline::BoldItalic(children) => {
                spans.extend(flatten_inlines(children, true, true, strike));
            }
            MdInline::Strikethrough(children) => {
                spans.extend(flatten_inlines(children, bold, italic, true));
            }
            MdInline::Code(text) => {
                spans.push(InlineSpan {
                    text: text.clone(),
                    color: Some(theme::peach()),
                    weight: None,
                    monospace: true,
                    strikethrough: strike,
                    link_url: None,
                });
            }
            MdInline::Link(text, url) => {
                let weight = if bold { Some(FontWeight::BOLD) } else { None };
                spans.push(InlineSpan {
                    text: text.clone(),
                    color: Some(theme::blue()),
                    weight,
                    monospace: false,
                    strikethrough: strike,
                    link_url: Some(url.clone()),
                });
            }
        }
    }
    spans
}

/// Build a StyledText element from inline spans — uses GPUI native text wrapping
/// instead of flex_row + flex_wrap to avoid layout gaps on fullscreen.
/// Returns (styled_text_element, link_urls) where link_urls maps link index to URL.
fn build_styled_text(spans: &[InlineSpan]) -> (String, Vec<(Range<usize>, HighlightStyle)>, Vec<(Range<usize>, String)>) {
    let mut full_text = String::new();
    let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
    let mut links: Vec<(Range<usize>, String)> = Vec::new();

    for span in spans {
        let start = full_text.len();
        full_text.push_str(&span.text);
        let end = full_text.len();
        if start == end { continue; }

        let mut hl = HighlightStyle::default();
        let mut has_style = false;

        if let Some(c) = span.color {
            hl.color = Some(c.into());
            has_style = true;
        }
        if let Some(w) = span.weight {
            hl.font_weight = Some(w);
            has_style = true;
        }
        if span.monospace {
            hl.background_color = Some(Hsla::from(theme::surface0()));
            has_style = true;
        }
        if span.strikethrough {
            hl.strikethrough = Some(StrikethroughStyle { thickness: px(1.), color: None });
            has_style = true;
        }
        if span.link_url.is_some() {
            hl.color = Some(Hsla::from(theme::blue()));
            hl.underline = Some(UnderlineStyle { thickness: px(1.), color: None, wavy: false });
            has_style = true;
        }

        if has_style {
            highlights.push((start..end, hl));
        }
        if let Some(url) = &span.link_url {
            links.push((start..end, url.clone()));
        }
    }

    (full_text, highlights, links)
}

// ── Syntax highlighting (regex-based) ────────────────────

struct SyntaxToken {
    text: String,
    color: Rgba,
}

fn highlight_code(lang: &str, code: &str) -> Vec<Vec<SyntaxToken>> {
    let lang = lang.to_lowercase();
    code.split('\n').map(|line| highlight_line(&lang, line)).collect()
}

fn highlight_line(lang: &str, line: &str) -> Vec<SyntaxToken> {
    match lang.as_ref() {
        "rust" | "rs" => highlight_rust(line),
        "javascript" | "js" | "jsx" => highlight_js(line),
        "typescript" | "ts" | "tsx" => highlight_ts(line),
        "python" | "py" => highlight_python(line),
        "html" | "xml" | "svg" => highlight_html(line),
        "css" | "scss" | "sass" => highlight_css(line),
        "json" => highlight_json(line),
        "bash" | "sh" | "zsh" | "shell" => highlight_bash(line),
        "sql" => highlight_sql(line),
        "yaml" | "yml" | "toml" => highlight_yaml(line),
        "go" | "swift" | "kotlin" | "java" | "c" | "cpp" | "c++" => highlight_c_like(line),
        _ => vec![SyntaxToken { text: line.to_string(), color: theme::text() }],
    }
}

// Shared tokenizer: scans for strings, comments, numbers, keywords, and idents
fn tokenize_generic(line: &str, keywords: &[&str], types: &[&str], comment_prefix: &str) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut byte_offset = 0usize;

    while i < len {
        // Single-line comment
        if !comment_prefix.is_empty() && line[byte_offset..].starts_with(comment_prefix) {
            tokens.push(SyntaxToken { text: line[byte_offset..].to_string(), color: theme::overlay() });
            return tokens;
        }

        // Strings (double or single quote)
        if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            let mut s = String::new();
            s.push(chars[i]);
            byte_offset += chars[i].len_utf8();
            i += 1;
            while i < len && chars[i] != quote {
                if chars[i] == '\\' && i + 1 < len {
                    s.push(chars[i]);
                    byte_offset += chars[i].len_utf8();
                    i += 1;
                }
                s.push(chars[i]);
                byte_offset += chars[i].len_utf8();
                i += 1;
            }
            if i < len { s.push(chars[i]); byte_offset += chars[i].len_utf8(); i += 1; }
            tokens.push(SyntaxToken { text: s, color: theme::green() });
            continue;
        }

        // Template literals
        if chars[i] == '`' {
            let mut s = String::new();
            s.push(chars[i]);
            byte_offset += chars[i].len_utf8();
            i += 1;
            while i < len && chars[i] != '`' {
                s.push(chars[i]);
                byte_offset += chars[i].len_utf8();
                i += 1;
            }
            if i < len { s.push(chars[i]); byte_offset += chars[i].len_utf8(); i += 1; }
            tokens.push(SyntaxToken { text: s, color: theme::green() });
            continue;
        }

        // Numbers
        if chars[i].is_ascii_digit() || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_ascii_digit()) {
            let mut s = String::new();
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                s.push(chars[i]);
                byte_offset += chars[i].len_utf8();
                i += 1;
            }
            tokens.push(SyntaxToken { text: s, color: theme::peach() });
            continue;
        }

        // Identifiers / keywords
        if chars[i].is_ascii_alphabetic() || chars[i] == '_' || chars[i] == '@' || chars[i] == '$' {
            let mut s = String::new();
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '!' || chars[i] == '?') {
                s.push(chars[i]);
                byte_offset += chars[i].len_utf8();
                i += 1;
            }
            let color = if keywords.contains(&s.as_str()) {
                theme::lavender()
            } else if types.contains(&s.as_str()) {
                theme::yellow()
            } else if s.starts_with('@') || s == "self" || s == "Self" || s == "this" || s == "super" {
                theme::red()
            } else if s.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                theme::yellow()
            } else {
                theme::text()
            };
            tokens.push(SyntaxToken { text: s, color });
            continue;
        }

        // Operators / punctuation
        let mut s = String::new();
        s.push(chars[i]);
        byte_offset += chars[i].len_utf8();
        i += 1;
        tokens.push(SyntaxToken { text: s, color: theme::overlay() });
    }

    tokens
}

fn highlight_rust(line: &str) -> Vec<SyntaxToken> {
    static KW: &[&str] = &[
        "fn", "let", "mut", "const", "static", "pub", "use", "mod", "crate", "extern",
        "struct", "enum", "trait", "impl", "type", "where", "for", "in", "loop", "while",
        "if", "else", "match", "return", "break", "continue", "as", "ref", "move", "async",
        "await", "dyn", "unsafe", "true", "false", "Some", "None", "Ok", "Err",
    ];
    static TY: &[&str] = &[
        "String", "Vec", "Option", "Result", "Box", "Rc", "Arc", "HashMap", "HashSet",
        "bool", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
        "i128", "isize", "f32", "f64", "str", "char",
    ];
    tokenize_generic(line, KW, TY, "//")
}

fn highlight_js(line: &str) -> Vec<SyntaxToken> {
    static KW: &[&str] = &[
        "function", "const", "let", "var", "return", "if", "else", "for", "while", "do",
        "switch", "case", "break", "continue", "new", "delete", "typeof", "instanceof",
        "class", "extends", "import", "export", "from", "default", "async", "await",
        "try", "catch", "finally", "throw", "yield", "of", "in", "true", "false", "null",
        "undefined", "this", "super", "=>",
    ];
    static TY: &[&str] = &["Array", "Object", "Map", "Set", "Promise", "Date", "RegExp", "Error", "JSON", "Math", "console"];
    tokenize_generic(line, KW, TY, "//")
}

fn highlight_ts(line: &str) -> Vec<SyntaxToken> {
    static KW: &[&str] = &[
        "function", "const", "let", "var", "return", "if", "else", "for", "while", "do",
        "switch", "case", "break", "continue", "new", "delete", "typeof", "instanceof",
        "class", "extends", "implements", "import", "export", "from", "default", "async",
        "await", "try", "catch", "finally", "throw", "yield", "of", "in", "true", "false",
        "null", "undefined", "this", "super", "=>", "type", "interface", "enum", "namespace",
        "abstract", "readonly", "as", "is", "keyof", "declare",
    ];
    static TY: &[&str] = &[
        "string", "number", "boolean", "any", "void", "never", "unknown", "object",
        "Array", "Object", "Map", "Set", "Promise", "Date", "RegExp", "Error", "Record",
        "Partial", "Required", "Readonly", "Pick", "Omit",
    ];
    tokenize_generic(line, KW, TY, "//")
}

fn highlight_python(line: &str) -> Vec<SyntaxToken> {
    static KW: &[&str] = &[
        "def", "class", "return", "if", "elif", "else", "for", "while", "break", "continue",
        "import", "from", "as", "try", "except", "finally", "raise", "with", "pass", "lambda",
        "yield", "assert", "global", "nonlocal", "del", "in", "not", "and", "or", "is",
        "True", "False", "None", "async", "await",
    ];
    static TY: &[&str] = &["int", "float", "str", "bool", "list", "dict", "tuple", "set", "type", "bytes", "range", "print", "len", "self"];
    tokenize_generic(line, KW, TY, "#")
}

fn highlight_html(line: &str) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let byte_pos = |idx: usize, chars: &[char]| -> usize { chars[..idx].iter().map(|c| c.len_utf8()).sum() };

    while i < len {
        // Comment
        if line[byte_pos(i, &chars)..].starts_with("<!--") {
            tokens.push(SyntaxToken { text: line[byte_pos(i, &chars)..].to_string(), color: theme::overlay() });
            return tokens;
        }
        // Tag
        if chars[i] == '<' {
            let mut s = String::new();
            s.push(chars[i]); i += 1;
            if i < len && chars[i] == '/' { s.push(chars[i]); i += 1; }
            // tag name
            let mut tag_name = String::new();
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '-') {
                tag_name.push(chars[i]); i += 1;
            }
            if !tag_name.is_empty() {
                tokens.push(SyntaxToken { text: s, color: theme::overlay() });
                tokens.push(SyntaxToken { text: tag_name, color: theme::red() });
            } else {
                tokens.push(SyntaxToken { text: s, color: theme::overlay() });
            }
            // attributes until >
            while i < len && chars[i] != '>' {
                if chars[i] == '"' || chars[i] == '\'' {
                    let q = chars[i];
                    let mut sv = String::new();
                    sv.push(chars[i]); i += 1;
                    while i < len && chars[i] != q { sv.push(chars[i]); i += 1; }
                    if i < len { sv.push(chars[i]); i += 1; }
                    tokens.push(SyntaxToken { text: sv, color: theme::green() });
                } else if chars[i].is_ascii_alphabetic() || chars[i] == '-' {
                    let mut attr = String::new();
                    while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '-') {
                        attr.push(chars[i]); i += 1;
                    }
                    tokens.push(SyntaxToken { text: attr, color: theme::yellow() });
                } else {
                    tokens.push(SyntaxToken { text: chars[i].to_string(), color: theme::overlay() });
                    i += 1;
                }
            }
            if i < len { tokens.push(SyntaxToken { text: ">".to_string(), color: theme::overlay() }); i += 1; }
            continue;
        }
        // Text content
        let mut s = String::new();
        while i < len && chars[i] != '<' { s.push(chars[i]); i += 1; }
        if !s.is_empty() { tokens.push(SyntaxToken { text: s, color: theme::text() }); }
    }
    tokens
}

fn highlight_css(line: &str) -> Vec<SyntaxToken> {
    static KW: &[&str] = &[
        "color", "background", "margin", "padding", "border", "display", "flex", "grid",
        "width", "height", "font", "position", "top", "left", "right", "bottom", "z-index",
        "overflow", "opacity", "transition", "transform", "animation", "none", "auto",
        "inherit", "initial", "!important",
    ];
    tokenize_generic(line, KW, &[], "//")
}

fn highlight_json(line: &str) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut byte_offset = 0usize;

    while i < len {
        if chars[i] == '"' {
            let mut s = String::new();
            s.push(chars[i]); byte_offset += chars[i].len_utf8(); i += 1;
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < len { s.push(chars[i]); byte_offset += chars[i].len_utf8(); i += 1; }
                s.push(chars[i]); byte_offset += chars[i].len_utf8(); i += 1;
            }
            if i < len { s.push(chars[i]); byte_offset += chars[i].len_utf8(); i += 1; }
            // key vs value: key is followed by ':'
            let rest = &line[byte_offset..].trim_start();
            let color = if rest.starts_with(':') { theme::blue() } else { theme::green() };
            tokens.push(SyntaxToken { text: s, color });
        } else if chars[i].is_ascii_digit() || chars[i] == '-' {
            let mut s = String::new();
            while i < len && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '-' || chars[i] == 'e' || chars[i] == 'E') {
                s.push(chars[i]); byte_offset += chars[i].len_utf8(); i += 1;
            }
            tokens.push(SyntaxToken { text: s, color: theme::peach() });
        } else if line[byte_offset..].starts_with("true") || line[byte_offset..].starts_with("false") || line[byte_offset..].starts_with("null") {
            let word = if line[byte_offset..].starts_with("true") { "true" } else if line[byte_offset..].starts_with("false") { "false" } else { "null" };
            tokens.push(SyntaxToken { text: word.to_string(), color: theme::lavender() });
            let word_chars = word.len(); // all ASCII, so byte len == char len
            i += word_chars;
            byte_offset += word.len();
        } else {
            tokens.push(SyntaxToken { text: chars[i].to_string(), color: theme::overlay() });
            byte_offset += chars[i].len_utf8();
            i += 1;
        }
    }
    tokens
}

fn highlight_bash(line: &str) -> Vec<SyntaxToken> {
    static KW: &[&str] = &[
        "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
        "function", "return", "exit", "echo", "cd", "ls", "rm", "cp", "mv", "mkdir", "cat",
        "grep", "sed", "awk", "export", "source", "local", "readonly", "set", "unset",
        "true", "false", "sudo", "apt", "brew", "npm", "cargo", "git", "docker", "curl", "wget",
    ];
    tokenize_generic(line, KW, &[], "#")
}

fn highlight_sql(line: &str) -> Vec<SyntaxToken> {
    static KW: &[&str] = &[
        "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "INSERT", "INTO", "VALUES", "UPDATE",
        "SET", "DELETE", "CREATE", "TABLE", "DROP", "ALTER", "INDEX", "JOIN", "LEFT", "RIGHT",
        "INNER", "OUTER", "ON", "AS", "ORDER", "BY", "GROUP", "HAVING", "LIMIT", "OFFSET",
        "UNION", "ALL", "DISTINCT", "COUNT", "SUM", "AVG", "MIN", "MAX", "NULL", "IS",
        "IN", "BETWEEN", "LIKE", "EXISTS", "CASE", "WHEN", "THEN", "ELSE", "END", "PRIMARY",
        "KEY", "FOREIGN", "REFERENCES", "DEFAULT", "NOT", "CONSTRAINT", "UNIQUE", "CHECK",
        "select", "from", "where", "and", "or", "not", "insert", "into", "values", "update",
        "set", "delete", "create", "table", "drop", "alter", "join", "left", "right", "inner",
        "outer", "on", "as", "order", "by", "group", "having", "limit", "offset", "null", "is",
        "in", "between", "like", "exists", "case", "when", "then", "else", "end", "primary",
        "key", "foreign", "references", "default", "constraint", "unique", "check",
    ];
    static TY: &[&str] = &[
        "INT", "INTEGER", "TEXT", "VARCHAR", "CHAR", "BOOLEAN", "FLOAT", "DOUBLE", "DECIMAL",
        "DATE", "TIMESTAMP", "SERIAL", "BIGINT", "SMALLINT", "UUID", "JSONB", "JSON",
        "int", "integer", "text", "varchar", "char", "boolean", "float", "double", "decimal",
        "date", "timestamp", "serial", "bigint", "smallint", "uuid", "jsonb", "json",
    ];
    tokenize_generic(line, KW, TY, "--")
}

fn highlight_yaml(line: &str) -> Vec<SyntaxToken> {
    let trimmed = line.trim_start();
    // Comment
    if trimmed.starts_with('#') {
        return vec![SyntaxToken { text: line.to_string(), color: theme::overlay() }];
    }
    // Key: value
    if let Some(colon_pos) = trimmed.find(": ") {
        let indent = line.len() - trimmed.len();
        let key_end = indent + colon_pos;
        let mut tokens = Vec::new();
        if indent > 0 {
            tokens.push(SyntaxToken { text: line[..indent].to_string(), color: theme::text() });
        }
        tokens.push(SyntaxToken { text: line[indent..key_end].to_string(), color: theme::blue() });
        tokens.push(SyntaxToken { text: ": ".to_string(), color: theme::overlay() });
        let val = &line[key_end + 2..];
        if val == "true" || val == "false" || val == "null" || val == "~" {
            tokens.push(SyntaxToken { text: val.to_string(), color: theme::lavender() });
        } else if val.starts_with('"') || val.starts_with('\'') {
            tokens.push(SyntaxToken { text: val.to_string(), color: theme::green() });
        } else if val.chars().next().map(|c| c.is_ascii_digit() || c == '-').unwrap_or(false) {
            tokens.push(SyntaxToken { text: val.to_string(), color: theme::peach() });
        } else {
            tokens.push(SyntaxToken { text: val.to_string(), color: theme::text() });
        }
        return tokens;
    }
    // List item
    if trimmed.starts_with("- ") {
        let indent = line.len() - trimmed.len();
        let mut tokens = Vec::new();
        if indent > 0 {
            tokens.push(SyntaxToken { text: line[..indent].to_string(), color: theme::text() });
        }
        tokens.push(SyntaxToken { text: "- ".to_string(), color: theme::overlay() });
        tokens.push(SyntaxToken { text: trimmed[2..].to_string(), color: theme::text() });
        return tokens;
    }
    vec![SyntaxToken { text: line.to_string(), color: theme::text() }]
}

fn highlight_c_like(line: &str) -> Vec<SyntaxToken> {
    static KW: &[&str] = &[
        "if", "else", "for", "while", "do", "switch", "case", "break", "continue", "return",
        "class", "struct", "enum", "interface", "func", "fn", "var", "let", "const", "val",
        "new", "delete", "import", "package", "public", "private", "protected", "static",
        "final", "abstract", "override", "virtual", "void", "null", "nil", "true", "false",
        "try", "catch", "throw", "throws", "finally", "defer", "go", "chan", "select",
        "async", "await", "this", "self", "super", "extends", "implements",
    ];
    static TY: &[&str] = &[
        "int", "float", "double", "char", "bool", "byte", "short", "long", "string",
        "String", "Int", "Float", "Double", "Bool", "Any", "Void",
    ];
    tokenize_generic(line, KW, TY, "//")
}

// ── Height estimation (for viewport virtualization) ──────

/// Estimate the rendered height of a block in pixels.
/// Approximate — doesn't need to be exact, just close enough for virtualization.
const EST_LINE_H: f32 = 24.0;
const EST_CHARS_PER_LINE: f32 = 80.0;
const EST_GAP: f32 = 4.0;

fn estimate_block_height(block: &MdBlock, container_w: f32) -> f32 {
    match block {
        MdBlock::Heading(level, inlines) => {
            let text_len = inline_text_len(inlines) as f32;
            let font_h = match level { 1 => 36.0, 2 => 30.0, 3 => 26.0, _ => 22.0 };
            let chars_per_line = container_w / (font_h * 0.55);
            let lines = (text_len / chars_per_line).ceil().max(1.0);
            12.0 + lines * font_h + 8.0 + if *level <= 2 { 1.0 } else { 0.0 }
        }
        MdBlock::Paragraph(inlines) => {
            let text_len = inline_text_len(inlines) as f32;
            let lines = (text_len / EST_CHARS_PER_LINE).ceil().max(1.0);
            lines * EST_LINE_H
        }
        MdBlock::CodeBlock(_, code) => {
            let code_lines = code.lines().count().max(1) as f32;
            24.0 + code_lines * 20.0 + 16.0 // header + lines + padding
        }
        MdBlock::UnorderedList(items) | MdBlock::OrderedList(items) => {
            let mut h = 0.0;
            for item in items {
                let text_len = inline_text_len(&item.content) as f32;
                let lines = (text_len / EST_CHARS_PER_LINE).ceil().max(1.0);
                h += lines * EST_LINE_H + 2.0;
                for child in &item.children {
                    h += estimate_block_height(child, container_w - 16.0);
                }
            }
            h
        }
        MdBlock::HorizontalRule => 25.0,
        MdBlock::Blockquote(inner) => {
            let mut h = 8.0; // py
            for b in inner {
                h += estimate_block_height(b, container_w - 16.0) + EST_GAP;
            }
            h
        }
        MdBlock::Table(rows) => {
            rows.len() as f32 * 36.0 + 2.0 // ~36px per row + border
        }
    }
}

fn inline_text_len(inlines: &[MdInline]) -> usize {
    inlines.iter().map(|il| match il {
        MdInline::Text(t) => t.len(),
        MdInline::Bold(c) | MdInline::Italic(c) | MdInline::BoldItalic(c) | MdInline::Strikethrough(c) => inline_text_len(c),
        MdInline::Code(t) => t.len(),
        MdInline::Link(t, _) => t.len(),
    }).sum()
}

// ── View ──────────────────────────────────────────────────

pub struct MarkdownPreviewView {
    path: PathBuf,
    blocks: Vec<MdBlock>,
    block_heights: Vec<f32>,
    /// Per-table horizontal scroll offsets, keyed by block index
    table_scroll_x: std::collections::HashMap<usize, f32>,
    /// Width of each table (total content width), keyed by block index
    table_widths: std::collections::HashMap<usize, f32>,
    scroll_handle: ScrollHandle,
}

impl MarkdownPreviewView {
    pub fn new(path: PathBuf) -> Self {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let blocks = parse_markdown(&content);
        let container_w = 800.0 - 64.0; // max_w minus padding
        let block_heights: Vec<f32> = blocks.iter()
            .map(|b| estimate_block_height(b, container_w))
            .collect();
        Self {
            path,
            blocks,
            block_heights,
            table_scroll_x: std::collections::HashMap::new(),
            table_widths: std::collections::HashMap::new(),
            scroll_handle: ScrollHandle::new(),
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    fn render_blocks(&mut self, cx: &mut Context<Self>) -> Div {
        let total_blocks = self.blocks.len();

        // Viewport virtualization: only render visible blocks ± buffer
        let scroll_y: f32 = (-self.scroll_handle.offset().y).into();
        let viewport_h: f32 = self.scroll_handle.bounds().size.height.into();
        let padding_top = 32.0_f32;

        let (first, last, top_spacer_h, bottom_spacer_h) = if viewport_h > 0.0 && total_blocks > 30 {
            let buffer_px = 500.0_f32;
            let view_top = (scroll_y - buffer_px).max(0.0);
            let view_bottom = scroll_y + viewport_h + buffer_px;

            let mut cumulative = padding_top;
            let mut first_idx = total_blocks;
            let mut last_idx = total_blocks;

            for (i, h) in self.block_heights.iter().enumerate() {
                let block_bottom = cumulative + h + EST_GAP;
                if first_idx == total_blocks && block_bottom > view_top {
                    first_idx = i;
                }
                if cumulative > view_bottom {
                    last_idx = i;
                    break;
                }
                cumulative += h + EST_GAP;
            }

            if first_idx == total_blocks { first_idx = 0; }
            if last_idx == total_blocks { last_idx = total_blocks; }

            // Compute spacer heights
            let top_h: f32 = self.block_heights[..first_idx].iter()
                .map(|h| h + EST_GAP).sum();
            let bottom_h: f32 = self.block_heights[last_idx..].iter()
                .map(|h| h + EST_GAP).sum();

            (first_idx, last_idx, top_h, bottom_h)
        } else {
            // Small document — render everything
            (0, total_blocks, 0.0, 0.0)
        };

        // Explicit container width for proper text wrapping in fullscreen
        let scroll_w: f32 = self.scroll_handle.bounds().size.width.into();
        let container_w = if scroll_w > 0.0 { scroll_w } else { 800.0 };

        let mut container = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(container_w))
            .p(px(32.))
            .gap(px(4.));

        if top_spacer_h > 0.0 {
            container = container.child(div().h(px(top_spacer_h)).flex_shrink_0());
        }

        let blocks = self.blocks.clone();
        for bi in first..last {
            container = container.child(self.render_block(&blocks[bi], &format!("b{}", bi), cx));
        }

        if bottom_spacer_h > 0.0 {
            container = container.child(div().h(px(bottom_spacer_h)).flex_shrink_0());
        }

        container
    }

    fn render_block(&mut self, block: &MdBlock, id_prefix: &str, cx: &mut Context<Self>) -> Div {
        match block {
            MdBlock::Heading(level, inlines) => {
                let (size, weight) = match level {
                    1 => (px(28.), FontWeight::BOLD),
                    2 => (px(22.), FontWeight::BOLD),
                    3 => (px(18.), FontWeight::SEMIBOLD),
                    4 => (px(16.), FontWeight::SEMIBOLD),
                    _ => (px(14.), FontWeight::SEMIBOLD),
                };
                let spans = flatten_inlines(inlines, false, false, false);
                let (text, highlights, links) = build_styled_text(&spans);

                let styled = StyledText::new(text.clone()).with_highlights(highlights);

                let mut heading = div()
                    .w_full()
                    .text_size(size)
                    .font_weight(weight)
                    .text_color(theme::text())
                    .mt(px(12.))
                    .pb(px(4.));
                if *level <= 2 {
                    heading = heading
                        .border_b_1()
                        .border_color(theme::surface1())
                        .pb(px(8.));
                }

                if links.is_empty() {
                    heading.child(styled)
                } else {
                    let link_ranges: Vec<Range<usize>> = links.iter().map(|(r, _)| r.clone()).collect();
                    let link_urls: Vec<String> = links.iter().map(|(_, u)| u.clone()).collect();
                    let id = ElementId::Name(format!("{}-heading", id_prefix).into());
                    heading.child(
                        InteractiveText::new(id, styled)
                            .on_click(link_ranges, move |idx, _window, _cx| {
                                if let Some(url) = link_urls.get(idx) {
                                    let _ = std::process::Command::new("open").arg(url).spawn();
                                }
                            })
                    )
                }
            }

            MdBlock::Paragraph(inlines) => {
                let spans = flatten_inlines(inlines, false, false, false);
                let (text, highlights, links) = build_styled_text(&spans);

                let styled = StyledText::new(text.clone()).with_highlights(highlights);

                let para = div()
                    .w_full()
                    .text_color(theme::text())
                    .line_height(px(24.));

                if links.is_empty() {
                    para.child(styled)
                } else {
                    let link_ranges: Vec<Range<usize>> = links.iter().map(|(r, _)| r.clone()).collect();
                    let link_urls: Vec<String> = links.iter().map(|(_, u)| u.clone()).collect();
                    let id = ElementId::Name(format!("{}-para", id_prefix).into());
                    para.child(
                        InteractiveText::new(id, styled)
                            .on_click(link_ranges, move |idx, _window, _cx| {
                                if let Some(url) = link_urls.get(idx) {
                                    let _ = std::process::Command::new("open").arg(url).spawn();
                                }
                            })
                    )
                }
            }

            MdBlock::CodeBlock(lang, code) => {
                let mut block_div = div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .bg(theme::surface0())
                    .rounded(px(6.))
                    .border_1()
                    .border_color(theme::surface1());

                if !lang.is_empty() {
                    block_div = block_div.child(
                        div()
                            .px(px(12.))
                            .py(px(4.))
                            .text_xs()
                            .text_color(theme::subtext())
                            .border_b_1()
                            .border_color(theme::surface1())
                            .child(lang.clone()),
                    );
                }

                let highlighted = highlight_code(lang, code);

                let mut code_div = div()
                    .flex()
                    .flex_col()
                    .px(px(16.))
                    .py(px(12.))
                    .text_sm()
                    .font_family("Berkeley Mono, SF Mono, Menlo, monospace");

                for line_tokens in &highlighted {
                    let mut line_div = div().flex().flex_row().h(px(20.));
                    if line_tokens.is_empty() {
                        line_div = line_div.child(" ");
                    } else {
                        for token in line_tokens {
                            line_div = line_div.child(
                                div().text_color(token.color).child(token.text.clone())
                            );
                        }
                    }
                    code_div = code_div.child(line_div);
                }

                block_div.child(code_div)
            }

            MdBlock::UnorderedList(items) => {
                let mut list = div().flex().flex_col().pl(px(16.)).gap(px(2.));
                for (ii, item) in items.iter().enumerate() {
                    let item_id = format!("{}-ul{}", id_prefix, ii);
                    let spans = flatten_inlines(&item.content, false, false, false);
                    let (text, highlights, links) = build_styled_text(&spans);
                    let styled = StyledText::new(text.clone()).with_highlights(highlights);

                    let bullet = match item.checkbox {
                        Some(true) => div().flex_shrink_0().mr(px(8.)).text_color(theme::green()).child("\u{2611}"),
                        Some(false) => div().flex_shrink_0().mr(px(8.)).text_color(theme::overlay()).child("\u{2610}"),
                        None => div().flex_shrink_0().mr(px(8.)).text_color(theme::overlay()).child("\u{2022}"),
                    };

                    let content_div = div().flex_1().min_w(px(0.)).text_color(theme::text());
                    let content_div = if links.is_empty() {
                        content_div.child(styled)
                    } else {
                        let link_ranges: Vec<Range<usize>> = links.iter().map(|(r, _)| r.clone()).collect();
                        let link_urls: Vec<String> = links.iter().map(|(_, u)| u.clone()).collect();
                        let id = ElementId::Name(format!("{}-text", item_id).into());
                        content_div.child(
                            InteractiveText::new(id, styled)
                                .on_click(link_ranges, move |idx, _w, _cx| {
                                    if let Some(url) = link_urls.get(idx) {
                                        let _ = std::process::Command::new("open").arg(url).spawn();
                                    }
                                })
                        )
                    };

                    let row = div().flex().flex_row().items_start()
                        .child(bullet)
                        .child(content_div);

                    if item.children.is_empty() {
                        list = list.child(row);
                    } else {
                        let mut item_container = div().flex().flex_col();
                        item_container = item_container.child(row);
                        for (ci, child) in item.children.iter().enumerate() {
                            let child_id = format!("{}-c{}", item_id, ci);
                            item_container = item_container.child(self.render_block(child, &child_id, cx));
                        }
                        list = list.child(item_container);
                    }
                }
                list
            }

            MdBlock::OrderedList(items) => {
                let mut list = div().flex().flex_col().pl(px(16.)).gap(px(2.));
                for (i, item) in items.iter().enumerate() {
                    let item_id = format!("{}-ol{}", id_prefix, i);
                    let spans = flatten_inlines(&item.content, false, false, false);
                    let (text, highlights, links) = build_styled_text(&spans);
                    let styled = StyledText::new(text.clone()).with_highlights(highlights);

                    let marker = match item.checkbox {
                        Some(true) => div().flex_shrink_0().mr(px(8.)).text_color(theme::green()).child(format!("{}. \u{2611}", i + 1)),
                        Some(false) => div().flex_shrink_0().mr(px(8.)).text_color(theme::overlay()).child(format!("{}. \u{2610}", i + 1)),
                        None => div().flex_shrink_0().mr(px(8.)).min_w(px(18.)).text_right().text_color(theme::overlay()).child(format!("{}.", i + 1)),
                    };

                    let content_div = div().flex_1().min_w(px(0.)).text_color(theme::text());
                    let content_div = if links.is_empty() {
                        content_div.child(styled)
                    } else {
                        let link_ranges: Vec<Range<usize>> = links.iter().map(|(r, _)| r.clone()).collect();
                        let link_urls: Vec<String> = links.iter().map(|(_, u)| u.clone()).collect();
                        let id = ElementId::Name(format!("{}-text", item_id).into());
                        content_div.child(
                            InteractiveText::new(id, styled)
                                .on_click(link_ranges, move |idx, _w, _cx| {
                                    if let Some(url) = link_urls.get(idx) {
                                        let _ = std::process::Command::new("open").arg(url).spawn();
                                    }
                                })
                        )
                    };

                    let row = div().flex().flex_row().items_start()
                        .child(marker)
                        .child(content_div);

                    if item.children.is_empty() {
                        list = list.child(row);
                    } else {
                        let mut item_container = div().flex().flex_col();
                        item_container = item_container.child(row);
                        for (ci, child) in item.children.iter().enumerate() {
                            let child_id = format!("{}-c{}", item_id, ci);
                            item_container = item_container.child(self.render_block(child, &child_id, cx));
                        }
                        list = list.child(item_container);
                    }
                }
                list
            }

            MdBlock::HorizontalRule => {
                div()
                    .w_full()
                    .h(px(1.))
                    .my(px(12.))
                    .bg(theme::surface1())
            }

            MdBlock::Blockquote(inner_blocks) => {
                let mut quote = div()
                    .flex()
                    .flex_col()
                    .pl(px(16.))
                    .py(px(4.))
                    .gap(px(4.))
                    .border_l_2()
                    .border_color(theme::blue())
                    .text_color(theme::subtext());

                let blocks = inner_blocks.clone();
                for (bi, inner) in blocks.iter().enumerate() {
                    let inner_id = format!("{}-bq{}", id_prefix, bi);
                    quote = quote.child(self.render_block(inner, &inner_id, cx));
                }
                quote
            }

            MdBlock::Table(rows) => {
                if rows.is_empty() { return div(); }
                let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);

                // Calculate column widths based on content
                let mut col_max_len: Vec<usize> = vec![0; col_count];
                for row in rows {
                    for (ci, cell) in row.iter().enumerate() {
                        let len: usize = cell.iter().map(|il| match il {
                            MdInline::Text(t) => t.len(),
                            MdInline::Bold(c) | MdInline::Italic(c) | MdInline::BoldItalic(c) | MdInline::Strikethrough(c) =>
                                c.iter().map(|x| if let MdInline::Text(t) = x { t.len() } else { 4 }).sum(),
                            MdInline::Code(t) => t.len(),
                            MdInline::Link(t, _) => t.len(),
                        }).sum();
                        col_max_len[ci] = col_max_len[ci].max(len);
                    }
                }
                let col_px: Vec<f32> = col_max_len.iter()
                    .map(|len| (*len as f32 * 7.5 + 24.0).clamp(48.0, 300.0))
                    .collect();
                let total_table_w: f32 = col_px.iter().sum::<f32>() + 2.0;

                // Extract block index from id_prefix (e.g. "b5") for scroll state
                let block_idx: usize = id_prefix.trim_start_matches('b')
                    .split('-').next().unwrap_or("0")
                    .parse().unwrap_or(0);
                let scroll_x = self.table_scroll_x.get(&block_idx).copied().unwrap_or(0.0);
                self.table_widths.insert(block_idx, total_table_w);

                let mut table = div()
                    .flex()
                    .flex_col()
                    .w(px(total_table_w))
                    .flex_shrink_0()
                    .border_1()
                    .border_color(theme::surface1())
                    .rounded(px(4.))
                    .ml(px(-scroll_x));

                for (row_idx, row) in rows.iter().enumerate() {
                    let is_header = row_idx == 0;
                    let mut row_div = div().flex().flex_row();
                    if is_header {
                        row_div = row_div.bg(theme::surface0());
                    }
                    if row_idx > 0 {
                        row_div = row_div.border_t_1().border_color(theme::surface1());
                    }

                    for col_idx in 0..col_count {
                        let cell_inlines = row.get(col_idx).cloned().unwrap_or_default();
                        let spans = flatten_inlines(&cell_inlines, is_header, false, false);
                        let (text, highlights, _links) = build_styled_text(&spans);
                        let styled = StyledText::new(text).with_highlights(highlights);

                        let mut cell = div()
                            .w(px(col_px.get(col_idx).copied().unwrap_or(100.0)))
                            .flex_shrink_0()
                            .px(px(12.))
                            .py(px(6.))
                            .text_sm()
                            .text_color(theme::text());

                        if col_idx > 0 {
                            cell = cell.border_l_1().border_color(theme::surface1());
                        }
                        cell = cell.child(styled);
                        row_div = row_div.child(cell);
                    }

                    table = table.child(row_div);
                }

                // Available content width for table clipping
                let viewport_w: f32 = {
                    let w: f32 = self.scroll_handle.bounds().size.width.into();
                    if w > 0.0 { (w - 64.0).max(200.0) } else { 736.0 }
                };
                let max_scroll_x = (total_table_w - viewport_w).max(0.0);
                let has_h_scroll = max_scroll_x > 0.0;

                let table_eid = ElementId::Name(format!("table-{}", block_idx).into());
                let mut table_wrapper = div()
                    .id(table_eid)
                    .w_full()
                    .overflow_x_hidden();

                if has_h_scroll {
                    table_wrapper = table_wrapper.on_scroll_wheel(
                        cx.listener(move |this, ev: &ScrollWheelEvent, _window, cx| {
                            let (dx, dy): (f32, f32) = match &ev.delta {
                                ScrollDelta::Lines(d) => (d.x * 20.0, d.y * 20.0),
                                ScrollDelta::Pixels(d) => {
                                    let x: f32 = d.x.into();
                                    let y: f32 = d.y.into();
                                    (x, y)
                                }
                            };
                            if dx.abs() > dy.abs() * 3.0 {
                                let tw = this.table_widths.get(&block_idx).copied().unwrap_or(0.0);
                                let vw: f32 = {
                                    let w: f32 = this.scroll_handle.bounds().size.width.into();
                                    if w > 0.0 { (w - 64.0).max(200.0) } else { 736.0 }
                                };
                                let max = (tw - vw).max(0.0);
                                let cur = this.table_scroll_x.get(&block_idx).copied().unwrap_or(0.0);
                                this.table_scroll_x.insert(block_idx, (cur - dx).clamp(0.0, max));
                                cx.stop_propagation();
                                cx.notify();
                            }
                        })
                    );
                }

                table_wrapper = table_wrapper.child(table);

                // Horizontal scrollbar rail for wide tables
                if has_h_scroll {
                    let ratio = viewport_w / total_table_w;
                    let thumb_w = (ratio * viewport_w).max(30.0);
                    let track_space = viewport_w - thumb_w;
                    let thumb_x = if max_scroll_x > 0.0 {
                        scroll_x / max_scroll_x * track_space
                    } else {
                        0.0
                    };
                    table_wrapper = table_wrapper.child(
                        div()
                            .w_full()
                            .h(px(4.))
                            .mt(px(4.))
                            .rounded(px(2.))
                            .bg(theme::surface0())
                            .relative()
                            .child(
                                div()
                                    .absolute()
                                    .left(px(thumb_x))
                                    .top_0()
                                    .w(px(thumb_w))
                                    .h_full()
                                    .rounded(px(2.))
                                    .bg(theme::overlay())
                            )
                    );
                }

                div().w_full().child(table_wrapper)
            }
        }
    }
}

impl Render for MarkdownPreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("markdown-preview")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll_handle)
            .on_scroll_wheel(cx.listener(|_this, _ev: &ScrollWheelEvent, _window, cx| {
                cx.notify();
            }))
            .bg(theme::base())
            .font_family("SF Pro Display, Helvetica Neue, sans-serif")
            .child(self.render_blocks(cx))
    }
}
