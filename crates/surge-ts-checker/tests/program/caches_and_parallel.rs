use surge_ts_checker::{CheckerOptions, SourceFileInput, check_program};

/// Regression coverage for the parallel check phase over script (non-module)
/// files. Every worker builds its per-file declaration table by cloning the
/// prebuilt shared global+ambient table; a worker that rebuilds it instead can
/// observe a different global merge than its peers, which shows up here as a
/// diagnostic difference from the serial run.
#[test]
fn parallel_script_files_match_serial_diagnostics() {
    use surge_ts_checker::check_program_with_stats_and_jobs;

    let files: Vec<SourceFileInput> = (0..8)
        .map(|i| SourceFileInput {
            file_name: format!("script_{i}.ts"),
            source_text: format!(
                "const good_{i}: number = {i};\nconst bad_{i}: string = {i};\nfunction helper_{i}(x: number): number {{ return x + good_{i}; }}\nhelper_{i}(bad_{i});\n"
            ),
        })
        .collect();

    let serial = check_program_with_stats_and_jobs(files.clone(), CheckerOptions::default(), 1);
    let parallel = check_program_with_stats_and_jobs(files, CheckerOptions::default(), 8);

    assert!(
        !serial.diagnostics.is_empty(),
        "expected the script fixtures to produce diagnostics"
    );
    let render = |diags: &[surge_ts_diagnostics::Diagnostic]| {
        let mut rendered: Vec<String> = diags
            .iter()
            .map(|d| format!("{} {} {}", d.file_name, d.code, d.message))
            .collect();
        rendered.sort();
        rendered
    };
    assert_eq!(render(&serial.diagnostics), render(&parallel.diagnostics));
}

/// One fixture module per index: a generic interface exercised at several
/// instantiations, one deliberate TS2322, and a cross-file import chain so the
/// module binding/import paths (whose preliminary structures are dropped at
/// `preliminary_release`) are all live.
fn region_fixture_files(count: usize) -> Vec<SourceFileInput> {
    (0..count)
        .map(|i| {
            let import = if i == 0 {
                String::new()
            } else {
                format!("import {{ ok_{p} }} from \"./mod_{p}\";\n", p = i - 1)
            };
            let use_import = if i == 0 {
                String::new()
            } else {
                format!("export const chained_{i}: string = ok_{p}.value;\n", p = i - 1)
            };
            SourceFileInput {
                file_name: format!("mod_{i}.ts"),
                source_text: format!(
                    "{import}export interface RegionBox_{i}<T> {{ value: T; }}\n\
                     export type RegionPair_{i}<T> = {{ first: T; second: RegionBox_{i}<T> }};\n\
                     export const ok_{i}: RegionBox_{i}<string> = {{ value: \"ok\" }};\n\
                     export const bad_{i}: RegionBox_{i}<number> = {{ value: \"oops\" }};\n\
                     export function use_{i}(input: RegionPair_{i}<boolean>): boolean {{ return input.first; }}\n\
                     {use_import}"
                ),
            }
        })
        .collect()
}

fn rendered_sorted(diags: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    let mut rendered: Vec<String> = diags
        .iter()
        .map(|d| format!("{} {} {:?} {}", d.file_name, d.code, d.span, d.message))
        .collect();
    rendered.sort();
    rendered
}

/// Region regression: a parallel worker context is reused across many files
/// (error files interleaved with clean ones), and `begin_file_check` resets the
/// file region between them. Any leak of one file's dedup keys or caches into
/// the next would make parallel output diverge from serial (which clones a
/// fresh context per file).
#[test]
fn parallel_worker_reuse_across_many_module_files_matches_serial() {
    use surge_ts_checker::check_program_with_stats_and_jobs;

    let files = region_fixture_files(24);
    let serial = check_program_with_stats_and_jobs(files.clone(), CheckerOptions::default(), 1);
    let parallel = check_program_with_stats_and_jobs(files, CheckerOptions::default(), 4);

    assert!(
        serial.diagnostics.len() >= 24,
        "expected one TS2322 per fixture file, got {}",
        serial.diagnostics.len()
    );
    assert_eq!(
        rendered_sorted(&serial.diagnostics),
        rendered_sorted(&parallel.diagnostics)
    );
}

