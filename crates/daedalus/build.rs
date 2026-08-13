//! This build script reads the specific platform that is being
//! targetted for `Daedalus` and uses the corresponding linker script
//! in the ld directory for that platform.

use std::{collections::HashSet, env, fs, path::PathBuf};

fn main() {
    // Cargo inserts features as ENV also with CARGO_FEATURE_<FEATURE_NAME>,
    // all platform `Daedalus` features begin with PLATFORM_ so we can grab them from the envs.
    let mut platforms: Vec<String> = env::vars()
        .filter_map(|(key, _)| {
            key.strip_prefix("CARGO_FEATURE_PLATFORM_")
                .map(|name| name.to_ascii_lowercase().replace('_', "-"))
        })
        .collect();

    platforms.sort();

    // Get all of the linker scripts these platforms refer to
    let mut all_valid_link_files = HashSet::new();

    for platform in platforms {
        let link_files_src = PathBuf::from("ld").join(format!("{platform}.ld"));

        // Check if it exists
        if link_files_src.exists() {
            // A valid one! add it to our total valids.
            all_valid_link_files.insert(link_files_src);
        }
    }

    // Only one valid linker script can be used in the end.
    let actual_linker_script = match all_valid_link_files.iter().collect::<Vec<_>>().as_slice() {
        [p] => *p,
        [] => panic!(
            "\x1b[93mno platform was selected with a valid linker script under ld; select one or create one.\x1b[0m"
        ),
        many => panic!(
            "\x1b[93mplatform linker scripts are mutually exclusive. We can only have one enabled, got: {many:?}\x1b[0m"
        ),
    };

    // copy to OUT_DIR, which is where the linker will search for it (under link.ld)
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::copy(actual_linker_script, out.join("link.ld")).unwrap();

    // actual rustc changes for linker
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=-Tlink.ld");
    println!("cargo:rustc-link-arg=--nmagic");
    println!("cargo:rerun-if-changed={}", actual_linker_script.display());
    println!("cargo:rerun-if-changed=ld");
}
