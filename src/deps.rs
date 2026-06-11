use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// Import graph over the packed file set. Edges only exist between files
/// that survived pruning, so every false-positive specifier that doesn't
/// resolve to a real repo file is dropped — resolution is the FP filter
/// for line-based extraction.
pub struct Graph {
    pub forward: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
    pub reverse: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
}

impl Graph {
    /// Build the graph from (relative path, content) pairs.
    pub fn build(files: &[(PathBuf, String)]) -> Self {
        let file_set: HashSet<PathBuf> = files.iter().map(|(p, _)| p.clone()).collect();
        let go_module = go_module_path(files);

        let mut forward: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
        let mut reverse: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();

        for (path, content) in files {
            let targets = match extension(path) {
                "py" => python_imports(path, content, &file_set),
                "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => js_imports(path, content, &file_set),
                "rs" => rust_imports(path, content, &file_set),
                "go" => go_imports(content, go_module.as_deref(), &file_set),
                _ => Vec::new(),
            };
            for target in targets {
                if target != *path {
                    forward
                        .entry(path.clone())
                        .or_default()
                        .insert(target.clone());
                    reverse.entry(target).or_default().insert(path.clone());
                }
            }
        }
        Self { forward, reverse }
    }

    /// Files reachable from `seeds` within `depth` hops, following both
    /// imports (what a seed needs) and importers (what needs the seed).
    pub fn related(&self, seeds: &[PathBuf], depth: usize) -> BTreeSet<PathBuf> {
        let mut seen: BTreeSet<PathBuf> = seeds.iter().cloned().collect();
        let mut queue: VecDeque<(PathBuf, usize)> = seeds.iter().map(|s| (s.clone(), 0)).collect();

        while let Some((file, d)) = queue.pop_front() {
            if d >= depth {
                continue;
            }
            let neighbors = self
                .forward
                .get(&file)
                .into_iter()
                .chain(self.reverse.get(&file))
                .flatten();
            for n in neighbors {
                if seen.insert(n.clone()) {
                    queue.push_back((n.clone(), d + 1));
                }
            }
        }
        seen
    }

    /// Markdown "Dependency map" body: each file with edges, listing imports
    /// (→) and importers (←) restricted to the packed set.
    pub fn map_section(&self, packed: &[PathBuf]) -> String {
        let packed_set: HashSet<&PathBuf> = packed.iter().collect();
        let mut out = String::new();
        for file in packed {
            let deps: Vec<_> = self
                .forward
                .get(file)
                .map(|s| s.iter().filter(|t| packed_set.contains(t)).collect())
                .unwrap_or_default();
            let users: Vec<_> = self
                .reverse
                .get(file)
                .map(|s| s.iter().filter(|t| packed_set.contains(t)).collect())
                .unwrap_or_default();
            if deps.is_empty() && users.is_empty() {
                continue;
            }
            out.push_str(&format!("{}\n", file.display()));
            for d in deps {
                out.push_str(&format!("  → {}\n", d.display()));
            }
            for u in users {
                out.push_str(&format!("  ← {}\n", u.display()));
            }
        }
        out
    }
}

fn extension(path: &Path) -> &str {
    path.extension().and_then(|e| e.to_str()).unwrap_or("")
}

/// First existing candidate, or None — this gate is what keeps line-based
/// extraction honest.
fn first_existing(candidates: Vec<PathBuf>, files: &HashSet<PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|c| files.contains(c))
}

// ---------------- Python ----------------

