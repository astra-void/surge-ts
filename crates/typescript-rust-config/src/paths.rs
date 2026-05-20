use std::{
    env,
    path::{Path, PathBuf},
};

pub fn resolve_project_path(project: &Path) -> (PathBuf, PathBuf) {
    let project = absolutize(project);

    if project.exists() && project.is_file() {
        let project = canonicalize_if_exists(&project);
        let root_dir = project
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| project.clone());
        return (project, root_dir);
    }

    if project.exists() && project.is_dir() {
        let project = canonicalize_if_exists(&project);
        let config_path = project.join("tsconfig.json");
        return (config_path, project);
    }

    if project
        .file_name()
        .is_some_and(|name| name == "tsconfig.json")
        || project.extension().is_some_and(|ext| ext == "json")
    {
        let root_dir = project
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        (project, root_dir)
    } else {
        let config_path = project.join("tsconfig.json");
        (config_path, project)
    }
}

pub fn resolve_path(base_dir: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

pub fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    match env::current_dir() {
        Ok(current_dir) => current_dir.join(path),
        Err(_) => path.to_path_buf(),
    }
}

pub fn canonicalize_if_exists(path: &Path) -> PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        normalize_path_buf(&canonical)
    } else {
        normalize_path_buf(path)
    }
}

#[allow(dead_code)]
pub fn canonicalize_if_exists_string(path: &Path) -> String {
    canonicalize_if_exists(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn cycle_key(path: &Path) -> PathBuf {
    canonicalize_if_exists(path)
}

pub fn normalize_path_string(path: &str) -> String {
    normalize_path_buf(Path::new(path))
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn normalize_path_buf(path: &Path) -> PathBuf {
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
