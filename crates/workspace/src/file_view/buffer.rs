/// Text buffer with undo/redo support.

#[derive(Clone, Debug)]
pub enum EditOp {
    Insert { row: usize, col: usize, text: String },
    Delete { row: usize, col: usize, text: String },
    Compound(Vec<EditOp>),
}

pub struct Buffer {
    pub lines: Vec<String>,
    undo_stack: Vec<EditOp>,
    redo_stack: Vec<EditOp>,
    /// For coalescing rapid single-char inserts
    coalesce_row: usize,
    coalesce_col: usize,
}

impl Buffer {
    pub fn new(content: &str) -> Self {
        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self {
            lines,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            coalesce_row: usize::MAX,
            coalesce_col: usize::MAX,
        }
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line(&self, row: usize) -> &str {
        &self.lines[row]
    }

    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    // ── Mutations ───────────────────────────────────

    pub fn insert_text(&mut self, row: usize, col: usize, text: &str) {
        // Coalesce single-char inserts at consecutive positions
        if text.len() == 1 && !text.contains('\n') && row == self.coalesce_row && col == self.coalesce_col {
            if let Some(EditOp::Insert { text: ref mut prev, .. }) = self.undo_stack.last_mut() {
                prev.push_str(text);
                self.apply_insert(row, col, text);
                self.coalesce_col = col + text.len();
                return;
            }
        }

        let op = EditOp::Insert { row, col, text: text.to_string() };
        self.undo_stack.push(op);
        self.redo_stack.clear();
        self.apply_insert(row, col, text);
        self.coalesce_row = row;
        self.coalesce_col = col + text.len();
    }

    pub fn delete_range(&mut self, sr: usize, sc: usize, er: usize, ec: usize) -> String {
        let text = self.extract_range(sr, sc, er, ec);
        let op = EditOp::Delete { row: sr, col: sc, text: text.clone() };
        self.undo_stack.push(op);
        self.redo_stack.clear();
        self.apply_delete(sr, sc, er, ec);
        self.break_coalesce();
        text
    }

    pub fn insert_newline(&mut self, row: usize, col: usize) -> (usize, usize) {
        self.insert_text(row, col, "\n");
        self.break_coalesce();
        (row + 1, 0)
    }

    pub fn insert_newline_with_indent(&mut self, row: usize, col: usize) -> (usize, usize) {
        let leading_ws: String = self.lines[row].chars().take_while(|c| c.is_whitespace()).collect();

        // Extra indent if line ends with { ( [ :
        let line_trimmed = self.lines[row][..col].trim_end();
        let extra = if line_trimmed.ends_with('{') || line_trimmed.ends_with('(')
            || line_trimmed.ends_with('[') || line_trimmed.ends_with(':') {
            "    "
        } else {
            ""
        };

        let insert = format!("\n{}{}", leading_ws, extra);
        let new_col = leading_ws.len() + extra.len();
        self.insert_text(row, col, &insert);
        self.break_coalesce();
        (row + 1, new_col)
    }

    pub fn duplicate_line(&mut self, row: usize) {
        if row >= self.lines.len() { return; }
        let line = self.lines[row].clone();
        let col = self.lines[row].len();
        let insert = format!("\n{}", line);
        self.insert_text(row, col, &insert);
        self.break_coalesce();
    }

    // ── Undo / Redo ────────────────────────────────

    pub fn undo(&mut self) -> Option<(usize, usize)> {
        let op = self.undo_stack.pop()?;
        let cursor = self.apply_inverse(&op);
        self.redo_stack.push(op);
        self.break_coalesce();
        Some(cursor)
    }

    pub fn redo(&mut self) -> Option<(usize, usize)> {
        let op = self.redo_stack.pop()?;
        let cursor = self.apply_forward(&op);
        self.undo_stack.push(op);
        self.break_coalesce();
        Some(cursor)
    }

    fn break_coalesce(&mut self) {
        self.coalesce_row = usize::MAX;
        self.coalesce_col = usize::MAX;
    }

    // ── Internal apply ─────────────────────────────

    fn apply_insert(&mut self, row: usize, col: usize, text: &str) {
        if !text.contains('\n') {
            let line = &mut self.lines[row];
            while line.len() < col { line.push(' '); }
            line.insert_str(col, text);
        } else {
            let parts: Vec<&str> = text.split('\n').collect();
            let rest = self.lines[row][col..].to_string();
            self.lines[row].truncate(col);
            self.lines[row].push_str(parts[0]);
            for (i, part) in parts[1..].iter().enumerate() {
                let mut new_line = part.to_string();
                if i == parts.len() - 2 {
                    new_line.push_str(&rest);
                }
                self.lines.insert(row + 1 + i, new_line);
            }
        }
    }

    fn apply_delete(&mut self, sr: usize, sc: usize, er: usize, ec: usize) {
        if sr == er {
            self.lines[sr].drain(sc..ec);
        } else {
            let end_rest = self.lines[er][ec..].to_string();
            self.lines[sr].truncate(sc);
            self.lines[sr].push_str(&end_rest);
            if er > sr {
                self.lines.drain((sr + 1)..=er);
            }
        }
    }

    fn extract_range(&self, sr: usize, sc: usize, er: usize, ec: usize) -> String {
        if sr == er {
            return self.lines[sr][sc..ec].to_string();
        }
        let mut result = self.lines[sr][sc..].to_string();
        result.push('\n');
        for row in (sr + 1)..er {
            result.push_str(&self.lines[row]);
            result.push('\n');
        }
        result.push_str(&self.lines[er][..ec]);
        result
    }

    fn apply_inverse(&mut self, op: &EditOp) -> (usize, usize) {
        match op {
            EditOp::Insert { row, col, text } => {
                // Inverse of insert = delete the inserted text
                let newlines = text.matches('\n').count();
                let end_row = row + newlines;
                let end_col = if newlines > 0 {
                    text.rsplit('\n').next().map(|s| s.len()).unwrap_or(0)
                } else {
                    col + text.len()
                };
                self.apply_delete(*row, *col, end_row, end_col);
                (*row, *col)
            }
            EditOp::Delete { row, col, text } => {
                // Inverse of delete = re-insert the deleted text
                self.apply_insert(*row, *col, text);
                let newlines = text.matches('\n').count();
                let end_col = if newlines > 0 {
                    text.rsplit('\n').next().map(|s| s.len()).unwrap_or(0)
                } else {
                    col + text.len()
                };
                (*row + newlines, end_col)
            }
            EditOp::Compound(ops) => {
                let mut cursor = (0, 0);
                for op in ops.iter().rev() {
                    cursor = self.apply_inverse(op);
                }
                cursor
            }
        }
    }

    fn apply_forward(&mut self, op: &EditOp) -> (usize, usize) {
        match op {
            EditOp::Insert { row, col, text } => {
                self.apply_insert(*row, *col, text);
                let newlines = text.matches('\n').count();
                let end_col = if newlines > 0 {
                    text.rsplit('\n').next().map(|s| s.len()).unwrap_or(0)
                } else {
                    col + text.len()
                };
                (*row + newlines, end_col)
            }
            EditOp::Delete { row, col, text } => {
                let newlines = text.matches('\n').count();
                let end_row = row + newlines;
                let end_col = if newlines > 0 {
                    text.rsplit('\n').next().map(|s| s.len()).unwrap_or(0)
                } else {
                    col + text.len()
                };
                self.apply_delete(*row, *col, end_row, end_col);
                (*row, *col)
            }
            EditOp::Compound(ops) => {
                let mut cursor = (0, 0);
                for op in ops {
                    cursor = self.apply_forward(op);
                }
                cursor
            }
        }
    }
}
