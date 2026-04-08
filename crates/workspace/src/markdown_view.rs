use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;

use crate::theme;

// ── Markdown AST ──────────────────────────────────────────

#[derive(Clone, Debug)]
enum MdBlock {
    Heading(u8, Vec<MdInline>),
    Paragraph(Vec<MdInline>),
    CodeBlock(String, String),           // language, code
    UnorderedList(Vec<Vec<MdInline>>),
    OrderedList(Vec<Vec<MdInline>>),
    HorizontalRule,
    Blockquote(Vec<MdInline>),
    Table(Vec<Vec<String>>),             // rows of cells (first row = header)
}

#[derive(Clone, Debug)]
enum MdInline {
    Text(String),
    Bold(Vec<MdInline>),
    Italic(Vec<MdInline>),
    BoldItalic(Vec<MdInline>),
    Code(String),
    Link(String, String), // text, url
}

// ── Parser ────────────────────────────────────────────────

fn parse_markdown(input: &str) -> Vec<MdBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
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
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                if !code.is_empty() { code.push('\n'); }
                code.push_str(lines[i]);
                i += 1;
            }
            if i < lines.len() { i += 1; }
            blocks.push(MdBlock::CodeBlock(lang, code));
            continue;
        }

        // Heading
        if line.starts_with('#') {
            let level = line.chars().take_while(|c| *c == '#').count().min(6) as u8;
            let text = line[(level as usize)..].trim_start();
            blocks.push(MdBlock::Heading(level, parse_inline(text)));
            i += 1;
            continue;
        }

        // Table: detect if current line + next lines form a table
        if line.contains('|') && i + 1 < lines.len() && is_table_separator(lines[i + 1]) {
            let mut rows: Vec<Vec<String>> = Vec::new();
            // Header row
            rows.push(parse_table_row(line));
            i += 1; // skip separator
            i += 1;
            // Data rows
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                rows.push(parse_table_row(lines[i]));
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

        // Blockquote
        if line.trim_start().starts_with("> ") || line.trim_start() == ">" {
            let text = line.trim_start().trim_start_matches('>').trim();
            blocks.push(MdBlock::Blockquote(parse_inline(text)));
            i += 1;
            continue;
        }

        // Unordered list
        if line.trim_start().starts_with("- ") || line.trim_start().starts_with("* ") {
            let mut items = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim_start();
                if l.starts_with("- ") || l.starts_with("* ") {
                    items.push(parse_inline(&l[2..]));
                    i += 1;
                } else {
                    break;
                }
            }
            blocks.push(MdBlock::UnorderedList(items));
            continue;
        }

        // Ordered list
        if line.trim_start().chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
            && line.trim_start().contains(". ")
        {
            let mut items = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim_start();
                if let Some(dot_pos) = l.find(". ") {
                    if l[..dot_pos].chars().all(|c| c.is_ascii_digit()) {
                        items.push(parse_inline(&l[dot_pos + 2..]));
                        i += 1;
                        continue;
                    }
                }
                break;
            }
            blocks.push(MdBlock::OrderedList(items));
            continue;
        }

        // Paragraph (collect consecutive non-empty lines)
        let mut para = String::new();
        while i < lines.len() && !lines[i].trim().is_empty()
            && !lines[i].starts_with('#')
            && !lines[i].trim_start().starts_with("```")
            && !lines[i].trim_start().starts_with("- ")
            && !lines[i].trim_start().starts_with("* ")
            && !lines[i].trim_start().starts_with("> ")
        {
            if !para.is_empty() { para.push(' '); }
            para.push_str(lines[i].trim());
            i += 1;
        }
        if !para.is_empty() {
            blocks.push(MdBlock::Paragraph(parse_inline(&para)));
        }
    }

    blocks
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
        // Inline code (highest priority — not nested)
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

        // Bold+Italic (***text***) or Bold (**text**) or Italic (*text*)
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

// ── Inline flattening (to plain text + highlight ranges) ──

