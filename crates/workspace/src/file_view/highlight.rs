/// Hand-written syntax tokenizer — no regex dependency.

use crate::theme;
use gpui::{rgb, Rgba};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    Keyword,              // control flow: if, else, return, for, while… → lavender #C586C0
    KeywordDeclaration,   // storage/declaration: const, let, fn, import, export… → blue #569CD6
    String,
    Comment,
    Number,
    Type,
    Function,
    Variable,             // parameter/variable names → light blue #9CDCFE
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

/// VS Code Dark Modern / Dark+ exact token colors
pub fn token_color(kind: TokenKind) -> Rgba {
    match kind {
        TokenKind::Keyword => theme::lavender(),              // #C586C0 — control flow: if, else, return…
        TokenKind::KeywordDeclaration => theme::blue(),       // #569CD6 — storage/declaration: const, fn, import…
        TokenKind::String => theme::peach(),                  // #CE9178 — string literals
        TokenKind::Comment => theme::green(),                 // #6A9955 — comments
        TokenKind::Number => rgb(0xb5cea8),                   // #B5CEA8 — number literals
        TokenKind::Type => theme::teal(),                     // #4EC9B0 — types, classes
        TokenKind::Function => theme::yellow(),               // #DCDCAA — function names
        TokenKind::Variable => rgb(0x9cdcfe),                 // #9CDCFE — variables, parameters
        TokenKind::Operator => theme::text(),                 // default — operators
        TokenKind::Punctuation => theme::text(),              // default — brackets, semicolons
        TokenKind::Plain => theme::text(),                    // default text
    }
}

pub struct LangDef {
    /// Control-flow keywords → lavender (#C586C0): if, else, return, for, while…
    pub keywords: &'static [&'static str],
    /// Storage/declaration keywords → blue (#569CD6): const, let, fn, import, class…
    pub keywords_decl: &'static [&'static str],
    /// Built-in type names (lowercase) → teal (#4EC9B0): string, number, boolean…
    pub builtin_types: &'static [&'static str],
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

            // Check if followed by (  → function call
            let followed_by_paren = {
                let mut j = i;
                while j < len && bytes[j].is_ascii_whitespace() { j += 1; }
                j < len && bytes[j] == b'('
            };

            // Check if preceded by < or </ → JSX/HTML tag name
            let is_tag_name = (start > 0 && bytes[start - 1] == b'<')
                || (start > 1 && bytes[start - 1] == b'/' && bytes[start - 2] == b'<');

            let kind = if lang.keywords_decl.contains(&word) {
                TokenKind::KeywordDeclaration
            } else if lang.keywords.contains(&word) {
                TokenKind::Keyword
            } else if lang.builtin_types.contains(&word) {
                TokenKind::Type
            } else if is_macro || followed_by_paren {
                TokenKind::Function
            } else if is_tag_name {
                TokenKind::Type
            } else {
                TokenKind::Variable
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
                // Arrow => is yellow in VS Code Dark+
                if &line[start..i] == "=>" {
                    TokenKind::Function
                } else {
                    TokenKind::Operator
                }
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
        "as", "await", "break", "continue", "else", "for", "if", "in", "loop",
        "match", "move", "return", "where", "while", "yield", "unsafe",
    ],
    keywords_decl: &[
        "async", "const", "crate", "dyn", "enum", "extern", "false", "fn", "impl", "let", "mod",
        "mut", "pub", "ref", "self", "Self", "static", "struct", "super", "trait", "true",
        "type", "use",
    ],
    builtin_types: &[
        "bool", "char", "str", "i8", "i16", "i32", "i64", "i128", "isize",
        "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64",
    ],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"'],
};

static JAVASCRIPT: LangDef = LangDef {
    keywords: &[
        "await", "break", "case", "catch", "continue", "default", "do", "else", "export",
        "finally", "for", "from", "if", "import", "return", "switch", "throw", "try",
        "while", "with", "yield",
    ],
    keywords_decl: &[
        "async", "class", "const", "debugger", "delete", "extends", "false", "function",
        "in", "instanceof", "let", "new", "null", "of", "super", "this", "true", "typeof",
        "undefined", "var", "void",
    ],
    builtin_types: &[],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\'', '`'],
};

static TYPESCRIPT: LangDef = LangDef {
    keywords: &[
        "await", "break", "case", "catch", "continue", "default", "do", "else", "export",
        "finally", "for", "from", "if", "import", "return", "switch", "throw", "try",
        "while", "with", "yield",
    ],
    keywords_decl: &[
        "abstract", "as", "async", "class", "const", "debugger", "declare", "delete", "enum",
        "extends", "false", "function", "implements", "in", "instanceof", "interface", "is",
        "keyof", "let", "module", "namespace", "new", "null", "of", "private", "protected",
        "public", "readonly", "static", "super", "this", "true", "type", "typeof",
        "undefined", "var", "void",
    ],
    builtin_types: &[
        "string", "number", "boolean", "any", "never", "object", "symbol", "bigint", "unknown",
    ],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\'', '`'],
};

static PYTHON: LangDef = LangDef {
    keywords: &[
        "and", "as", "assert", "await", "break", "continue", "del", "elif", "else",
        "except", "finally", "for", "from", "if", "import", "lambda", "not", "or", "pass",
        "raise", "return", "try", "while", "with", "yield",
    ],
    keywords_decl: &[
        "async", "False", "None", "True", "class", "def", "global", "in", "is", "nonlocal",
    ],
    builtin_types: &[
        "int", "float", "str", "bool", "list", "dict", "tuple", "set", "bytes",
    ],
    line_comment: "#",
    block_comment: None,
    string_delims: &['"', '\''],
};

