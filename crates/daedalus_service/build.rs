use std::fmt::Write;
use std::{collections::HashSet, env, fs, path::PathBuf};

use lepton3::{parser, validator};
use serde::Deserialize;

// These are the builtin services that are handled by the daedalus core rather
// than a daedalus program.
const BUILTIN_SERVICES: [&str; 2] = ["memory::write_mem_32", "memory::read_mem_32"];

/// The parsed content of a Daedalus
/// program manifest.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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

/// The parsed content of a Daedalus
/// template which contains all of the
/// boot phases and their ordering
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Template {
    // The set of all phases
    #[serde(default)]
    pub phases: Vec<Phase>,

    // The entry phase to the bootloader
    pub entry: String,
}

/// A boot phase which sequentially
/// runs all the referenced program until
/// it succeeds (yield) before proceeding
/// to the next.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Phase {
    // The name of this phase for others to reference
    pub name: String,

    // The next phase to run after all these programs have succeeded
    pub next: String,

    // The program to run during this phase, by name
    pub program: String,
}

/// Writes out the image in a static format for a program
/// directly into a rust struct.
///
/// Will panic on any error associated with parsing, validating
/// or writing out the image.
///
/// This returns a struct literal constructor in a string form
/// that constructs the struct
fn write_out_image(name: &String, image_path: PathBuf, out: &mut String) -> String {
    // Read all the bytes from the image path
    let bytes = fs::read(&image_path).unwrap_or_else(|e| {
        panic!("\x1b[93merror reading {}: {e}\x1b[0m", image_path.display());
    });

    // Parse the image from the bytes.
    let image = parser::parse(&bytes).unwrap_or_else(|e| {
        panic!(
            "\x1b[93merror parsing image {}: {e}\x1b[0m",
            image_path.display()
        );
    });

    // Validate the file to ensure it's validity
    validator::validate(&image).unwrap_or_else(|e| {
        panic!(
            "\x1b[93merror validating image {}: {e}\x1b[0m",
            image_path.display()
        );
    });

    // The name of the produced struct
    let struct_name = format!("{}Image", name.to_uppercase());

    // Number of each elements in the debug section of the image
    let debug_files = image
        .debug_info
        .as_ref()
        .map(|debug_info| debug_info.files.len())
        .unwrap_or(0);
    let debug_locations = image
        .debug_info
        .as_ref()
        .map(|debug_info| debug_info.locations.len())
        .unwrap_or(0);

    // Number of elements in each table of the image
    let object_table_size = image.object_table.len();
    let function_table_size = image.function_table.len();
    let instructions_len = image.instructions.len();

    // Write out the struct for this image's image.
    writeln!(
        out,
        "
        pub struct {struct_name}DebugInfo {{
            pub files: [&'static str; {debug_files}],
            pub locations: [StaticSourceLocation; {debug_locations}]
        }}

        pub struct {struct_name} {{
            pub header: Header,
            pub object_table: [ObjectType; {object_table_size}],
            pub function_table: [Function; {function_table_size}],
            pub instructions: &'static [u8; {instructions_len}],
            pub debug_info: Option<{struct_name}DebugInfo>,
        }}
        
        /// We need this to implement the LeptonImage trait for
        /// it to be a valid lepton3 image that we can use in the VM
        impl LeptonImage<StaticSourceLocation> for {struct_name} {{
            type File = &'static str;

            fn header(&self) -> &Header {{
                &self.header
            }}

            fn object_table(&self) -> &[ObjectType] {{
                &self.object_table
            }}

            fn function_table(&self) -> &[Function] {{
                &self.function_table
            }}

            fn instructions(&self) -> &[u8] {{
                self.instructions
            }}

            fn files(&self) -> Option<&[Self::File]> {{
                self.debug_info.as_ref().map(|debug_info| &debug_info.files[..])
            }}

            fn locations(&self) -> Option<&[StaticSourceLocation]> {{
                self.debug_info.as_ref().map(|debug_info| &debug_info.locations[..])
            }}
        }}

        impl StaticLeptonImage for {struct_name} {{}}
        ",
    )
    .unwrap();

    // Write out the object table as a array literal
    let mut object_table_literal = String::from("[");
    for object in image.object_table {
        // Write out the object literal.
        writeln!(
            object_table_literal,
            "
            ObjectType {{
                field_count: {}
            }},",
            object.field_count
        )
        .unwrap();
    }
    object_table_literal.push_str("]");

    // Write out the function table as an array literal
    let mut function_table_literal = String::from("[");
    for function in image.function_table {
        // Write out the function literal.
        writeln!(
            function_table_literal,
            "
            Function {{
                arg_count: {},
                local_count: {},
                instruction_offset: {},
                instruction_length: {},
            }},",
            function.arg_count,
            function.local_count,
            function.instruction_offset,
            function.instruction_length
        )
        .unwrap();
    }
    function_table_literal.push_str("]");

    // We re-write out the instructions so we don't need to manually write out each u8 byte of the instructions.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let instructions = out_dir.join(format!("image_{struct_name}"));

    fs::write(&instructions, image.instructions.as_slice()).unwrap();

    // Write out the debug info literal as part of this construction
    let mut debug_info_literal = String::from("None,");
    if let Some(debug_info) = image.debug_info {
        // Write out the file table.
        let mut file_table_literal = String::from("[");
        for file in debug_info.files {
            // Write out the file literal.
            writeln!(file_table_literal, "{:?},", file).unwrap();
        }
        file_table_literal.push_str("]");

        // Write out the source locations table.
        let mut source_locations_literal = String::from("[");
        for location in debug_info.locations {
            // Write out the source location literal.
            writeln!(
                source_locations_literal,
                "StaticSourceLocation {{
                    instruction_offset: {},
                    file: {},
                    line: {},
                    column: {},
                    context: {:?},
                }},",
                location.instruction_offset,
                location.file,
                location.line,
                location.column,
                location.context
            )
            .unwrap();
        }
        source_locations_literal.push_str("]");

        debug_info_literal = format!(
            "
            Some(
                {struct_name}DebugInfo {{
                    files: {file_table_literal},
                    locations: {source_locations_literal}
                }}
            )
        "
        )
    };

    // Return the constructor literal
    format!(
        "
        {struct_name} {{
            header: Header {{
                version_major: {},
                flags: ImageFlags::from_raw({}),
                entry_point: {},
            }},
            object_table: {object_table_literal},
            function_table: {function_table_literal},
            instructions: include_bytes!({instructions:?}),
            debug_info: {debug_info_literal},
        }}",
        image.header.version_major,
        image.header.flags.to_raw(),
        image.header.entry_point
    )
}

