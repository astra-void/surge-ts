use std::{
    env,
    path::{Path, PathBuf},
};

pub(crate) fn resolve_project_path(project: &Path) -> (PathBuf, PathBuf) {
    let project = absolutize(project);

    if project.exists() && project.is_file() {
        let root_dir = project
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| project.clone());
        return (project, root_dir);
    }

    if project.exists() && project.is_dir() {
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

pub(crate) fn resolve_path(base_dir: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

pub(crate) fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    match env::current_dir() {
        Ok(current_dir) => current_dir.join(path),
        Err(_) => path.to_path_buf(),
    }
}

pub(crate) fn canonicalize_if_exists(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) fn cycle_key(path: &Path) -> PathBuf {
    canonicalize_if_exists(path)
}
