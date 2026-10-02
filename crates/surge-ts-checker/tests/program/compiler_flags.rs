use surge_ts_checker::CheckerOptions;

use super::*;

fn emit_collision_codes(no_emit: bool) -> Vec<String> {
    let source = "export {};\nvar require = 1;\nconst Reflect = 1;\nclass A { static x = 1 }\nclass B extends A { static y = super.x }\nexport const __esModule = true;\n";
    let options = CheckerOptions {
        module_emit: surge_ts_checker::ModuleEmitKind::CommonJS,
        target_es2022: false,
        no_emit,
        ..Default::default()
    };
    let mut codes = codes(&program_with_options(&[("a.ts", source)], options));
    codes.sort();
    codes
}

#[test]
fn commonjs_emit_reserves_wrapper_names() {
    assert_eq!(emit_collision_codes(false), vec!["TS1216", "TS2441", "TS2818"]);
}
