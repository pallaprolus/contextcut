use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct PackedFile {
    pub rel_path: PathBuf,
    pub content: String,
}

/// Render the packed files as one Markdown document: a file-tree header
/// followed by a language-tagged fenced block per file.
pub fn render(root: &Path, files: &[PackedFile]) -> String {
    let mut out = String::new();
    let root_name = root
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| root.display().to_string());

    out.push_str(&format!("# {root_name}\n\n## File tree\n\n```\n"));
    out.push_str(&tree(files));
    out.push_str("```\n");

    for file in files {
        let lang = language_tag(&file.rel_path);
        let fence = fence_for(&file.content);
        out.push_str(&format!(
            "\n## {}\n\n{fence}{lang}\n{}\n{fence}\n",
            file.rel_path.display(),
            file.content.trim_end_matches('\n')
        ));
    }
    out
}

/// A fence longer than any backtick run in the content, minimum ```.
fn fence_for(content: &str) -> String {
    let longest_run = content
        .lines()
        .map(|l| l.chars().take_while(|c| *c == '`').count())
        .max()
        .unwrap_or(0);
    "`".repeat(longest_run.max(2) + 1)
}

/// Nested dirs-first tree rendering of the packed paths.
fn tree(files: &[PackedFile]) -> String {
    #[derive(Default)]
    struct Node {
        dirs: BTreeMap<String, Node>,
        files: Vec<String>,
    }

    let mut root = Node::default();
    for file in files {
        let mut node = &mut root;
        let parts: Vec<_> = file.rel_path.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        for part in &parts[..parts.len() - 1] {
            node = node.dirs.entry(part.clone()).or_default();
        }
        node.files.push(parts.last().cloned().unwrap_or_default());
    }

    fn write_node(node: &Node, depth: usize, out: &mut String) {
        let pad = "  ".repeat(depth);
        for (name, child) in &node.dirs {
            out.push_str(&format!("{pad}{name}/\n"));
            write_node(child, depth + 1, out);
        }
        let mut files = node.files.clone();
        files.sort();
        for name in files {
            out.push_str(&format!("{pad}{name}\n"));
        }
    }

    let mut out = String::new();
    write_node(&root, 0, &mut out);
    out
}

/// Map a path to a Markdown fence language tag; empty when unknown.
fn language_tag(path: &Path) -> &'static str {
    let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    match name.as_ref() {
        "Dockerfile" => return "dockerfile",
        "Makefile" => return "makefile",
        _ => {}
    }
    match path.extension().map(|e| e.to_string_lossy()).as_deref() {
        Some("py") => "python",
        Some("rs") => "rust",
        Some("js" | "mjs" | "cjs") => "javascript",
        Some("ts" | "mts") => "typescript",
        Some("tsx") => "tsx",
        Some("jsx") => "jsx",
        Some("go") => "go",
        Some("c" | "h") => "c",
        Some("cpp" | "cc" | "hpp") => "cpp",
        Some("java") => "java",
        Some("rb") => "ruby",
        Some("sh" | "bash" | "zsh") => "bash",
        Some("yaml" | "yml") => "yaml",
        Some("toml") => "toml",
        Some("json") => "json",
        Some("md") => "markdown",
        Some("html") => "html",
        Some("css") => "css",
        Some("sql") => "sql",
        Some("xml") => "xml",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packed(path: &str, content: &str) -> PackedFile {
        PackedFile {
            rel_path: PathBuf::from(path),
            content: content.to_string(),
        }
    }

    #[test]
    fn language_map() {
        assert_eq!(language_tag(Path::new("a.py")), "python");
        assert_eq!(language_tag(Path::new("a.rs")), "rust");
        assert_eq!(language_tag(Path::new("a.tsx")), "tsx");
        assert_eq!(language_tag(Path::new("Dockerfile")), "dockerfile");
        assert_eq!(language_tag(Path::new("a.weird")), "");
    }

    #[test]
    fn fence_grows_past_content_backticks() {
        assert_eq!(fence_for("plain"), "```");
        assert_eq!(fence_for("has ```python inside"), "```"); // inline, not line-leading run? no: take_while from line start
        let block = "```\ncode\n```";
        assert_eq!(fence_for(block), "````");
    }

    #[test]
    fn tree_renders_dirs_first_sorted() {
        let files = vec![
            packed("z.py", ""),
            packed("src/main.py", ""),
            packed("src/util/helpers.py", ""),
            packed("a.md", ""),
        ];
        let got = tree(&files);
        let expected = "src/\n  util/\n    helpers.py\n  main.py\na.md\nz.py\n";
        assert_eq!(got, expected);
    }

    #[test]
    fn snapshot_mini_render() {
        let files = vec![
            packed("src/main.py", "def main():\n    pass\n"),
            packed("README.md", "# Demo\n"),
        ];
        let markdown = render(Path::new("demo"), &files);
        insta::assert_snapshot!(markdown);
    }
}
