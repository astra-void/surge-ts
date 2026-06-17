//! Node built-in module specifier recognition, mirroring tsc's
//! `nodeCoreModules` set and `getCannotResolveModuleNameErrorForSpecificModule`.
//!
//! TypeScript does not bridge `node:fs` to `fs` (or vice versa): each
//! `declare module` is matched by exact specifier string. The only
//! Node-protocol-specific behavior is the missing-module diagnostic: when an
//! unresolved specifier names a Node built-in, tsc reports the install-node hint
//! (TS2580 / TS2591) instead of the generic TS2307. The lists and the set
//! construction below mirror tsc 6.0.3 exactly.

use std::collections::HashSet;
use std::sync::LazyLock;

use typescript_rust_diagnostics::Diagnostic;

use crate::context::CheckerContext;

const UNPREFIXED_NODE_CORE_MODULES_LIST: &[&str] = &[
    "assert",
    "assert/strict",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "dns/promises",
    "domain",
    "events",
    "fs",
    "fs/promises",
    "http",
    "http2",
    "https",
    "inspector",
    "inspector/promises",
    "module",
    "net",
    "os",
    "path",
    "path/posix",
    "path/win32",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "readline/promises",
    "repl",
    "stream",
    "stream/consumers",
    "stream/promises",
    "stream/web",
    "string_decoder",
    "sys",
    "timers",
    "timers/promises",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "util/types",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

const EXCLUSIVELY_PREFIXED_NODE_CORE_MODULES: &[&str] = &[
    "node:quic",
    "node:sea",
    "node:sqlite",
    "node:test",
    "node:test/reporters",
];

static NODE_CORE_MODULES: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let mut set = HashSet::new();
    for name in UNPREFIXED_NODE_CORE_MODULES_LIST {
        set.insert((*name).to_string());
        set.insert(format!("node:{name}"));
    }
    for name in EXCLUSIVELY_PREFIXED_NODE_CORE_MODULES {
        set.insert((*name).to_string());
    }
    set
});

/// Mirrors tsc's `getCannotResolveModuleNameErrorForSpecificModule`: a Node
/// built-in specifier yields the install-@types/node hint (TS2580 under a
/// `types: ["*"]` wildcard, TS2591 otherwise); anything else yields `None` so
/// the caller falls back to the generic TS2307.
pub(crate) fn cannot_resolve_module_name_error_for_specific_module(
    ctx: &CheckerContext,
    module_specifier: &str,
) -> Option<Diagnostic> {
    if !NODE_CORE_MODULES.contains(module_specifier) {
        return None;
    }

    Some(if ctx.options.types_uses_wildcard() {
        Diagnostic::ts2580(module_specifier, ctx.file_name.clone())
    } else {
        Diagnostic::ts2591(module_specifier, ctx.file_name.clone())
    })
}
