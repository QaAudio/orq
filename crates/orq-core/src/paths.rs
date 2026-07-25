use globset::{Glob, GlobSetBuilder};

/// Returns true if two path globs can overlap (conservative).
pub fn globs_overlap(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    // Exact path contained in glob, or mutual prefix heuristics.
    if glob_matches(a, b) || glob_matches(b, a) {
        return true;
    }
    // Strip ** and * for prefix comparison
    let na = normalize_prefix(a);
    let nb = normalize_prefix(b);
    if na.is_empty() || nb.is_empty() {
        return true;
    }
    na.starts_with(&nb) || nb.starts_with(&na)
}

fn normalize_prefix(g: &str) -> String {
    g.split(['*', '?'])
        .next()
        .unwrap_or("")
        .trim_end_matches(['/', '\\'])
        .replace('\\', "/")
        .to_lowercase()
}

pub fn glob_matches(pattern: &str, path: &str) -> bool {
    let Ok(glob) = Glob::new(pattern) else {
        return pattern == path;
    };
    let mut builder = GlobSetBuilder::new();
    builder.add(glob);
    let Ok(set) = builder.build() else {
        return false;
    };
    set.is_match(path) || set.is_match(path.replace('\\', "/"))
}

pub fn claims_overlap(a: &[String], b: &[String]) -> bool {
    for x in a {
        for y in b {
            if globs_overlap(x, y) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_same() {
        assert!(globs_overlap("src/**", "src/**"));
    }

    #[test]
    fn overlap_nested() {
        assert!(globs_overlap("src/**", "src/engine/**"));
    }

    #[test]
    fn no_overlap_siblings() {
        assert!(!globs_overlap("src/a/**", "src/b/**"));
    }
}