#[derive(Clone)]
struct InlineSpan {
    text: String,
    color: Option<Rgba>,
    weight: Option<FontWeight>,
    monospace: bool,
}

fn flatten_inlines(inlines: &[MdInline], bold: bool, italic: bool) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    for inline in inlines {
        match inline {
            MdInline::Text(text) => {
                let color = if italic { Some(theme::subtext()) } else { None };
                let weight = if bold { Some(FontWeight::BOLD) } else { None };
                spans.push(InlineSpan { text: text.clone(), color, weight, monospace: false });
            }
            MdInline::Bold(children) => {
                spans.extend(flatten_inlines(children, true, italic));
            }
            MdInline::Italic(children) => {
                spans.extend(flatten_inlines(children, bold, true));
            }
            MdInline::BoldItalic(children) => {
                spans.extend(flatten_inlines(children, true, true));
            }
            MdInline::Code(text) => {
                spans.push(InlineSpan {
                    text: text.clone(),
                    color: Some(theme::peach()),
                    weight: None,
                    monospace: true,
                });
            }
            MdInline::Link(text, _url) => {
                let weight = if bold { Some(FontWeight::BOLD) } else { None };
                spans.push(InlineSpan { text: text.clone(), color: Some(theme::blue()), weight, monospace: false });
            }
        }
    }
    spans
}

/// Split spans into word-level divs for proper flex-wrap
fn render_inline_spans(spans: &[InlineSpan]) -> Vec<Div> {
    let mut divs = Vec::new();

    for span in spans {
        if span.monospace {
            // Code spans: render as single unit, no bg to avoid artifacts
            let mut d = div()
                .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
                .text_sm()
                .text_color(span.color.unwrap_or(theme::text()));
            if let Some(w) = span.weight {
                d = d.font_weight(w);
            }
            d = d.child(span.text.clone());
            divs.push(d);
        } else {
            // Text: split into words so flex_wrap can break lines
            let words = split_preserving_spaces(&span.text);
            for word in words {
                let mut d = div();
                if let Some(c) = span.color {
                    d = d.text_color(c);
                }
                if let Some(w) = span.weight {
                    d = d.font_weight(w);
                }
                d = d.child(word);
                divs.push(d);
            }
        }
    }

    divs
}