fn python_imports(source: &Path, content: &str, files: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let source_dir = source.parent().unwrap_or(Path::new(""));
    let mut out = Vec::new();

    for line in content.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("import ") {
            // "import a.b, c.d as e"
            for part in rest.split(',') {
                let module = part.split_whitespace().next().unwrap_or("");
                out.extend(resolve_py_module(module, 0, source_dir, files, &[]));
            }
        } else if let Some(rest) = line.strip_prefix("from ") {
            // "from a.b import c, d" / "from . import x" / "from ..pkg import y"
            let Some((module_part, imported)) = rest.split_once(" import ") else {
                continue;
            };
            let module_part = module_part.trim();
            let level = module_part.chars().take_while(|c| *c == '.').count();
            let module = &module_part[level..];
            let names: Vec<&str> = imported
                .split(',')
                .map(|n| n.split_whitespace().next().unwrap_or(""))
                .collect();
            out.extend(resolve_py_module(module, level, source_dir, files, &names));
        }
    }
    out
}

/// Resolve a dotted module (optionally relative, `level` leading dots) to a
/// repo file. Tries root-relative and source-dir-relative bases; for
/// `from X import name`, also tries X/name.py (name may be a submodule).
fn resolve_py_module(
    module: &str,
    level: usize,
    source_dir: &Path,
    files: &HashSet<PathBuf>,
    imported_names: &[&str],
) -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = Vec::new();
    if level > 0 {
        // Relative: one dot = source dir, each extra dot goes up one.
        let mut base = source_dir.to_path_buf();
        for _ in 1..level {
            base = base.parent().map(Path::to_path_buf).unwrap_or_default();
        }
        bases.push(base);
    } else {
        bases.push(PathBuf::new()); // repo root
        bases.push(source_dir.to_path_buf()); // sibling modules
    }

    let module_path: PathBuf = module.split('.').filter(|s| !s.is_empty()).collect();
    let mut found = Vec::new();
    for base in &bases {
        let stem = base.join(&module_path);
        // The module itself: a/b.py or a/b/__init__.py …
        if let Some(hit) = first_existing(
            vec![stem.with_extension("py"), stem.join("__init__.py")],
            files,
        ) {
            found.push(hit);
        }
        // … plus each imported name that is itself a submodule: a/b/c.py.
        for name in imported_names {
            if !name.is_empty() {
                let candidate = stem.join(name).with_extension("py");
                if files.contains(&candidate) {
                    found.push(candidate);
                }
            }
        }
    }
    found
}

// ---------------- JS / TS ----------------

fn js_imports(source: &Path, content: &str, files: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let source_dir = source.parent().unwrap_or(Path::new(""));
    let mut out = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim_start();
        let mut specs: Vec<String> = Vec::new();

        if trimmed.starts_with("import ") || trimmed.starts_with("export ") {
            if let Some(spec) = quoted_after(trimmed, " from ") {
                specs.push(spec);
            } else if let Some(spec) = quoted_after(trimmed, "import ") {
                specs.push(spec); // bare `import './side-effect'`
            }
        }
        // require('...') anywhere on the line; resolution gates false hits.
        let mut rest = trimmed;
        while let Some(idx) = rest.find("require(") {
            rest = &rest[idx + "require(".len()..];
            if let Some(spec) = leading_quoted(rest) {
                specs.push(spec);
            }
        }

        for spec in specs {
            // Only relative specifiers map to repo files.
            if !spec.starts_with('.') {
                continue;
            }
            let stem = normalize(&source_dir.join(&spec));
            let exts = ["ts", "tsx", "js", "jsx", "mjs", "cjs"];
            let mut candidates: Vec<PathBuf> = exts
                .iter()
                .map(|e| stem.with_extension(e))
                .chain(exts.iter().map(|e| stem.join("index").with_extension(e)))
                .collect();
            if stem.extension().is_some() {
                candidates.insert(0, stem.clone());
            }
            if let Some(hit) = first_existing(candidates, files) {
                out.push(hit);
            }
        }
    }
    out
}

/// The quoted string immediately following `marker`, if present.
fn quoted_after(line: &str, marker: &str) -> Option<String> {
    let idx = line.find(marker)?;
    leading_quoted(&line[idx + marker.len()..])
}

/// A leading 'spec' or "spec" (after optional whitespace).
fn leading_quoted(s: &str) -> Option<String> {
    let s = s.trim_start();
    let quote = s.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let rest = &s[1..];
    rest.find(quote).map(|end| rest[..end].to_string())
}

