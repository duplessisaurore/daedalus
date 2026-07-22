//! This build script reads daedalus programs and outputs them
//! under a programs.rs with the daedalus programs & daedalus manifest
//! generated as rust structs that can then be embedded into daedalus
//! itself through the `daedalus_program` crate.

use std::fmt::Write;
use std::{collections::HashSet, env, fs, path::PathBuf};

use lepton3::{parser, validator};
use serde::Deserialize;

/// The parsed content of a Daedalus
/// program manifest.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    // The name defined for this program in the manifest
    name: String,

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

/// Converts the name to it's Image struct name
pub fn to_image_struct_name(name: &str) -> String {
    format!("{}Image", name.to_uppercase())
}

/// Converts the name to it's PROGRAM const name
pub fn to_program_const_name(name: &str) -> String {
    format!("{}_PROGRAM", name.to_uppercase())
}

/// Converts the name to it's debug info struct name
pub fn to_debug_struct_name(name: &str) -> String {
    format!("{}DebugInfo", to_image_struct_name(name))
}

/// Converts the name to it's PHASE const name
pub fn to_phase_const_name(name: &str) -> String {
    format!("{}_PHASE", name.to_uppercase())
}

/// Writes out the image in a static format for a program
/// directly into a rust struct.
///
/// Will panic on any error associated with parsing, validating
/// or writing out the image.
///
/// This returns a struct literal constructor in a string form
/// that constructs the struct
fn write_out_image(name: &str, image_path: PathBuf, out: &mut String) -> String {
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
    let struct_name = to_image_struct_name(name);
    let debug_struct_name = to_debug_struct_name(name);

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
        pub struct {debug_struct_name} {{
            pub files: [&'static str; {debug_files}],
            pub locations: [StaticSourceLocation; {debug_locations}]
        }}

        pub struct {struct_name} {{
            pub header: Header,
            pub object_table: [ObjectType; {object_table_size}],
            pub function_table: [Function; {function_table_size}],
            pub instructions: &'static [u8; {instructions_len}],
            pub debug_info: Option<{debug_struct_name}>,
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
    object_table_literal.push(']');

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
    function_table_literal.push(']');

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
        file_table_literal.push(']');

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
        source_locations_literal.push(']');

        debug_info_literal = format!(
            "
            Some(
                {debug_struct_name} {{
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

    // This is the buf for the static image structs
    let mut image_structs_out = String::new();

    // The generated output will be the set of all programs described
    // with each program being some Program<StaticDaedalusImageVariants> as a const.
    let mut output = String::new();

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

        // An invalid name is one that isn't purely ascii isn't permitted.
        if !manifest.name.is_ascii() || manifest.name.is_empty() {
            panic!(
                "\x1b[93minvalid non-ascii or empty program name `{}`\x1b[0m",
                manifest.name
            );
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

        // Write out the image struct
        let image_literal = write_out_image(&manifest.name, image_path, &mut image_structs_out);

        // The name of the produced program & struct
        let program_name = to_program_const_name(&manifest.name);
        let struct_name = to_image_struct_name(&manifest.name);

        // Update the output with our included program
        writeln!(
            output,
            "static {program_name}: &'static Program<StaticDaedalusImageVariants> = &Program {{
        name: {:?},
        image: &StaticDaedalusImageVariants::{struct_name}(&{}),
        }};",
            manifest.name, image_literal
        )
        .unwrap();
    }

    output.push_str(&image_structs_out);

    // Build enum arms for the enum image of programs
    let mut enum_variants = String::new();
    for name in seen.iter() {
        enum_variants.push_str(&format!(
            "{}(&'static {}),",
            to_image_struct_name(name),
            to_image_struct_name(name)
        ));
    }

    // Build enum arms for the enum method calling of programs
    // for the LeptonImage trait
    let enum_method_builder = |method: &'static str| {
        let mut enum_match_arms = String::new();
        for name in seen.iter() {
            enum_match_arms.push_str(&format!(
                "StaticDaedalusImageVariants::{}(image_variant) => image_variant.{},",
                to_image_struct_name(name),
                method
            ));
        }

        enum_match_arms
    };

    output.push_str(&format!(
        "
        /// The enum variant of all images because they're all one
        /// constant static type
        pub enum StaticDaedalusImageVariants {{
            {enum_variants}  
        }}

        impl LeptonImage<StaticSourceLocation> for StaticDaedalusImageVariants {{
            type File = &'static str;

            fn header(&self) -> &Header {{
                match &self {{
                    {}
                }}
            }}

            fn object_table(&self) -> &[ObjectType] {{
                match &self {{
                    {}
                }}
            }}

            fn function_table(&self) -> &[Function] {{
                match &self {{
                    {}
                }}
            }}

            fn instructions(&self) -> &[u8] {{
                match &self {{
                    {}
                }}
            }}

            fn files(&self) -> Option<&[Self::File]> {{
                match &self {{
                    {}
                }}
            }}

            fn locations(&self) -> Option<&[StaticSourceLocation]> {{
                match &self {{
                    {}
                }}
            }}
        }}

        impl StaticLeptonImage for StaticDaedalusImageVariants {{}}
        ",
        enum_method_builder("header()"),
        enum_method_builder("object_table()"),
        enum_method_builder("function_table()"),
        enum_method_builder("instructions()"),
        enum_method_builder("files()"),
        enum_method_builder("locations()"),
    ));

    // Build match arms for the const lookup of programs
    let mut arms = String::new();
    for name in seen.iter() {
        arms.push_str(&format!(
            "{:?} => Some({}),",
            name,
            to_program_const_name(name)
        ));
    }

    // Generate the get match statement for the programs
    output.push_str(&format!(
        "
        /// Looks up an embedded program by name
        pub const fn get_program(name: &str) -> Option<&'static Program<StaticDaedalusImageVariants>> {{
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

    // Duplicate phase names are not permitted
    let mut seen_phases = HashSet::new();

    // All referenced programs, programs provided must all exist
    let mut required_programs = HashSet::new();

    // All required phases for proper running
    let mut required_phases = HashSet::new();

    for phase in template.phases {
        // If we have a duplicate name this isn't allowed as those phases would conflict
        if !seen_phases.insert(phase.name.clone()) {
            panic!(
                "\x1b[93mduplicate daedalus phase name `{}`\x1b[0m",
                phase.name
            );
        }

        // An invalid name is one that isn't purely ascii isn't permitted.
        if !phase.name.is_ascii() || phase.name.is_empty() {
            panic!(
                "\x1b[93minvalid non-ascii or empty daedalus phase name `{}`\x1b[0m",
                phase.name
            );
        }

        // The name of the produced phase const
        let phase_name = to_phase_const_name(&phase.name);
        let program_name = to_program_const_name(&phase.program);

        // Update the output with our included phase
        writeln!(
            output,
            "static {phase_name}: &'static Phase<StaticDaedalusImageVariants> = &Phase {{
        name: {:?},
        program: {program_name},
        next: {:?},
    }};",
            phase.name, phase.next
        )
        .unwrap();

        // This requires next phase to exist in our set of phases
        required_phases.insert(phase.next);

        // This requires this program to exist (else doesn't make sense!)
        required_programs.insert(phase.program);
    }

    output.push('\n');

    // The entry must exist in the set of phases
    if !seen_phases.contains(&template.entry) {
        panic!("\x1b[93mmissing entry phase `{:?}`\x1b[0m", template.entry)
    }

    // The programs must exist in the set of programs
    if !seen.is_superset(&required_programs) {
        panic!(
            "\x1b[93mmissing required program `{:?}`: these were not provided by any referenced daedalus programs\n\nprovided programs: {:?}\x1b[0m",
            required_programs.difference(&seen),
            seen
        )
    }

    // Build match arms for the const lookup of phases
    let mut arms = String::new();
    for name in seen_phases.iter() {
        arms.push_str(&format!(
            "{:?} => Some({}),",
            name,
            to_phase_const_name(name)
        ));
    }

    // Generate the get match statement for the phases
    output.push_str(&format!(
        "
        /// Looks up an embedded phase by name
        pub const fn get_phase(name: &str) -> Option<&'static Phase<StaticDaedalusImageVariants>> {{
            match name {{
                {}
                _ => None,
            }}
        }}
    ",
        arms
    ));

    // Write the entry out as a reference to the phases
    output.push_str(&format!(
        "pub const fn get_entry_phase() -> &'static Phase<StaticDaedalusImageVariants> {{ get_phase({:?}).unwrap() }}",
        template.entry
    ));

    fs::write(&out_file, output).unwrap();
}
