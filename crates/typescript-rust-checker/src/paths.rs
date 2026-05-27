use std::path::{Path, PathBuf};

pub(crate) fn canonicalize_if_exists_string(path: &Path) -> String {
    crate::program::record_string_path_lookup();
    canonicalize_if_exists(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(crate) fn normalize_path_string(path: &str) -> String {
    crate::program::record_string_path_lookup();
    normalize_path_buf(Path::new(path))
        .to_string_lossy()
        .replace('\\', "/")
}

fn canonicalize_if_exists(path: &Path) -> PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        normalize_path_buf(&canonical)
    } else {
        normalize_path_buf(path)
    }
}

fn normalize_path_buf(path: &Path) -> PathBuf {
    let path = path.to_string_lossy().replace('\\', "/");
    let is_absolute = path.starts_with('/');
    let mut drive_letter = "";

    let path_to_split = if path.chars().nth(1) == Some(':') {
        drive_letter = &path[0..2];
        &path[2..]
    } else {
        &path
    };

    let mut segments = Vec::new();
    for segment in path_to_split.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if segment == ".." {
            if let Some(last) = segments.last() {
                if last != ".." {
                    segments.pop();
                    continue;
                }
            }

            if !is_absolute && drive_letter.is_empty() {
                segments.push(segment.to_string());
            }

            continue;
        }

        segments.push(segment.to_string());
    }

    let mut result = String::new();
    if !drive_letter.is_empty() {
        result.push_str(drive_letter);
        if path_to_split.starts_with('/') {
            result.push('/');
        }
    } else if is_absolute {
        result.push('/');
    }

    result.push_str(&segments.join("/"));

    if result.is_empty() {
        if is_absolute {
            PathBuf::from("/")
        } else {
            PathBuf::from(".")
        }
    } else {
        PathBuf::from(result)
    }
}
