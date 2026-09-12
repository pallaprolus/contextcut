use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::{
    deps::Graph,
    renderer::{self, PackedFile},
    review::Review,
    tokens,
};

pub struct Selection<'a> {
    pub root: &'a Path,
    pub graph: Option<&'a Graph>,
    pub map: bool,
    pub distances: BTreeMap<PathBuf, usize>,
    pub review: Option<&'a Review>,
    pub budget: Option<usize>,
}

impl Selection<'_> {
    fn render(&self, files: &[PackedFile], omitted: &[PathBuf]) -> String {
        let mut out = self.review.map(Review::header).unwrap_or_default();
        if self.review.is_some() || self.budget.is_some() {
            out.push_str("## Context selection\n\n");
            if let Some(budget) = self.budget {
                out.push_str(&format!(
                    "Output budget: {budget} tokens (o200k_base; other models may differ).\n\n"
                ));
            }
            for file in files {
                let path = &file.rel_path;
                let reason = if let Some(review) = self.review.filter(|r| r.changed.contains(path))
                {
                    if review.deleted.contains(path) {
                        "deleted; base revision content".into()
                    } else if review.untracked.contains(path) {
                        "untracked addition; current content".into()
                    } else {
                        "changed; current content".into()
                    }
                } else {
                    match self.distances.get(path) {
                        Some(0) => "explicitly selected".into(),
                        Some(n) => format!("import graph: {n} hop(s) from a selected file"),
                        None => "repository context".into(),
                    }
                };
                out.push_str(&format!("- {} — {reason}\n", path.display()));
            }
            out.push_str(&format!(
                "\n{} related/candidate files omitted to fit the budget.\n",
                omitted.len()
            ));
            for path in omitted.iter().take(20) {
                out.push_str(&format!("- {}\n", path.display()));
            }
            if omitted.len() > 20 {
                out.push_str("- (remaining omitted paths not listed)\n");
            }
            out.push_str("\nImport matching is heuristic; unconnected files and tests may be missing. File bodies can be truncated by --max-file-size, with inline markers.\n\n");
        }
        let paths: Vec<_> = files.iter().map(|f| f.rel_path.clone()).collect();
        let map = self
            .graph
            .filter(|_| self.map)
            .map(|g| g.map_section(&paths));
        out.push_str(&renderer::render(self.root, files, map.as_deref()));
        out
    }

    /// Preserve seeds and the complete patch. Greedily add whole neighboring
    /// files, nearest first. Recount the entire document including metadata.
    pub fn fit(&self, files: Vec<PackedFile>) -> Result<(String, usize, usize)> {
        let full = self.render(&files, &[]);
        let Some(budget) = self.budget else {
            return Ok((full, files.len(), 0));
        };
        if tokens::count_budget(&full) <= budget {
            return Ok((full, files.len(), 0));
        }
        let (mut selected, mut candidates): (Vec<_>, Vec<_>) = files
            .into_iter()
            .partition(|f| self.distances.get(&f.rel_path) == Some(&0));
        candidates.sort_by(|a, b| {
            self.distances
                .get(&a.rel_path)
                .unwrap_or(&usize::MAX)
                .cmp(self.distances.get(&b.rel_path).unwrap_or(&usize::MAX))
                .then_with(|| a.rel_path.cmp(&b.rel_path))
        });
        let mut omitted: Vec<_> = candidates.iter().map(|f| f.rel_path.clone()).collect();
        let required = tokens::count_budget(&self.render(&selected, &omitted));
        if required > budget {
            bail!(
                "required changes/selected files and review metadata need {required} o200k_base tokens, exceeding --budget {budget}; increase --budget or narrow the selection. No output was written or copied"
            );
        }
        for file in candidates {
            let path = file.rel_path.clone();
            let index = omitted
                .iter()
                .position(|p| p == &path)
                .expect("candidate omitted");
            omitted.remove(index);
            selected.push(file);
            selected.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
            if tokens::count_budget(&self.render(&selected, &omitted)) > budget {
                selected.retain(|f| f.rel_path != path);
                omitted.insert(index, path);
            }
        }
        let markdown = self.render(&selected, &omitted);
        Ok((markdown, selected.len(), omitted.len()))
    }
}
