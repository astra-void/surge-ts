use surge_ts_checker::check_source;

fn codes(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn check(source_text: &str) -> Vec<surge_ts_diagnostics::Diagnostic> {
    check_source(source_text, "example.ts")
}

// The hook-option shape `Hook | Hook[]` (ofetch's `FetchHooks`, tRPC's option
// bags): a callback written against it takes its parameter types from the
// union's single callable member, both as a method shorthand and as an arrow.
#[test]
fn union_of_hook_and_hook_array_types_a_method_shorthand() {
    let diagnostics = check(
        "interface Ctx { options: { body?: string } }\n\
         type MaybeArray<T> = T | T[];\n\
         type Hook = (context: Ctx) => void;\n\
         interface Opts { onResponse?: MaybeArray<Hook> }\n\
         declare function request(url: string, opts?: Opts): void;\n\
         request(\"/x\", {\n\
             onResponse(ctx) {\n\
                 const body: string | undefined = ctx.options.body;\n\
                 void body;\n\
             },\n\
         });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn union_of_hook_and_hook_array_types_an_arrow_property() {
    let diagnostics = check(
        "interface Ctx { options: { body?: string } }\n\
         type Hook = (context: Ctx) => void;\n\
         interface Opts { onResponse?: Hook | Hook[] }\n\
         declare function request(url: string, opts?: Opts): void;\n\
         request(\"/x\", {\n\
             onResponse: (ctx) => {\n\
                 const body: string | undefined = ctx.options.body;\n\
                 void body;\n\
             },\n\
         });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Several callable members cannot be told apart without signature matching, so
// the callback must not be typed as one of them — using the parameter as the
// *other* member's type stays clean rather than reporting against the guess.
#[test]
fn union_of_several_callables_does_not_bind_one_members_parameters() {
    let diagnostics = check(
        "type H1 = (a: string) => void;\n\
         type H2 = (a: number) => void;\n\
         interface Opts { on?: H1 | H2 }\n\
         declare function request(opts: Opts): void;\n\
         request({ on: (a) => { const n: number = a; void n; } });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
