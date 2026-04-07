/// Hand-written syntax tokenizer — no regex dependency.

use crate::theme;
use gpui::Rgba;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    Keyword,
    String,
    Comment,
    Number,
    Type,
    Function,
    Operator,
    Punctuation,
    Plain,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub kind: TokenKind,
}

pub fn token_color(kind: TokenKind) -> Rgba {
    // VS Code Dark+ inspired color mapping
    match kind {
        TokenKind::Keyword => theme::lavender(),    // purple/pink — control flow keywords
        TokenKind::String => theme::peach(),        // orange/salmon — string literals
        TokenKind::Comment => theme::green(),       // green — comments
        TokenKind::Number => theme::green(),        // light green — number literals
        TokenKind::Type => theme::teal(),           // teal/cyan — types, classes
        TokenKind::Function => theme::yellow(),     // yellow — function names
        TokenKind::Operator => theme::text(),       // light gray — operators
        TokenKind::Punctuation => theme::subtext(), // gray — brackets, semicolons
        TokenKind::Plain => theme::text(),          // default text
    }
}

pub struct LangDef {
    pub keywords: &'static [&'static str],
    pub line_comment: &'static str,
    pub block_comment: Option<(&'static str, &'static str)>,
    pub string_delims: &'static [char],
}

pub fn lang_for_ext(ext: &str) -> Option<&'static LangDef> {
    match ext {
        "rs" => Some(&RUST),
        "js" | "jsx" | "mjs" | "cjs" => Some(&JAVASCRIPT),
        "ts" | "tsx" => Some(&TYPESCRIPT),
        "py" => Some(&PYTHON),
        "go" => Some(&GO),
        "c" | "h" => Some(&C_LANG),
        "cpp" | "cc" | "cxx" | "hpp" => Some(&CPP),
        "java" => Some(&JAVA),
        "swift" => Some(&SWIFT),
        "toml" => Some(&TOML),
        "json" => Some(&JSON),
        "yaml" | "yml" => Some(&YAML),
        "sh" | "bash" | "zsh" => Some(&SHELL),
        "css" => Some(&CSS),
        "html" | "htm" => Some(&HTML),
        "md" | "markdown" => Some(&MARKDOWN),
        _ => None,
    }
}

pub fn lang_name(ext: &str) -> &'static str {
    match ext {
        "rs" => "Rust",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "ts" | "tsx" => "TypeScript",
        "py" => "Python",
        "go" => "Go",
        "c" | "h" => "C",
        "cpp" | "cc" | "cxx" | "hpp" => "C++",
        "java" => "Java",
        "swift" => "Swift",
        "toml" => "TOML",
        "json" => "JSON",
        "yaml" | "yml" => "YAML",
        "sh" | "bash" | "zsh" => "Shell",
        "css" => "CSS",
        "html" | "htm" => "HTML",
        "md" | "markdown" => "Markdown",
        _ => "Plain Text",
    }
}

// ── Tokenize a single line ─────────────────────────

