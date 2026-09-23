use std::{
    collections::HashSet,
    env, fs,
    io::Write,
    panic::{self, AssertUnwindSafe},
};

use surge_ts_checker::{
    CheckerOptions, SourceFileInput, check_program_with_options, check_source_with_options,
};

struct VirtualFile {
    file_name: String,
    source_text: String,
}

fn split_virtual_files(source: &str, fallback_name: &str) -> Vec<VirtualFile> {
    let marker = |line: &str| {
        let trimmed = line.trim_start();
        let rest = trimmed.strip_prefix("//")?.trim_start();
        let rest = rest.strip_prefix('@')?;
        let (name, value) = rest.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("filename") {
            Some(value.trim().to_string())
        } else {
            None
        }
    };

    let has_marker = source.lines().any(|line| marker(line).is_some());
    if !has_marker {
        return vec![VirtualFile {
            file_name: fallback_name.to_string(),
            source_text: source.to_string(),
        }];
    }

    let mut virtual_files = Vec::new();
    let mut current_file_name: Option<String> = None;
    let mut current_source = String::new();
    let mut saw_marker = false;

    for line in source.split_inclusive('\n') {
        let line_without_newline = line.trim_end_matches('\n').trim_end_matches('\r');
        if let Some(file_name) = marker(line_without_newline) {
            saw_marker = true;
            if let Some(previous) = current_file_name.take() {
                virtual_files.push(VirtualFile {
                    file_name: previous,
                    source_text: current_source,
                });
            }
            current_file_name = Some(file_name);
            current_source = String::new();
            continue;
        }
        if saw_marker {
            current_source.push_str(line);
        }
    }

    if let Some(file_name) = current_file_name {
        virtual_files.push(VirtualFile {
            file_name,
            source_text: current_source,
        });
    }

    virtual_files
}

fn header_options(source: &str) -> CheckerOptions {
    let mut options = CheckerOptions::default();

    for line in source.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("//") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('@') else {
            continue;
        };
        let Some((name, value)) = rest.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        let first = value.split(',').next().unwrap_or(value).trim();
        let flag = first.eq_ignore_ascii_case("true");

        match name.as_str() {
            "strict" => options.no_implicit_any = flag,
            "noimplicitany" => options.no_implicit_any = flag,
            "nounusedlocals" => options.no_unused_locals = flag,
            "nounusedparameters" => options.no_unused_parameters = flag,
            "noimplicitreturns" => options.no_implicit_returns = flag,
            "nofallthroughcasesinswitch" => options.no_fallthrough_cases_in_switch = flag,
            "noimplicitoverride" => options.no_implicit_override = flag,
            "nopropertyaccessfromindexsignature" => {
                options.no_property_access_from_index_signature = flag
            }
            "nouncheckedindexedaccess" => options.no_unchecked_indexed_access = flag,
            "allowimportingtsextensions" => options.allow_importing_ts_extensions = flag,
            "allowarbitraryextensions" => options.allow_arbitrary_extensions = flag,
            "skiplibcheck" => options.skip_lib_check = flag,
            "resolvejsonmodule" => options.resolve_json_module = flag,
            "allowumdglobalaccess" => options.allow_umd_global_access = flag,
            "nolib" => options.no_lib = flag,
            "types" => {
                options.types = value
                    .split(',')
                    .map(|entry| entry.trim().to_string())
                    .filter(|entry| !entry.is_empty())
                    .collect()
            }
            "jsx" => {
                let mode = first.to_ascii_lowercase();
                options.jsx_classic_react = mode == "react";
                options.jsx_automatic_runtime = mode == "react-jsx" || mode == "react-jsxdev";
            }
            _ => {}
        }
    }

    options
}

fn codes(options: CheckerOptions, files: &[VirtualFile], program: bool) -> Vec<String> {
    let diagnostics = if program {
        let inputs = files
            .iter()
            .map(|file| SourceFileInput {
                file_name: file.file_name.clone(),
                source_text: file.source_text.clone(),
            })
            .collect();
        check_program_with_options(inputs, options)
    } else {
        let mut all = Vec::new();
        for file in files {
            all.extend(check_source_with_options(
                &file.source_text,
                &file.file_name,
                options.clone(),
            ));
        }
        all
    };
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

#[test]
fn dump_upstream_codes() {
    let Ok(dir) = env::var("SURGE_UPSTREAM_TRIAGE_DIR") else {
        return;
    };
    let out_path = env::var("SURGE_UPSTREAM_TRIAGE_OUT").expect("SURGE_UPSTREAM_TRIAGE_OUT");

    let done: HashSet<String> = fs::read_to_string(&out_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split('\t').next().map(|name| name.trim().to_string()))
        .collect();

    let mut entries: Vec<_> = fs::read_dir(&dir)
        .expect("triage dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    entries.sort();

    panic::set_hook(Box::new(|_| {}));

    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if done.contains(&name) {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap_or_default();
        let files = split_virtual_files(&source, &name);
        let program = files.len() > 1;

        let mut line = format!("{name}\t{}\t", if program { "program" } else { "single" });
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            codes(header_options(&source), &files, program)
        }));
        match result {
            Ok(found) => line.push_str(&found.join(",")),
            Err(_) => line.push_str("PANIC"),
        }
        line.push('\n');

        let mut out = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&out_path)
            .expect("open out");
        out.write_all(line.as_bytes()).expect("write out");
        out.flush().expect("flush out");
    }
}
