use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::context::CheckerContext;
use crate::program::{ProgramTimings, record_program_file_timing, record_program_timing};

use super::registry::{GeneratedDefaultLibSnapshot, generated_default_lib_snapshot_for_file_name};

pub(crate) fn inject_generated_default_lib_snapshot_for_file_name(
    file_name: &str,
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> bool {
    let Some(snapshot) = generated_default_lib_snapshot_for_file_name(file_name) else {
        return false;
    };

    inject_generated_default_lib_snapshot(snapshot, ctx, timings);
    true
}

fn inject_generated_default_lib_snapshot(
    snapshot: &'static GeneratedDefaultLibSnapshot,
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) {
    let start = Instant::now();
    let lowered_type_declarations = snapshot.type_declarations.len() as u64;

    for (name, declaration) in snapshot.type_declarations.iter() {
        let _ = ctx
            .ambient_global_type_declarations
            .insert(name.clone(), declaration.clone());
    }

    for (name, symbol) in snapshot.symbols.iter_shared() {
        if name.as_ref() == "globalThis" {
            continue;
        }

        if ctx.ambient_global_symbols.get(name).is_none() {
            let _ = ctx
                .ambient_global_symbols
                .insert_shared(name.clone(), symbol.clone());
        }
    }

    record_program_timing(timings, |timings| {
        let elapsed = start.elapsed();
        timings.generated_default_lib_lower_time += elapsed;
        timings.generated_default_lib_global_collection += elapsed;
    });

    record_program_file_timing(timings, snapshot.file_name, |metrics| {
        metrics.collect_type_declarations_passes += 1;
        metrics.lowered_type_declarations += lowered_type_declarations;
    });
}