/// The expected diagnostic surface of `region_fixture_files(6)`, the same one
/// the default-cap fixture pins against tsc: the generic instantiation caches
/// are recomputable memos, so any bucket cap must produce byte-identical
/// diagnostics (only time/memory may change).
fn assert_region_fixture_diagnostics(diags: &[surge_ts_diagnostics::Diagnostic]) {
    let ts2322: Vec<&surge_ts_diagnostics::Diagnostic> = diags
        .iter()
        .filter(|d| d.code.to_string() == "TS2322")
        .collect();
    assert_eq!(
        ts2322.len(),
        6,
        "expected exactly one TS2322 per fixture file: {:?}",
        rendered_sorted(diags)
    );
    for (i, diagnostic) in ts2322.iter().enumerate() {
        assert_eq!(diagnostic.file_name, format!("mod_{i}.ts"));
    }
}

/// Same fixture and same golden expectation as the default-cap fixture
/// (tests/checker/program_caches_and_parallel/generic_cache_default_cap_expected_diagnostics.ts),
/// but with the per-declaration cache bucket cap forced to 1 (over-cap
/// instantiations recompute instead of caching). Also checks a second
/// in-process run for determinism under the bound. The test runs in its own
/// process, so the env override cannot leak into other tests.
#[test]
fn generic_cache_bounded_cap_expected_diagnostics() {
    if super::run_in_own_process(module_path!(), "generic_cache_bounded_cap_expected_diagnostics") {
        return;
    }
    // Safety: set before any checker thread is spawned in this isolated,
    // single-test process.
    unsafe { std::env::set_var("SURGE_GENERIC_CACHE_BUCKET_CAP", "1") };
    let first = check_program(region_fixture_files(6));
    assert_region_fixture_diagnostics(&first);
    let second = check_program(region_fixture_files(6));
    assert_eq!(rendered_sorted(&first), rendered_sorted(&second));
}

fn program_files(files: Vec<SourceFileInput>) -> Vec<surge_ts_diagnostics::Diagnostic> {
    check_program(files)
}

/// One consumer module per index for the signature-context generic tier: each
/// module only *references* `core.ts`'s `Internals<string, number>` from inside
/// a generic function signature (a non-concrete resolution site) and carries
/// one anchor TS2322. Reusing one module's clean expansion for the next must
/// not change any module's diagnostics.
fn signature_context_fixture_files(count: usize) -> Vec<SourceFileInput> {
    let mut files = vec![SourceFileInput {
        file_name: "core.ts".to_string(),
        source_text: "export interface Internals<O, I> {\n\
             \x20 out: O;\n\
             \x20 inp: I;\n\
             \x20 parse(value: I): O;\n\
             }\n"
        .to_string(),
    }];
    files.extend((0..count).map(|i| SourceFileInput {
        file_name: format!("use_{i}.ts"),
        source_text: format!(
            "import {{ Internals }} from \"./core\";\n\
             export function pick_{i}<T>(seed: T, internals: Internals<string, number>): string {{\n\
             \x20 return internals.out;\n\
             }}\n\
             export function feed_{i}<T>(seed: T, internals: Internals<string, number>): number {{\n\
             \x20 return internals.inp;\n\
             }}\n\
             export const bad_{i}: string = {i};\n"
        ),
    }));
    files
}

fn signature_context_expected(diags: &[surge_ts_diagnostics::Diagnostic], count: usize) {
    let ts2322: Vec<_> = diags
        .iter()
        .filter(|d| d.code.to_string() == "TS2322")
        .collect();
    assert_eq!(
        ts2322.len(),
        count,
        "expected exactly the per-module anchor TS2322s: {:?}",
        rendered_sorted(diags)
    );
    for diagnostic in &ts2322 {
        assert!(
            diagnostic.message.contains("bad_")
                || diagnostic
                    .message
                    .contains("Type 'number' is not assignable to type 'string'"),
            "unexpected TS2322: {} {}",
            diagnostic.file_name,
            diagnostic.message
        );
    }
}