/// Split text into chunks that preserve spacing for flex layout.
/// "hello world" → ["hello ", "world"]
fn split_preserving_spaces(text: &str) -> Vec<String> {
    if text.is_empty() { return vec![]; }
    let mut chunks = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch == ' ' {
            current.push(ch);
            chunks.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

// ── View ──────────────────────────────────────────────────

pub struct MarkdownPreviewView {
    path: PathBuf,
    blocks: Vec<MdBlock>,
    scroll_handle: ScrollHandle,
}

impl MarkdownPreviewView {
    pub fn new(path: PathBuf) -> Self {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let blocks = parse_markdown(&content);
        Self {
            path,
            blocks,
            scroll_handle: ScrollHandle::new(),
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    fn render_blocks(&self, _cx: &mut Context<Self>) -> Div {
        let mut container = div()
            .flex()
            .flex_col()
            .p(px(32.))
            .gap(px(4.))
            .max_w(px(800.));

        for block in &self.blocks {
            container = container.child(self.render_block(block));
        }

        container
    }

    fn render_block(&self, block: &MdBlock) -> Div {
        match block {
            MdBlock::Heading(level, inlines) => {
                let (size, weight) = match level {
                    1 => (px(28.), FontWeight::BOLD),
                    2 => (px(22.), FontWeight::BOLD),
                    3 => (px(18.), FontWeight::SEMIBOLD),
                    4 => (px(16.), FontWeight::SEMIBOLD),
                    _ => (px(14.), FontWeight::SEMIBOLD),
                };
                let spans = flatten_inlines(inlines, false, false);
                let word_divs = render_inline_spans(&spans);

                let mut heading = div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_baseline()
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
                for d in word_divs {
                    heading = heading.child(d);
                }
                heading
            }

            MdBlock::Paragraph(inlines) => {
                let spans = flatten_inlines(inlines, false, false);
                let word_divs = render_inline_spans(&spans);

                let mut para = div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_baseline()
                    .text_color(theme::text())
                    .line_height(px(24.));
                for d in word_divs {
                    para = para.child(d);
                }
                para
            }

            MdBlock::CodeBlock(lang, code) => {
                let mut block = div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .bg(theme::surface0())
                    .rounded(px(6.))
                    .border_1()
                    .border_color(theme::surface1());

                if !lang.is_empty() {
                    block = block.child(
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

                // Render code lines individually for proper display
                let mut code_div = div()
                    .flex()
                    .flex_col()
                    .px(px(16.))
                    .py(px(12.))
                    .text_sm()
                    .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
                    .text_color(theme::text());

                for line in code.split('\n') {
                    code_div = code_div.child(
                        div().h(px(20.)).child(if line.is_empty() { " ".to_string() } else { line.to_string() })
                    );
                }

                block.child(code_div)
            }

            MdBlock::UnorderedList(items) => {
                let mut list = div().flex().flex_col().pl(px(16.)).gap(px(2.));
                for item in items {
                    let spans = flatten_inlines(item, false, false);
                    let word_divs = render_inline_spans(&spans);
                    let mut row = div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_baseline()
                        .text_color(theme::text())
                        .child(
                            div()
                                .text_color(theme::overlay())
                                .mr(px(8.))
                                .child("\u{2022}"),
                        );
                    for d in word_divs {
                        row = row.child(d);
                    }
                    list = list.child(row);
                }
                list
            }

            MdBlock::OrderedList(items) => {
                let mut list = div().flex().flex_col().pl(px(16.)).gap(px(2.));
                for (i, item) in items.iter().enumerate() {
                    let spans = flatten_inlines(item, false, false);
                    let word_divs = render_inline_spans(&spans);
                    let mut row = div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_baseline()
                        .text_color(theme::text())
                        .child(
                            div()
                                .text_color(theme::overlay())
                                .mr(px(8.))
                                .min_w(px(18.))
                                .text_right()
                                .child(format!("{}.", i + 1)),
                        );
                    for d in word_divs {
                        row = row.child(d);
                    }
                    list = list.child(row);
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

            MdBlock::Blockquote(inlines) => {
                let spans = flatten_inlines(inlines, false, false);
                let word_divs = render_inline_spans(&spans);
                let mut quote = div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_baseline()
                    .pl(px(16.))
                    .py(px(4.))
                    .border_l_2()
                    .border_color(theme::blue())
                    .text_color(theme::subtext());
                for d in word_divs {
                    quote = quote.child(d);
                }
                quote
            }

            MdBlock::Table(rows) => {
                if rows.is_empty() { return div(); }
                let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);

                let mut table = div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .border_1()
                    .border_color(theme::surface1())
                    .rounded(px(4.))
                    .overflow_hidden();

                for (row_idx, row) in rows.iter().enumerate() {
                    let is_header = row_idx == 0;
                    let mut row_div = div()
                        .flex()
                        .flex_row()
                        .w_full();
                    if is_header {
                        row_div = row_div.bg(theme::surface0());
                    }
                    if row_idx > 0 {
                        row_div = row_div.border_t_1().border_color(theme::surface1());
                    }

                    for col_idx in 0..col_count {
                        let cell_text = row.get(col_idx).cloned().unwrap_or_default();
                        let mut cell = div()
                            .flex_1()
                            .px(px(12.))
                            .py(px(6.))
                            .text_sm()
                            .text_color(theme::text())
                            .child(cell_text);
                        if is_header {
                            cell = cell.font_weight(FontWeight::BOLD);
                        }
                        if col_idx > 0 {
                            cell = cell.border_l_1().border_color(theme::surface1());
                        }
                        row_div = row_div.child(cell);
                    }

                    table = table.child(row_div);
                }

                table
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
            .bg(theme::base())
            .font_family("SF Pro Display, Helvetica Neue, sans-serif")
            .child(self.render_blocks(cx))
    }
}