/// Resolve ".." / "." segments without touching the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp.as_os_str().to_str() {
            Some("..") => {
                out.pop();
            }
            Some(".") => {}
            _ => out.push(comp),
        }
    }
    out
}

// ---------------- Rust ----------------

fn rust_imports(source: &Path, content: &str, files: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let source_dir = source.parent().unwrap_or(Path::new(""));
    let mut out = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim_start().trim_start_matches("pub ");
        if let Some(rest) = trimmed.strip_prefix("mod ") {
            let name = rest.trim_end().trim_end_matches(';');
            if name.chars().all(|c| c.is_alphanumeric() || c == '_') && !name.is_empty() {
                let candidates = vec![
                    source_dir.join(format!("{name}.rs")),
                    source_dir.join(name).join("mod.rs"),
                ];
                if let Some(hit) = first_existing(candidates, files) {
                    out.push(hit);
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("use crate::") {
            let path_part: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
                .collect();
            let segs: Vec<&str> = path_part.split("::").filter(|s| !s.is_empty()).collect();
            // Try each prefix of the use-path as a module file under src/.
            for k in (1..=segs.len()).rev() {
                let joined: PathBuf = segs[..k].iter().collect();
                let candidates = vec![
                    Path::new("src").join(&joined).with_extension("rs"),
                    Path::new("src").join(&joined).join("mod.rs"),
                ];
                if let Some(hit) = first_existing(candidates, files) {
                    out.push(hit);
                    break;
                }
            }
        }
    }
    out
}

// ---------------- Go ----------------

/// The `module` line from go.mod, if the repo has one.
fn go_module_path(files: &[(PathBuf, String)]) -> Option<String> {
    files
        .iter()
        .find(|(p, _)| p.file_name().is_some_and(|n| n == "go.mod"))
        .and_then(|(_, content)| {
            content
                .lines()
                .find_map(|l| l.trim().strip_prefix("module "))
                .map(|m| m.trim().to_string())
        })
}

fn go_imports(content: &str, module: Option<&str>, files: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let Some(module) = module else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut in_block = false;

    for line in content.lines() {
        let trimmed = line.trim();
        let spec = if in_block {
            if trimmed == ")" {
                in_block = false;
                continue;
            }
            leading_quoted(trimmed.split_whitespace().last().unwrap_or(""))
                .or_else(|| leading_quoted(trimmed))
        } else if trimmed == "import (" {
            in_block = true;
            continue;
        } else if let Some(rest) = trimmed.strip_prefix("import ") {
            leading_quoted(rest.split_whitespace().last().unwrap_or(rest))
        } else {
            None
        };

        let Some(spec) = spec else { continue };
        let Some(rel_dir) = spec.strip_prefix(module).map(|s| s.trim_start_matches('/')) else {
            continue;
        };
        // A Go import targets a package directory — edge to its .go files.
        let dir = Path::new(rel_dir);
        out.extend(
            files
                .iter()
                .filter(|f| f.parent() == Some(dir) && extension(f) == "go")
                .cloned(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(entries: &[(&str, &str)]) -> Graph {
        let files: Vec<(PathBuf, String)> = entries
            .iter()
            .map(|(p, c)| (PathBuf::from(p), c.to_string()))
            .collect();
        Graph::build(&files)
    }

    fn edge(g: &Graph, from: &str, to: &str) -> bool {
        g.forward
            .get(Path::new(from))
            .is_some_and(|s| s.contains(Path::new(to)))
    }

    #[test]
    fn python_root_and_relative_imports() {
        let g = graph(&[
            ("src/a.py", "from src.b import helper\n"),
            ("src/b.py", "import src.c\nfrom . import d\n"),
            ("src/c.py", ""),
            ("src/d.py", ""),
        ]);
        assert!(edge(&g, "src/a.py", "src/b.py"));
        assert!(edge(&g, "src/b.py", "src/c.py"));
        assert!(edge(&g, "src/b.py", "src/d.py"));
    }

    #[test]
    fn python_from_import_submodule_and_package_init() {
        let g = graph(&[
            ("pkg/__init__.py", ""),
            ("pkg/mod.py", ""),
            ("main.py", "import pkg\nfrom pkg import mod\n"),
        ]);
        assert!(edge(&g, "main.py", "pkg/__init__.py"));
        assert!(edge(&g, "main.py", "pkg/mod.py"));
    }

    #[test]
    fn python_traps_string_and_comment() {
        let g = graph(&[
            ("a.py", "s = \"import b\"\n# import b\nx = 1\n"),
            ("b.py", ""),
        ]);
        assert!(!g.forward.contains_key(Path::new("a.py")));
    }

    #[test]
    fn js_import_require_and_index_resolution() {
        let g = graph(&[
            (
                "app.js",
                "import { x } from './lib/util';\nconst y = require('./store');\n",
            ),
            ("lib/util.js", "import '../app.css';\n"),
            ("store/index.ts", ""),
            ("app.css", ""),
        ]);
        assert!(edge(&g, "app.js", "lib/util.js"));
        assert!(edge(&g, "app.js", "store/index.ts"));
        assert!(edge(&g, "lib/util.js", "app.css"));
    }

    #[test]
    fn js_traps_bare_packages_and_unresolved() {
        let g = graph(&[
            (
                "a.js",
                "import React from 'react';\nconst t = `require('./ghost')`;\n",
            ),
            ("b.js", ""),
        ]);
        // 'react' is a bare package; './ghost' doesn't resolve to a file.
        assert!(!g.forward.contains_key(Path::new("a.js")));
    }

    #[test]
    fn rust_mod_and_use_crate() {
        let g = graph(&[
            ("src/main.rs", "mod walker;\nuse crate::pruner::decide;\n"),
            ("src/walker.rs", ""),
            ("src/pruner.rs", ""),
        ]);
        assert!(edge(&g, "src/main.rs", "src/walker.rs"));
        assert!(edge(&g, "src/main.rs", "src/pruner.rs"));
    }

    #[test]
    fn go_module_imports() {
        let g = graph(&[
            ("go.mod", "module example.com/myapp\n\ngo 1.22\n"),
            (
                "main.go",
                "import (\n\t\"fmt\"\n\t\"example.com/myapp/util\"\n)\n",
            ),
            ("util/strings.go", ""),
        ]);
        assert!(edge(&g, "main.go", "util/strings.go"));
    }

    #[test]
    fn related_bfs_respects_depth() {
        let g = graph(&[
            ("a.py", "import b\n"),
            ("b.py", "import c\n"),
            ("c.py", ""),
            ("d.py", ""),
        ]);
        let seeds = vec![PathBuf::from("a.py")];
        let d1 = g.related(&seeds, 1);
        assert!(d1.contains(Path::new("a.py")) && d1.contains(Path::new("b.py")));
        assert!(!d1.contains(Path::new("c.py")));

        let d2 = g.related(&seeds, 2);
        assert!(d2.contains(Path::new("c.py")));
        assert!(!d2.contains(Path::new("d.py")));
    }

    #[test]
    fn related_follows_reverse_edges() {
        let g = graph(&[("a.py", "import b\n"), ("b.py", "")]);
        // Seeding on the *imported* file must pull in its importer.
        let related = g.related(&[PathBuf::from("b.py")], 1);
        assert!(related.contains(Path::new("a.py")));
    }

    #[test]
    fn map_section_lists_both_directions() {
        let g = graph(&[("a.py", "import b\n"), ("b.py", "")]);
        let packed = vec![PathBuf::from("a.py"), PathBuf::from("b.py")];
        let map = g.map_section(&packed);
        assert!(map.contains("a.py\n  → b.py"));
        assert!(map.contains("b.py\n  ← a.py"));
    }
}
