use std::fmt::Write;
use std::{collections::HashSet, env, fs, path::PathBuf};

use serde::Deserialize;

// These are the builtin services that are handled by the daedalus core rather
// than a daedalus program.
const BUILTIN_SERVICES: [&str; 2] = ["memory::write_mem_32", "memory::read_mem_32"];

/// The parsed content of a Daedalus
/// program manifest.
#[derive(Deserialize)]
struct Manifest {
    // The name defined for this program in the manifest
    name: String,

    // The services defined in this manifest
    #[serde(default)]
    services: Vec<String>,

    // The services this program requires
    #[serde(default)]
    requires: Vec<String>,

    // The source lepton3 image for this program
    image: String,
}

fn main() {
    // Daedalus programs if the environment variable is changed needs to rerun
    println!("cargo:rerun-if-env-changed=DAEDALUS_PROGRAMS");

    // The temp output for our programs
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // The output programs.rs file which we are generating that will contain all of the built
    // programs as `Program`'s generated from their manifests
    let out_file = out_dir.join("programs.rs");

    // All of the files we are testing
    let list = env::var("DAEDALUS_PROGRAMS").unwrap_or_default();
    let entries: Vec<&str> = list
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect();

    // No entries??? We die because likely this is user error.
    if entries.is_empty() {
        panic!(
            "\x1b[93mDAEDALUS_PROGRAMS is empty, embedding no programs is not permitted. Exiting.\x1b[0m"
        );
    }

    // The seen set of names, a duplicate program name is not permitted
    let mut seen = HashSet::new();

    // All referenced services, services provided must be a superset of required
    let mut required = HashSet::new();
    let mut provided = HashSet::with_capacity(BUILTIN_SERVICES.len());
    provided.extend(BUILTIN_SERVICES.map(String::from));

    // The generated output will be the set of all programs described
    let mut output = String::from("pub static PROGRAMS: &[Program] = &[\n");

    for entry in entries {
        let entry_path = PathBuf::from(entry);
        println!("cargo:rerun-if-changed={}", entry_path.display());

        // Parse the manifest format
        let manifest: Manifest =
            toml::from_str(&fs::read_to_string(&entry_path).unwrap_or_else(|e| {
                panic!("\x1b[93merror reading {}: {e}\x1b[0m", entry_path.display())
            }))
            .unwrap_or_else(|e| {
                panic!("\x1b[93merror parsing {}: {e}\x1b[0m", entry_path.display())
            });

        // If we have a duplicate name this isn't allowed as those programs would conflict
        if !seen.insert(manifest.name.clone()) {
            panic!("\x1b[93mduplicate program name `{}`\x1b[0m", manifest.name);
        }

        // The generated include_bytes! for the image to include it as part of our full bootloader image
        let image_path = entry_path.parent().unwrap().join(&manifest.image);
        let image_path = fs::canonicalize(&image_path).unwrap_or_else(|e| {
            panic!(
                "\x1b[93mprogram `{}`: image {} not found ({e}), build it first\x1b[0m",
                manifest.name,
                image_path.display(),
            )
        });

        // If the image changes we need to rebuild
        println!("cargo:rerun-if-changed={}", image_path.display());

        // Update the output with our included program
        writeln!(
            output,
            "    Program {{
        name: {:?},
        services: &[{}],
        requires: &[{}],
        image: include_bytes!({:?}),
    }},",
            manifest.name,
            manifest
                .services
                .iter()
                .map(|elem| format!("\"{}\"", elem))
                .collect::<Vec<_>>()
                .join(","),
            manifest
                .requires
                .iter()
                .map(|elem| format!("\"{}\"", elem))
                .collect::<Vec<_>>()
                .join(","),
            image_path,
        )
        .unwrap();

        // Add to services set to ensure we meet all services req at comp time
        required.extend(manifest.requires);
        provided.extend(
            manifest
                .services
                .iter()
                .map(|service| format!("{}::{}", manifest.name, service)),
        );
    }

    // Check that we have all requried services
    if !provided.is_superset(&required) {
        panic!(
            "\x1b[93mmissing required services `{:?}`: these were not provided by any referenced daedalus programs\n\nprovided services: {:?}\x1b[0m",
            required.difference(&provided),
            provided
        )
    }

    // Write to the programs.rs file in OUT_DIR our grabbed program manifests
    output.push_str("];\n");
    fs::write(&out_file, output).unwrap();
}