/// Repeated identical user-generic instantiations inside generic signatures
/// must produce byte-identical diagnostics whether or not the expansion is
/// reused, across repeated in-process runs and across job counts.
#[test]
fn signature_context_generic_reuse_matches_fresh_expansion() {
    use surge_ts_checker::check_program_with_stats_and_jobs;

    let files = signature_context_fixture_files(12);
    let first = check_program(files.clone());
    signature_context_expected(&first, 12);

    let second = check_program(files.clone());
    assert_eq!(rendered_sorted(&first), rendered_sorted(&second));

    for jobs in [2usize, 4, 8] {
        let parallel =
            check_program_with_stats_and_jobs(files.clone(), CheckerOptions::default(), jobs);
        assert_eq!(
            rendered_sorted(&first),
            rendered_sorted(&parallel.diagnostics),
            "jobs={jobs} diverged"
        );
    }
}

/// A recursive generic interface referenced from generic signatures stays
/// sound: repeated references and repeated runs are stable.
#[test]
fn signature_context_recursive_generic_repeated_references_are_stable() {
    let mut files = vec![SourceFileInput {
        file_name: "core.ts".to_string(),
        source_text: "export interface Chain<T> { value: T; next: Chain<T>; }\n".to_string(),
    }];
    files.extend((0..6).map(|i| SourceFileInput {
        file_name: format!("use_{i}.ts"),
        source_text: format!(
            "import {{ Chain }} from \"./core\";\n\
             export function walk_{i}<T>(seed: T, chain: Chain<string>): string {{\n\
             \x20 return chain.next.value;\n\
             }}\n\
             export const bad_{i}: string = {i};\n"
        ),
    }));
    let first = program_files(files.clone());
    let second = program_files(files);
    assert_eq!(rendered_sorted(&first), rendered_sorted(&second));
    let ts2322 = first
        .iter()
        .filter(|d| d.code.to_string() == "TS2322")
        .count();
    assert_eq!(ts2322, 6, "anchors only: {:?}", rendered_sorted(&first));
}

/// Instantiations whose arguments carry an in-scope type parameter (which
/// resolves to the `unknown` placeholder) must stay uncached and stable.
#[test]
fn signature_context_placeholder_arguments_stay_stable() {
    let mut files = vec![SourceFileInput {
        file_name: "core.ts".to_string(),
        source_text: "export interface Internals<O, I> { out: O; inp: I; }\n".to_string(),
    }];
    files.extend((0..6).map(|i| SourceFileInput {
        file_name: format!("use_{i}.ts"),
        source_text: format!(
            "import {{ Internals }} from \"./core\";\n\
             export function poly_{i}<T>(seed: T, internals: Internals<T, number>): number {{\n\
             \x20 return internals.inp;\n\
             }}\n\
             export const bad_{i}: string = {i};\n"
        ),
    }));
    let first = program_files(files.clone());
    let second = program_files(files);
    assert_eq!(rendered_sorted(&first), rendered_sorted(&second));
    let ts2322 = first
        .iter()
        .filter(|d| d.code.to_string() == "TS2322")
        .count();
    assert_eq!(ts2322, 6, "anchors only: {:?}", rendered_sorted(&first));
}

/// The per-declaration bucket cap only causes recomputation, never different
/// diagnostics — including for signature-context instantiations.
#[test]
fn signature_context_bounded_cap_identical_diagnostics() {
    if super::run_in_own_process(
        module_path!(),
        "signature_context_bounded_cap_identical_diagnostics",
    ) {
        return;
    }
    // Safety: set before any checker thread is spawned in this isolated,
    // single-test process.
    unsafe { std::env::set_var("SURGE_GENERIC_CACHE_BUCKET_CAP", "1") };
    let files = signature_context_fixture_files(6);
    let bounded = check_program(files.clone());
    signature_context_expected(&bounded, 6);
    let again = check_program(files);
    assert_eq!(rendered_sorted(&bounded), rendered_sorted(&again));
}