fn main() {
    // Daedalus programs if the environment variable is changed needs to rerun
    println!("cargo:rerun-if-env-changed=DAEDALUS_PROGRAMS");
    println!("cargo:rerun-if-env-changed=DAEDALUS_TEMPLATE");

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

    // The template file, this defines the order of program execution
    let template_file = env::var("DAEDALUS_TEMPLATE").unwrap_or_default();

    // No entries??? We die because likely this is user error.
    if entries.is_empty() {
        panic!(
            "\x1b[93mDAEDALUS_PROGRAMS is empty, embedding no programs is not permitted. Exiting.\x1b[0m"
        );
    }

    // No template, there's no structure on how to run the programs!
    if template_file.is_empty() {
        panic!(
            "\x1b[93mDAEDALUS_TEMPLATE is empty, running no programs is not permitted. Exiting.\x1b[0m"
        );
    }

    // The seen set of names, a duplicate program name is not permitted
    let mut seen = HashSet::new();

    // Names with insertion order preserved.
    let mut ordered_names = Vec::new();

    // All referenced services, services provided must be a superset of required
    let mut required = HashSet::new();
    let mut provided = HashSet::with_capacity(BUILTIN_SERVICES.len());
    provided.extend(BUILTIN_SERVICES.map(String::from));

    // This is the buf for the static image structs
    let mut image_structs_out = String::new();

    // The generated output will be the set of all programs described
    let mut output = String::from("pub static PROGRAMS: &[Program<impl StaticLeptonImage>] = &[\n");

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
        ordered_names.push(manifest.name.clone());

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

        // Write out the image struct
        let image_literal = write_out_image(&manifest.name, image_path, &mut image_structs_out);

        // Update the output with our included program
        writeln!(
            output,
            "    Program {{
        name: {:?},
        services: &[{}],
        requires: &[{}],
        image: &{},
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
            image_literal
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

    output.push_str("];\n");
    output.extend(image_structs_out.chars());

    // Build match arms for the const lookup of programs
    let mut arms = String::new();

    // The same order we pushed the programs, we need to retrieve with the match
    // for const support in our generated code
    for (iter, entry) in ordered_names.iter().enumerate() {
        arms.push_str(&format!("{:?} => Some(&PROGRAMS[{}]),", entry, iter));
    }

    // Generate the get match statement for the programs
    output.push_str(&format!(
        "
        /// Looks up an embedded program by name
        pub const fn get(name: &str) -> Option<&'static Program<impl StaticLeptonImage>> {{
            match name {{
                {}
                _ => None,
            }}
        }}
    ",
        arms
    ));

    // Grab the template to make the actual program phases
    let template: Template = toml::from_str(
        &fs::read_to_string(&template_file)
            .unwrap_or_else(|e| panic!("\x1b[93merror reading {}: {e}\x1b[0m", template_file)),
    )
    .unwrap_or_else(|e| panic!("\x1b[93merror parsing {}: {e}\x1b[0m", template_file));

    println!("cargo:rerun-if-env-changed={}", template_file);

    // The template will output a vec of `Phase`'s.

    // Duplicate phase names are not permitted
    let mut seen_phases = HashSet::new();
    let mut ordered_phases: Vec<String> = Vec::new();

    // All referenced programs, programs provided must all exist
    let mut required_programs = HashSet::new();

    // All required phases for proper running
    let mut required_phases = HashSet::new();

    // The generated output will be the set of all phases described
    output.push_str("pub static PHASES: &[Phase] = &[\n");

    for phase in template.phases {
        // If we have a duplicate name this isn't allowed as those phases would conflict
        if !seen_phases.insert(phase.name.clone()) {
            panic!(
                "\x1b[93mduplicate daedalus phase name `{}`\x1b[0m",
                phase.name
            );
        }
        ordered_phases.push(phase.name.clone());

        // Update the output with our included phase
        writeln!(
            output,
            "    Phase {{
        name: {:?},
        program: &get({:?}).unwrap(),
        next: {:?},
    }},",
            phase.name, phase.program, phase.next
        )
        .unwrap();

        // This requires next phase to exist in our set of phases
        required_phases.insert(phase.next);

        // This requires this program to exist (else doesn't make sense!)
        required_programs.insert(phase.program);
    }

    output.push_str("];\n");

    // The entry must exist in the set of phases
    if !seen_phases.contains(&template.entry) {
        panic!("\x1b[93mmissing entry phase `{:?}`\x1b[0m", template.entry)
    }

    // Write the entry out as a reference to the programs
    output.push_str(&format!(
        "pub static ENTRY: &'static Phase = &PHASES[{}];",
        ordered_phases
            .iter()
            .enumerate()
            .find(|phase| phase.1.as_str() == template.entry)
            .unwrap()
            .0
    ));

    fs::write(&out_file, output).unwrap();
}
