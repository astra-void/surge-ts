use std::{fs, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir.parent().expect("workspace crates dir");
    let diagnostics_dir = crates_dir.join("surge-ts-diagnostics");
    let catalog_path = diagnostics_dir.join("diagnostic-messages.json");
    let generated_path = diagnostics_dir.join("src/generated.rs");
    let snapshot_path =
        diagnostics_dir.join("tests/fixtures/typescript-diagnostics/catalog.snapshot.toml");

    let entries = match surge_ts_diagnostics_codegen::load_catalog(&catalog_path) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("failed to load catalog {}: {error}", catalog_path.display());
            return ExitCode::from(1);
        }
    };

    let generated = match surge_ts_diagnostics_codegen::generate_rust(&entries) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("failed to generate Rust output: {error}");
            return ExitCode::from(1);
        }
    };

    if let Err(error) = fs::write(&generated_path, generated) {
        eprintln!("failed to write {}: {error}", generated_path.display());
        return ExitCode::from(1);
    }

    let snapshot = match surge_ts_diagnostics_codegen::generate_snapshot_toml(&entries) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("failed to generate TOML snapshot: {error}");
            return ExitCode::from(1);
        }
    };

    if let Err(error) = fs::write(&snapshot_path, snapshot) {
        eprintln!("failed to write {}: {error}", snapshot_path.display());
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}