/// Cross-run isolation: a second in-process program that redeclares the same
/// file/declaration/argument tuple with a *different* shape must see its own
/// shape, proving instantiation caches do not survive program teardown.
#[test]
fn signature_context_cache_does_not_leak_across_programs() {
    let first_files = vec![
        SourceFileInput {
            file_name: "core.ts".to_string(),
            source_text: "export interface Internals<O, I> { out: O; inp: I; }\n".to_string(),
        },
        SourceFileInput {
            file_name: "use.ts".to_string(),
            source_text: "import { Internals } from \"./core\";\n\
                 export function f<T>(seed: T, i: Internals<string, number>): string {\n\
                 \x20 return i.out;\n\
                 }\n"
            .to_string(),
        },
    ];
    let first = check_program(first_files);
    assert!(
        first.is_empty(),
        "first program is clean: {:?}",
        rendered_sorted(&first)
    );

    // Same file names, same tuple, but `out` is now the *second* parameter.
    let second_files = vec![
        SourceFileInput {
            file_name: "core.ts".to_string(),
            source_text: "export interface Internals<O, I> { out: I; inp: I; }\n".to_string(),
        },
        SourceFileInput {
            file_name: "use.ts".to_string(),
            source_text: "import { Internals } from \"./core\";\n\
                 export function f<T>(seed: T, i: Internals<string, number>): string {\n\
                 \x20 return i.out;\n\
                 }\n"
            .to_string(),
        },
    ];
    let second = check_program(second_files);
    assert_eq!(
        second
            .iter()
            .map(|d| d.code.to_string())
            .collect::<Vec<_>>(),
        vec!["TS2322".to_string()],
        "second program's reshaped `out: I` must be seen: {:?}",
        rendered_sorted(&second)
    );
}

/// The zod `util.MakeReadonly` shape: a distributive conditional whose true
/// branch instantiates a library generic from `infer` captures.
const MAKE_READONLY_SHAPE: &str = "export type MakeRO<T> = T extends Map<infer K, infer V>\n\
     \x20 ? ReadonlyMap<K, V>\n\
     \x20 : Readonly<T>;\n";

/// The full fixture set must produce byte-identical rendered diagnostics
/// across job counts and repeated runs.
#[test]
fn distributive_conditional_fixtures_deterministic_across_jobs() {
    use surge_ts_checker::check_program_with_stats_and_jobs;

    let files = vec![
        SourceFileInput {
            file_name: "util.ts".to_string(),
            source_text: MAKE_READONLY_SHAPE.to_string(),
        },
        SourceFileInput {
            file_name: "any_use.ts".to_string(),
            source_text: "import { MakeRO } from \"./util\";\n\
                 export interface Holder<T> { value: MakeRO<T>; }\n\
                 export function go<T>(seed: T, h: Holder<any>): void {\n\
                 \x20 const v = h.value;\n\
                 }\n"
            .to_string(),
        },
        SourceFileInput {
            file_name: "map_use.ts".to_string(),
            source_text: "import { MakeRO } from \"./util\";\n\
                 type RO = MakeRO<Map<string, number>>;\n\
                 declare const ro: RO;\n\
                 export const bad: string = ro.size;\n"
                .to_string(),
        },
    ];
    let serial =
        check_program_with_stats_and_jobs(files.clone(), CheckerOptions::default(), 1).diagnostics;
    for jobs in [2usize, 4] {
        let parallel =
            check_program_with_stats_and_jobs(files.clone(), CheckerOptions::default(), jobs)
                .diagnostics;
        assert_eq!(
            rendered_sorted(&serial),
            rendered_sorted(&parallel),
            "jobs={jobs} diverged"
        );
    }
}
