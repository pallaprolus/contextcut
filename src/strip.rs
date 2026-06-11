use std::path::Path;

/// Strip full-line comments for languages where that's safe to do
/// line-by-line. Inline trailing comments are deliberately left alone —
/// distinguishing them from `#`/`//` inside string literals needs a real
/// parser (tree-sitter, planned for v0.2). Unknown extensions pass through.
pub fn strip_comments(path: &Path, content: &str) -> String {
    let Some(prefix) = comment_prefix(path) else {
        return content.to_string();
    };

    let mut out = String::with_capacity(content.len());
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        // Keep shebangs: "#!" on line 1 is not a comment.
        if i == 0 && trimmed.starts_with("#!") {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if trimmed.starts_with(prefix) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The full-line comment prefix for the file's language, if supported.
fn comment_prefix(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_string_lossy();
    match ext.as_ref() {
        "py" | "sh" | "bash" | "zsh" | "yaml" | "yml" | "toml" | "rb" => Some("#"),
        "rs" | "js" | "mjs" | "cjs" | "ts" | "mts" | "tsx" | "jsx" | "go" | "c" | "h" | "cpp"
        | "cc" | "hpp" | "java" => Some("//"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_python_comment_lines() {
        let src = "# top comment\nx = 1\n    # indented comment\ny = 2\n";
        let got = strip_comments(Path::new("a.py"), src);
        assert_eq!(got, "x = 1\ny = 2\n");
    }

    #[test]
    fn keeps_hash_inside_string() {
        let src = "x = \"# not a comment\"\n";
        let got = strip_comments(Path::new("a.py"), src);
        assert_eq!(got, src);
    }

    #[test]
    fn keeps_shebang() {
        let src = "#!/usr/bin/env python\n# real comment\nx = 1\n";
        let got = strip_comments(Path::new("run.py"), src);
        assert_eq!(got, "#!/usr/bin/env python\nx = 1\n");
    }

    #[test]
    fn keeps_url_in_string_for_slash_languages() {
        let src = "let url = \"https://example.com\";\n// gone\n";
        let got = strip_comments(Path::new("a.rs"), src);
        assert_eq!(got, "let url = \"https://example.com\";\n");
    }

    #[test]
    fn unknown_extension_passthrough() {
        let src = "# whatever\n";
        assert_eq!(strip_comments(Path::new("a.weird"), src), src);
    }

    #[test]
    fn idempotent() {
        let src = "# c\nx = 1\n";
        let once = strip_comments(Path::new("a.py"), src);
        let twice = strip_comments(Path::new("a.py"), &once);
        assert_eq!(once, twice);
    }
}
