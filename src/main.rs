mod ini;
mod pe;
mod types;

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

use anyhow::{Context, Result};
use case_insensitive_string::CaseInsensitiveString;
use clap::{CommandFactory, FromArgMatches, Parser};
use indexmap::IndexMap;

use types::{DllExports, Import, PeInput};

#[derive(Parser)]
#[command(about = "Validates Windows PE imports against a set of known system DLLs")]
struct Cli {
    /// Input DLLs/EXEs or directories to validate
    #[arg(required = true)]
    inputs: Vec<PathBuf>,

    /// System DLL sources: DLL/EXE file, directory, or .ini file (repeatable)
    #[arg(short, long, value_name = "SOURCE", group = "a")]
    system: Vec<PathBuf>,

    /// Write exports of inputs to an INI file instead of checking them
    #[arg(long, value_name = "FILE", group = "a")]
    export_ini: Option<PathBuf>,

    /// Extract DLLs common to all input INI files into FILE, and write importing versions of each input
    #[arg(long, value_name = "FILE", group = "a")]
    merge_common: Option<PathBuf>,

    /// With --merge-common: overwrite input files in-place instead of writing _extend copies
    #[arg(short = 'w', long)]
    in_place: bool,

    /// Maximum allowed OS version as MAJOR,MINOR
    #[arg(long, value_name = "MAJOR,MINOR")]
    os_version: Option<OsVersionSpec>,

    /// Ignore files matching these names when scanning directories (case insensitive, repeatable)
    #[arg(short, long, value_name = "FILENAME")]
    ignore: Vec<String>,
}

#[derive(Clone)]
struct OsVersionSpec(u16, u16);

impl FromStr for OsVersionSpec {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let (major_str, minor_str) = s
            .split_once(',')
            .with_context(|| format!("expected MAJOR,MINOR but got '{s}'"))?;
        let major = major_str
            .parse::<u16>()
            .with_context(|| format!("invalid major version '{major_str}'"))?;
        let minor = minor_str
            .parse::<u16>()
            .with_context(|| format!("invalid minor version '{minor_str}'"))?;
        Ok(OsVersionSpec(major, minor))
    }
}

fn is_pe_extension(ext: Option<&OsStr>) -> bool {
    matches!(
        ext.and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).as_deref(),
        Some("dll" | "exe")
    )
}

fn expand_path_to_pe_files(path: &Path, ignore: &[String]) -> Result<Vec<PathBuf>> {
    if path.is_dir() {
        println!("Loading PE files in {path:?}");
        let entries = fs::read_dir(path)
            .with_context(|| format!("failed to read directory '{}'", path.display()))?;
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.with_context(|| {
                format!("failed to read directory entry in '{}'", path.display())
            })?;
            let p = entry.path();
            let ignored = p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| ignore.iter().any(|ig| n.eq_ignore_ascii_case(ig)));
            if p.is_file() && is_pe_extension(p.extension()) && !ignored {
                files.push(p);
            }
        }
        Ok(files)
    } else {
        Ok(vec![path.to_owned()])
    }
}

fn collect_system_sources(paths: &[PathBuf], ignore: &[String]) -> Result<(IndexMap<CaseInsensitiveString, DllExports>, Option<(u16, u16)>)> {
    let mut map: IndexMap<CaseInsensitiveString, DllExports> = IndexMap::new();
    let mut min_version: Option<(u16, u16)> = None;

    for path in paths {
        if path.extension().and_then(|e| e.to_str()).map(|s| s.eq_ignore_ascii_case("ini")).unwrap_or(false) {
            let (dlls, version) = crate::ini::read_ini(path)?;
            for entry in dlls {
                map.insert(entry.name.clone().into(), entry);
            }
            if let Some(v) = version {
                min_version = Some(min_version.map_or(v, |mv| mv.min(v)));
            }
        } else {
            for file_path in expand_path_to_pe_files(path, ignore)? {
                let name = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .with_context(|| format!("invalid file name '{}'", file_path.display()))?
                    .to_owned();
                let bytes = fs::read(&file_path)
                    .with_context(|| format!("failed to read '{}'", file_path.display()))?;
                let entry = match pe::parse_exports(name.clone(), &bytes) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("Warning: skipping '{name}': {e}");
                        continue;
                    }
                };
                println!("Exports: {} from {name}", entry.exports.len());
                map.insert(name.into(), entry);
            }
        }
    }

    Ok((map, min_version))
}

fn collect_inputs(paths: &[PathBuf], ignore: &[String]) -> Result<(Vec<PeInput>, Vec<DllExports>)> {
    let mut inputs = Vec::new();
    let mut exports = Vec::new();

    for path in paths {
        for file_path in expand_path_to_pe_files(path, ignore)? {
            let name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .with_context(|| format!("invalid file name '{}'", file_path.display()))?
                .to_owned();
            let bytes = fs::read(&file_path)
                .with_context(|| format!("failed to read '{}'", file_path.display()))?;
            let input = match pe::parse_pe_input(&name, &bytes) {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("Warning: skipping '{name}': {e}");
                    continue;
                }
            };
            let file_exports = pe::parse_exports(name.clone(), &bytes).ok();
            let export_count = file_exports.as_ref().map(|e| e.exports.len()).unwrap_or(0);
            println!("Imports: {}, Exports: {export_count} from {name}", input.imports.len());
            inputs.push(input);
            if let Some(e) = file_exports {
                exports.push(e);
            }
        }
    }

    Ok((inputs, exports))
}