static GO: LangDef = LangDef {
    keywords: &[
        "break", "case", "continue", "default", "defer", "else", "fallthrough", "for", "go",
        "goto", "if", "range", "return", "select", "switch",
    ],
    keywords_decl: &[
        "chan", "const", "false", "func", "import", "interface", "map", "nil", "package",
        "struct", "true", "type", "var",
    ],
    builtin_types: &[
        "bool", "byte", "complex64", "complex128", "error", "float32", "float64",
        "int", "int8", "int16", "int32", "int64", "rune", "string",
        "uint", "uint8", "uint16", "uint32", "uint64", "uintptr",
    ],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\'', '`'],
};

static C_LANG: LangDef = LangDef {
    keywords: &[
        "break", "case", "continue", "default", "do", "else", "for", "goto", "if", "return",
        "sizeof", "switch", "while",
    ],
    keywords_decl: &[
        "auto", "char", "const", "double", "enum", "extern", "false", "float", "int", "long",
        "NULL", "register", "short", "signed", "static", "struct", "true", "typedef", "union",
        "unsigned", "void", "volatile",
    ],
    builtin_types: &[],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\''],
};

static CPP: LangDef = LangDef {
    keywords: &[
        "and", "break", "case", "catch", "continue", "default", "delete", "do", "else", "for",
        "goto", "if", "new", "noexcept", "operator", "or", "return", "sizeof", "switch", "this",
        "throw", "try", "while",
    ],
    keywords_decl: &[
        "alignas", "alignof", "auto", "bool", "char", "class", "const", "constexpr", "decltype",
        "double", "enum", "explicit", "export", "extern", "false", "float", "friend", "inline",
        "int", "long", "mutable", "namespace", "nullptr", "private", "protected", "public",
        "register", "short", "signed", "static", "struct", "template", "true", "typedef",
        "typeid", "typename", "union", "unsigned", "using", "virtual", "void", "volatile",
    ],
    builtin_types: &[],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\''],
};

static JAVA: LangDef = LangDef {
    keywords: &[
        "assert", "break", "case", "catch", "continue", "default", "do", "else", "finally",
        "for", "goto", "if", "instanceof", "new", "return", "switch", "synchronized", "this",
        "throw", "throws", "try", "while",
    ],
    keywords_decl: &[
        "abstract", "boolean", "byte", "char", "class", "const", "double", "enum", "extends",
        "false", "final", "float", "implements", "import", "int", "interface", "long", "native",
        "null", "package", "private", "protected", "public", "short", "static", "strictfp",
        "super", "transient", "true", "void", "volatile",
    ],
    builtin_types: &[],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\''],
};

static SWIFT: LangDef = LangDef {
    keywords: &[
        "break", "case", "catch", "continue", "default", "defer", "do", "else", "fallthrough",
        "for", "guard", "if", "in", "is", "repeat", "return", "switch", "throw", "throws",
        "try", "where", "while",
    ],
    keywords_decl: &[
        "associatedtype", "class", "deinit", "enum", "extension", "false", "fileprivate",
        "func", "import", "init", "inout", "internal", "let", "nil", "open", "operator",
        "private", "protocol", "public", "self", "Self", "static", "struct", "subscript",
        "super", "true", "typealias", "var",
    ],
    builtin_types: &[],
    line_comment: "//",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"'],
};

static TOML: LangDef = LangDef {
    keywords: &[],
    keywords_decl: &["true", "false"],
    builtin_types: &[],
    line_comment: "#",
    block_comment: None,
    string_delims: &['"', '\''],
};

static JSON: LangDef = LangDef {
    keywords: &[],
    keywords_decl: &["true", "false", "null"],
    builtin_types: &[],
    line_comment: "",
    block_comment: None,
    string_delims: &['"'],
};

static YAML: LangDef = LangDef {
    keywords: &[],
    keywords_decl: &["true", "false", "null", "yes", "no", "on", "off"],
    builtin_types: &[],
    line_comment: "#",
    block_comment: None,
    string_delims: &['"', '\''],
};

static SHELL: LangDef = LangDef {
    keywords: &[
        "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
        "in", "select", "until", "return", "exit", "break", "continue",
    ],
    keywords_decl: &[
        "function", "local", "export", "readonly", "declare", "set", "unset", "shift", "source",
        "true", "false",
    ],
    builtin_types: &[],
    line_comment: "#",
    block_comment: None,
    string_delims: &['"', '\''],
};

static CSS: LangDef = LangDef {
    keywords: &[],
    keywords_decl: &[
        "important", "inherit", "initial", "unset", "none", "auto", "block", "flex", "grid",
        "inline", "relative", "absolute", "fixed", "sticky", "solid", "dashed", "dotted",
    ],
    builtin_types: &[],
    line_comment: "",
    block_comment: Some(("/*", "*/")),
    string_delims: &['"', '\''],
};

static HTML: LangDef = LangDef {
    keywords: &[],
    keywords_decl: &[],
    builtin_types: &[],
    line_comment: "",
    block_comment: Some(("<!--", "-->")),
    string_delims: &['"', '\''],
};

static MARKDOWN: LangDef = LangDef {
    keywords: &[],
    keywords_decl: &[],
    builtin_types: &[],
    line_comment: "",
    block_comment: None,
    string_delims: &[],
};