pub fn tokenize_line(line: &str, lang: &LangDef, in_block_comment: bool) -> (Vec<Token>, bool) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens: Vec<Token> = Vec::new();
    let mut i = 0;
    let mut in_bc = in_block_comment;

    while i < len {
        // Block comment continuation
        if in_bc {
            let start = i;
            if let Some((_, close)) = lang.block_comment {
                if let Some(pos) = line[i..].find(close) {
                    i += pos + close.len();
                    in_bc = false;
                } else {
                    i = len;
                }
            } else {
                i = len;
            }
            tokens.push(Token { start, end: i, kind: TokenKind::Comment });
            continue;
        }

        // Skip whitespace
        if bytes[i].is_ascii_whitespace() {
            let start = i;
            while i < len && bytes[i].is_ascii_whitespace() { i += 1; }
            tokens.push(Token { start, end: i, kind: TokenKind::Plain });
            continue;
        }

        // Line comment
        if !lang.line_comment.is_empty() && line[i..].starts_with(lang.line_comment) {
            tokens.push(Token { start: i, end: len, kind: TokenKind::Comment });
            i = len;
            continue;
        }

        // Block comment start
        if let Some((open, _close)) = lang.block_comment {
            if line[i..].starts_with(open) {
                let start = i;
                i += open.len();
                if let Some(close) = lang.block_comment.map(|(_, c)| c) {
                    if let Some(pos) = line[i..].find(close) {
                        i += pos + close.len();
                    } else {
                        in_bc = true;
                        i = len;
                    }
                }
                tokens.push(Token { start, end: i, kind: TokenKind::Comment });
                continue;
            }
        }

        // Strings
        if lang.string_delims.contains(&(bytes[i] as char)) {
            let delim = bytes[i] as char;
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' { i += 2; continue; }
                if bytes[i] as char == delim { i += 1; break; }
                i += 1;
            }
            tokens.push(Token { start, end: i, kind: TokenKind::String });
            continue;
        }

        // Numbers
        if bytes[i].is_ascii_digit() || (bytes[i] == b'.' && i + 1 < len && bytes[i+1].is_ascii_digit()) {
            let start = i;
            // Handle hex: 0x...
            if bytes[i] == b'0' && i + 1 < len && (bytes[i+1] == b'x' || bytes[i+1] == b'X') {
                i += 2;
                while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') { i += 1; }
            } else {
                while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'_'
                    || bytes[i] == b'e' || bytes[i] == b'E') { i += 1; }
            }
            // Numeric suffix (e.g., f32, u64, usize)
            if i < len && (bytes[i] == b'f' || bytes[i] == b'u' || bytes[i] == b'i') {
                while i < len && bytes[i].is_ascii_alphanumeric() { i += 1; }
            }
            tokens.push(Token { start, end: i, kind: TokenKind::Number });
            continue;
        }

        // Identifiers / keywords / types / functions
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1; }
            let word = &line[start..i];

            // Macro calls in Rust (word!)
            let is_macro = i < len && bytes[i] == b'!';

            let kind = if lang.keywords.contains(&word) {
                TokenKind::Keyword
            } else if is_macro {
                TokenKind::Function
            } else if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                TokenKind::Type
            } else {
                // Check if followed by (  → function call
                let mut j = i;
                while j < len && bytes[j].is_ascii_whitespace() { j += 1; }
                if j < len && bytes[j] == b'(' {
                    TokenKind::Function
                } else {
                    TokenKind::Plain
                }
            };
            tokens.push(Token { start, end: i, kind });
            continue;
        }

        // Operators and punctuation
        let start = i;
        i += 1;
        let b = bytes[start];
        let kind = match b {
            b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'!' | b'<' | b'>' | b'&' | b'|' | b'^' | b'~' => {
                // Consume multi-char operators
                while i < len && matches!(bytes[i], b'=' | b'>' | b'<' | b'&' | b'|') {
                    i += 1;
                    if i - start > 3 { break; }
                }
                TokenKind::Operator
            }
            b'{' | b'}' | b'(' | b')' | b'[' | b']' | b';' | b',' | b'.' | b':' | b'?' | b'@' | b'#' => {
                TokenKind::Punctuation
            }
            _ => TokenKind::Plain,
        };
        tokens.push(Token { start, end: i, kind });
    }

    (tokens, in_bc)
}

// ── Language definitions ───────────────────────────

static RUST: LangDef = LangDef {
    keywords: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
        "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
        "trait", "true", "type", "unsafe", "use", "where", "while", "yield",
    ],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"'],
};

static JAVASCRIPT: LangDef = LangDef {
    keywords: &[
        "async", "await", "break", "case", "catch", "class", "const", "continue", "debugger",
        "default", "delete", "do", "else", "export", "extends", "false", "finally", "for",
        "from", "function", "if", "import", "in", "instanceof", "let", "new", "null", "of",
        "return", "super", "switch", "this", "throw", "true", "try", "typeof", "undefined",
        "var", "void", "while", "with", "yield",
    ],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\'', '`'],
};

static TYPESCRIPT: LangDef = LangDef {
    keywords: &[
        "abstract", "as", "async", "await", "break", "case", "catch", "class", "const",
        "continue", "debugger", "declare", "default", "delete", "do", "else", "enum", "export",
        "extends", "false", "finally", "for", "from", "function", "if", "implements", "import",
        "in", "instanceof", "interface", "is", "keyof", "let", "module", "namespace", "new",
        "null", "of", "private", "protected", "public", "readonly", "return", "static", "super",
        "switch", "this", "throw", "true", "try", "type", "typeof", "undefined", "var", "void",
        "while", "with", "yield",
    ],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\'', '`'],
};

