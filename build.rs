/// Build script — regenerates FlatBuffers Rust bindings from .fbs schemas.
///
/// Requires `flatc` >= 24.x. Point the `FLATC` env-var at the binary if it is
/// not on `PATH`.  If `flatc` cannot be found the build continues using the
/// pre-committed generated files inside `src/protocol/generated/`.
use std::path::Path;
use std::process::Command;

fn main() {
    let schemas = [
        "fbs/common.fbs",
        "fbs/control.fbs",
        "fbs/chat.fbs",
        "fbs/room.fbs",
        "fbs/shard.fbs",
        "fbs/ack.fbs",
        "fbs/upload.fbs",
        "fbs/presence.fbs",
        "fbs/error.fbs",
    ];

    // Re-run this script whenever a schema changes.
    for schema in &schemas {
        println!("cargo:rerun-if-changed={}", schema);
    }
    println!("cargo:rerun-if-env-changed=FLATC");

    let flatc = std::env::var("FLATC").unwrap_or_else(|_| "flatc".to_string());

    // Skip generation if flatc is not available — use pre-committed code.
    if !flatc_available(&flatc) {
        println!(
            "cargo:warning=flatc not found at '{}'; using pre-committed generated code",
            flatc
        );
        return;
    }

    let out_dir = "src/protocol/generated";
    std::fs::create_dir_all(out_dir).expect("failed to create generated dir");

    let status = Command::new(&flatc)
        .args(["--rust", "--gen-mutable", "--gen-name-strings", "-I", "fbs", "-o", out_dir])
        .args(&schemas)
        .status()
        .expect("failed to run flatc");

    assert!(status.success(), "flatc exited with status {}", status);

    // Post-process generated files to fix cross-module import paths.
    //
    // flatc generates `use crate::common_generated::*` assuming all generated
    // files sit at the crate root.  Since we nest them under
    // `src/protocol/generated/`, we rewrite each generated import to use
    // `super::` relative paths instead.
    fix_generated_imports(out_dir);
}

/// Replace `use crate::{name}_generated::*` with `use super::{name}_generated::garou::*`
/// in every generated Rust file in `out_dir`.
fn fix_generated_imports(out_dir: &str) {
    let dir = std::path::Path::new(out_dir);
    let entries: Vec<_> = dir
        .read_dir()
        .expect("failed to read generated dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "rs").unwrap_or(false))
        .collect();

    for entry in entries {
        let path = entry.path();
        let content = std::fs::read_to_string(&path).expect("failed to read generated file");

        // Replace all occurrences of:
        //   use crate::<module>_generated::*;
        // with:
        //   use super::<module>_generated::garou::*;
        let mut fixed = content.clone();

        // Collect module names referenced as crate:: imports
        let prefix = "use crate::";
        let suffix = "_generated::*;";
        let mut to_replace: Vec<String> = vec![];
        let mut pos = 0;
        while let Some(start) = fixed[pos..].find(prefix) {
            let abs_start = pos + start;
            let after = &fixed[abs_start + prefix.len()..];
            if let Some(end) = after.find(suffix) {
                let mod_name = &after[..end];
                if !to_replace.contains(&mod_name.to_string()) {
                    to_replace.push(mod_name.to_string());
                }
                pos = abs_start + 1;
            } else {
                break;
            }
        }

        for mod_name in &to_replace {
            let old_import = format!("use crate::{}{}::*;", mod_name, "_generated");
            let new_file_level = format!("use super::{}{}::garou::*;", mod_name, "_generated");
            let new_mod_level  = format!("  use super::super::{}{}::garou::*;", mod_name, "_generated");
            // Module-level imports (2-space indented) need an extra `super::`.
            let indented_old = format!("  {}", old_import);
            fixed = fixed.replace(&indented_old, &new_mod_level);
            fixed = fixed.replace(&old_import, &new_file_level);
        }

        if fixed != content {
            std::fs::write(&path, &fixed).expect("failed to write patched generated file");
        }
    }
}

fn flatc_available(binary: &str) -> bool {
    Command::new(binary)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Verify that the generated directory exists (non-empty) so that the crate
/// can always compile even without flatc installed.
fn _check_generated_exists() {
    let path = Path::new("src/protocol/generated");
    if !path.exists() || path.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        panic!(
            "src/protocol/generated/ is empty and flatc is not available. \
             Commit the generated files or install flatc."
        );
    }
}