fn premade_ini_help() -> String {
    let mut names: Vec<_> = ini::EmbeddedInis::iter().collect();
    names.sort();
    let list = names.join("\n  ");
    format!("Available embedded INI files:\n  {list}")
}

fn main() -> Result<()> {
    let matches = Cli::command().after_help(premade_ini_help()).get_matches();
    let cli = Cli::from_arg_matches(&matches)
        .map_err(|e| e.exit())
        .unwrap();

    if let Some(ref common_path) = cli.merge_common {
        let all_inputs: Vec<(PathBuf, Vec<DllExports>, Option<(u16, u16)>)> = cli.inputs.iter()
            .map(|p| crate::ini::read_ini(p).map(|(dlls, ver)| (p.clone(), dlls, ver)))
            .collect::<Result<_>>()?;

        let (_, first_dlls, _) = &all_inputs[0];
        let common: Vec<&DllExports> = first_dlls.iter().filter(|dll| {
            all_inputs[1..].iter().all(|(_, other_dlls, _)| {
                other_dlls.iter().any(|other| {
                    other.name == dll.name && other == *dll
                })
            })
        }).collect();

        println!("Found {} common DLLs", common.len());

        let common_filename = common_path
            .file_name()
            .and_then(|n| n.to_str())
            .with_context(|| format!("invalid file name '{}'", common_path.display()))?;

        let common_owned: Vec<DllExports> = common.iter().map(|d| (*d).clone()).collect();
        crate::ini::write_ini(&common_owned, None, common_path)?;

        let common_names: HashSet<&CaseInsensitiveString> = common
            .iter()
            .map(|d| &d.name)
            .collect();

        for (input_path, dlls, version) in &all_inputs {
            let non_common: Vec<DllExports> = dlls.iter()
                .filter(|d| !common_names.contains(&d.name))
                .cloned()
                .collect();

            let out_path = if cli.in_place {
                input_path.clone()
            } else {
                let stem = input_path.file_stem().and_then(|s| s.to_str())
                    .with_context(|| format!("invalid file name '{}'", input_path.display()))?;
                let ext = input_path.extension().and_then(|s| s.to_str()).unwrap_or("ini");
                input_path.with_file_name(format!("{stem}_extend.{ext}"))
            };

            crate::ini::write_ini_with_extend(&non_common, *version, common_filename, &out_path)?;
            println!("Wrote {}", out_path.display());
        }

        return Ok(());
    }

    let (mut system_map, ini_version) = collect_system_sources(&cli.system, &cli.ignore)?;
    let sys_exports: usize = system_map.values().map(|e| e.exports.len()).sum();

    let os_version = cli.os_version.clone().or_else(|| {
        ini_version.map(|(major, minor)| {
            println!("Note: using os_version {major}.{minor} from system INI");
            OsVersionSpec(major, minor)
        })
    });
    println!("Loaded {sys_exports} exports from {} system DLLs", system_map.len());

    let (inputs, input_exports) = collect_inputs(&cli.inputs, &cli.ignore)?;
    let in_imports: usize = inputs.iter().flat_map(|i| &i.imports).map(|d| d.imports.len()).sum();
    let in_exports: usize = input_exports.iter().map(|e| e.exports.len()).sum();
    println!("Loaded {in_imports} imports, {in_exports} exports from {} input DLLs", inputs.len());

    if let Some(ref ini_path) = cli.export_ini {
        crate::ini::write_ini(&input_exports, None, ini_path)?;
        return Ok(());
    }

    for exports in input_exports {
        system_map.entry(exports.name.clone().into()).or_insert(exports);
    }

    let mut errors: Vec<String> = Vec::new();

    for input in &inputs {
        if let Some(OsVersionSpec(max_major, max_minor)) = os_version.clone() {
            if !input.is_dll {
                let (major, minor) = input.subsystem_version;
                if (major, minor) > (max_major, max_minor) {
                    errors.push(format!(
                        "'{}': SubsystemVersion {}.{} exceeds maximum {}.{}",
                        input.name, major, minor, max_major, max_minor
                    ));
                }
            }
        }

        for dll_imports in &input.imports {
            match system_map.get(&dll_imports.dll_name) {
                None => {
                    errors.push(format!(
                        "{}: imports {} function(s) from {} but it was not found in system sources",
                        input.name, dll_imports.imports.len(), dll_imports.dll_name
                    ));
                    for import in &dll_imports.imports {
                        match import {
                            Import::ByName(name) => {
                                errors.push(format!("  {}", name));
                            }
                            Import::ByOrdinal(ord) => {
                                errors.push(format!("  ordinal {}", ord));
                            }
                        }
                    }
                }
                Some(sys) => {
                    for import in &dll_imports.imports {
                        match import {
                            Import::ByName(name) => {
                                if sys.exports_by_name.get(name.as_str()).is_none() {
                                    errors.push(format!(
                                        "'{}': import '{}' from '{}' not found",
                                        input.name, name, dll_imports.dll_name
                                    ));
                                }
                            }
                            Import::ByOrdinal(ord) => {
                                if sys.exports.get(ord).is_none() {
                                    errors.push(format!(
                                        "'{}': import ordinal {} from '{}' not found",
                                        input.name, ord, dll_imports.dll_name
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if errors.is_empty() {
        println!("\nAll OK!");
    } else {
        for error in &errors {
            eprintln!("{error}");
        }
        process::exit(1);
    }

    Ok(())
}