static PYTHON: LangDef = LangDef {
    keywords: &[
        "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
        "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
        "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
        "return", "try", "while", "with", "yield",
    ],
    line_comment: "#",
    block_comment: None,
    string_delims: &['"', '\''],
};

static GO: LangDef = LangDef {
    keywords: &[
        "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough",
        "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range",
        "return", "select", "struct", "switch", "type", "var", "true", "false", "nil",
    ],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\'', '`'],
};

static C_LANG: LangDef = LangDef {
    keywords: &[
        "auto", "break", "case", "char", "const", "continue", "default", "do", "double",
        "else", "enum", "extern", "float", "for", "goto", "if", "int", "long", "register",
        "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
        "union", "unsigned", "void", "volatile", "while", "NULL", "true", "false",
    ],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\''],
};

static CPP: LangDef = LangDef {
    keywords: &[
        "alignas", "alignof", "and", "auto", "bool", "break", "case", "catch", "char", "class",
        "const", "constexpr", "continue", "decltype", "default", "delete", "do", "double",
        "else", "enum", "explicit", "export", "extern", "false", "float", "for", "friend",
        "goto", "if", "inline", "int", "long", "mutable", "namespace", "new", "noexcept",
        "nullptr", "operator", "or", "private", "protected", "public", "register", "return",
        "short", "signed", "sizeof", "static", "struct", "switch", "template", "this", "throw",
        "true", "try", "typedef", "typeid", "typename", "union", "unsigned", "using", "virtual",
        "void", "volatile", "while",
    ],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\''],
};

static JAVA: LangDef = LangDef {
    keywords: &[
        "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class",
        "const", "continue", "default", "do", "double", "else", "enum", "extends", "false",
        "final", "finally", "float", "for", "goto", "if", "implements", "import", "instanceof",
        "int", "interface", "long", "native", "new", "null", "package", "private", "protected",
        "public", "return", "short", "static", "strictfp", "super", "switch", "synchronized",
        "this", "throw", "throws", "transient", "true", "try", "void", "volatile", "while",
    ],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\''],
};

static SWIFT: LangDef = LangDef {
    keywords: &[
        "associatedtype", "break", "case", "catch", "class", "continue", "default", "defer",
        "deinit", "do", "else", "enum", "extension", "fallthrough", "false", "fileprivate",
        "for", "func", "guard", "if", "import", "in", "init", "inout", "internal", "is", "let",
        "nil", "open", "operator", "private", "protocol", "public", "repeat", "return", "self",
        "Self", "static", "struct", "subscript", "super", "switch", "throw", "throws", "true",
        "try", "typealias", "var", "where", "while",
    ],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"'],
};

static TOML: LangDef = LangDef {
    keywords: &["true", "false"],
    line_comment: "#",
    block_comment: None,
    string_delims: &['"', '\''],
};

static JSON: LangDef = LangDef {
    keywords: &["true", "false", "null"],
    line_comment: "",
    block_comment: None,
    string_delims: &['"'],
};

static YAML: LangDef = LangDef {
    keywords: &["true", "false", "null", "yes", "no", "on", "off"],
    line_comment: "#",
    block_comment: None,
    string_delims: &['"', '\''],
};

static SHELL: LangDef = LangDef {
    keywords: &[
        "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
        "function", "in", "select", "until", "return", "exit", "break", "continue", "local",
        "export", "readonly", "declare", "set", "unset", "shift", "source", "true", "false",
    ],
    line_comment: "#",
    block_comment: None,
    string_delims: &['"', '\''],
};

static CSS: LangDef = LangDef {
    keywords: &[
        "important", "inherit", "initial", "unset", "none", "auto", "block", "flex", "grid",
        "inline", "relative", "absolute", "fixed", "sticky", "solid", "dashed", "dotted",
    ],
    line_comment: "",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\''],
};

static HTML: LangDef = LangDef {
    keywords: &[],
    line_comment: "",
    block_comment: Some(("<!--", "-->")),
    string_delims: &['"', '\''],
};

static MARKDOWN: LangDef = LangDef {
    keywords: &[],
    line_comment: "",
    block_comment: None,
    string_delims: &[],
};
