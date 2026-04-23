use anyhow::{Context, Result};
use indexmap::IndexMap;
use ini::Ini;
use rust_embed::Embed;
use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::Path;

use crate::types::DllExports;

#[derive(Embed)]
#[folder = "premade_ini/"]
#[prefix = "PREMADE/"]
#[include = "*.ini"]
pub struct EmbeddedInis;

fn encode_unnamed_ranges(ordinals: &mut Vec<u16>) -> String {
    ordinals.sort_unstable();
    let mut out = String::new();
    let mut i = 0;
    while i < ordinals.len() {
        let start = ordinals[i];
        let mut end = start;
        while i + 1 < ordinals.len() && ordinals[i + 1] == end + 1 {
            i += 1;
            end = ordinals[i];
        }
        if !out.is_empty() {
            out.push(',');
        }
        if start == end {
            out.push_str(&start.to_string());
        } else {
            out.push_str(&format!("{}-{}", start, end));
        }
        i += 1;
    }
    out
}

fn decode_unnamed_ranges(s: &str, path: &Path, dll_name: &str) -> Result<Vec<u16>> {
    let mut ordinals = Vec::new();
    for part in s.split(',') {
        if let Some((a, b)) = part.split_once('-') {
            let start: u16 = a.parse().with_context(|| {
                format!(
                    "bad range '{}' in __unnamed in '[{}]' of '{}'",
                    part,
                    dll_name,
                    path.display()
                )
            })?;
            let end: u16 = b.parse().with_context(|| {
                format!(
                    "bad range '{}' in __unnamed in '[{}]' of '{}'",
                    part,
                    dll_name,
                    path.display()
                )
            })?;
            ordinals.extend(start..=end);
        } else {
            let ord: u16 = part.parse().with_context(|| {
                format!(
                    "bad ordinal '{}' in __unnamed in '[{}]' of '{}'",
                    part,
                    dll_name,
                    path.display()
                )
            })?;
            ordinals.push(ord);
        }
    }
    Ok(ordinals)
}

pub fn read_ini(path: &Path) -> Result<(Vec<DllExports>, Option<(u16, u16)>)> {
    // rust_embed keys always use forward slashes.
    let embed_key = path.to_string_lossy().replace('\\', "/");
    let try_ini = match EmbeddedInis::get(&embed_key) {
        Some(ini) => Ini::read_from(&mut Cursor::new(&ini.data)),
        None => Ini::load_from_file(path),
    };
    let ini = try_ini
        .with_context(|| format!("failed to load INI file '{}'", path.display()))?;

    let mut result = Vec::new();
    let mut extend_files: Vec<String> = Vec::new();
    let mut meta_subsystem_version: Option<(u16, u16)> = None;

    for (section_name, props) in ini.iter() {
        let Some(dll_name) = section_name else {
            continue;
        };

        if dll_name == "META" {
            for (key, value) in props.iter() {
                if key == "extend" {
                    extend_files.push(value.to_owned());
                } else if key == "max_subsystem_version" {
                    if let Some((maj_str, min_str)) = value.split_once('.') {
                        if let (Ok(major), Ok(minor)) = (maj_str.parse::<u16>(), min_str.parse::<u16>()) {
                            meta_subsystem_version = Some((major, minor));
                        }
                    }
                }
            }
            continue;
        }

        let mut exports: IndexMap<u16, Option<String>> = IndexMap::new();
        let mut exports_by_name: IndexMap<String, u16> = IndexMap::new();

        for (key, value) in props.iter() {
            if key == "__unnamed" {
                for ordinal in decode_unnamed_ranges(value, path, dll_name)? {
                    exports.insert(ordinal, None);
                }
                continue;
            }
            let ordinal: u16 = key.parse().with_context(|| {
                format!(
                    "invalid ordinal '{}' in section '[{}]' of '{}'",
                    key,
                    dll_name,
                    path.display()
                )
            })?;
            let export_name = if value.is_empty() {
                None
            } else {
                Some(value.to_owned())
            };
            if let Some(ref n) = export_name {
                exports_by_name.insert(n.clone().into(), ordinal);
            }
            exports.insert(ordinal, export_name);
        }

        result.push(DllExports {
            name: dll_name.into(),
            exports,
            exports_by_name,
            subsystem_version: None,
            is_dll: true,
        });
    }

    let parent = path.parent().unwrap_or(Path::new("."));
    let mut extended: Vec<DllExports> = Vec::new();
    for filename in &extend_files {
        let (ext_dlls, ext_version) = read_ini(&parent.join(filename))?;
        extended.extend(ext_dlls);
        if let Some(v) = ext_version {
            meta_subsystem_version = Some(meta_subsystem_version.map_or(v, |mv| mv.max(v)));
        }
    }
    extended.extend(result);

    Ok((extended, meta_subsystem_version))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn roundtrip(mut input: Vec<u16>) -> Vec<u16> {
        let encoded = encode_unnamed_ranges(&mut input);
        let decoded = decode_unnamed_ranges(&encoded, Path::new("test.ini"), "test.dll").unwrap();
        let mut sorted = decoded;
        sorted.sort_unstable();
        sorted
    }

    fn sorted(mut v: Vec<u16>) -> Vec<u16> {
        v.sort_unstable();
        v
    }

    #[test]
    fn single_ordinal() {
        assert_eq!(roundtrip(vec![5]), vec![5]);
    }

    #[test]
    fn two_consecutive_encodes_as_range() {
        let mut v = vec![4, 5];
        let encoded = encode_unnamed_ranges(&mut v);
        assert_eq!(encoded, "4-5");
        assert_eq!(roundtrip(vec![4, 5]), vec![4, 5]);
    }

    #[test]
    fn contiguous_range() {
        assert_eq!(roundtrip(vec![1, 2, 3]), vec![1, 2, 3]);
    }

    #[test]
    fn non_contiguous_singles() {
        assert_eq!(roundtrip(vec![1, 3]), vec![1, 3]);
    }

    #[test]
    fn mixed_ranges_and_singles() {
        // 1-3, gap at 4, 5-6, gap, 8
        let input = vec![1, 2, 3, 5, 6, 8];
        let mut v = input.clone();
        let encoded = encode_unnamed_ranges(&mut v);
        assert_eq!(encoded, "1-3,5-6,8");
        assert_eq!(roundtrip(input), vec![1, 2, 3, 5, 6, 8]);
    }

    #[test]
    fn unsorted_input_is_handled() {
        assert_eq!(roundtrip(vec![3, 1, 2]), sorted(vec![1, 2, 3]));
    }

    #[test]
    fn single_element_does_not_encode_as_range() {
        let mut v = vec![7];
        let encoded = encode_unnamed_ranges(&mut v);
        assert_eq!(encoded, "7");
    }

    #[test]
    fn boundary_ordinals() {
        assert_eq!(roundtrip(vec![0, 1, 65534, 65535]), vec![0, 1, 65534, 65535]);
    }

    #[test]
    fn gap_of_two_is_not_merged() {
        let mut v = vec![1, 3];
        let encoded = encode_unnamed_ranges(&mut v);
        assert_eq!(encoded, "1,3");
    }
}

fn populate_dll_sections(ini: &mut Ini, dll_exports: &[DllExports]) {
    let mut sorted: Vec<&DllExports> = dll_exports.iter().collect();
    sorted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    for entry in sorted {
        let mut section = ini.with_section(Some(&entry.name));
        let mut unnamed: Vec<u16> = entry
            .exports
            .iter()
            .filter_map(|(&ord, name)| name.is_none().then_some(ord))
            .collect();
        if !unnamed.is_empty() {
            section.set("__unnamed", encode_unnamed_ranges(&mut unnamed));
        }
        for (ordinal, name) in &entry.exports {
            if let Some(n) = name {
                section.set(ordinal.to_string(), n.as_str());
            }
        }
    }
}

fn flush_ini(ini: Ini, path: &Path) -> Result<()> {
    // write_to_file in rust-ini uses an unbuffered File, causing one syscall per
    // key-value pair. Wrapping in BufWriter reduces this to a handful of syscalls.
    let ctx = || format!("failed to write INI file '{}'", path.display());
    let file = File::create(path).with_context(ctx)?;
    ini.write_to(&mut BufWriter::new(file)).with_context(ctx)?;
    Ok(())
}

fn set_meta_subsystem_version(ini: &mut Ini, dll_exports: &[DllExports], explicit: Option<(u16, u16)>) {
    let max_version = dll_exports.iter()
        .filter(|e| !e.is_dll)
        .filter_map(|e| e.subsystem_version)
        .max()
        .or(explicit);
    if let Some((major, minor)) = max_version {
        ini.with_section(Some("META")).set("max_subsystem_version", format!("{major}.{minor}"));
    }
}

pub fn write_ini(dll_exports: &[DllExports], version: Option<(u16, u16)>, path: &Path) -> Result<()> {
    let mut ini = Ini::new();
    set_meta_subsystem_version(&mut ini, dll_exports, version);
    populate_dll_sections(&mut ini, dll_exports);
    flush_ini(ini, path)
}

pub fn write_ini_with_extend(dll_exports: &[DllExports], version: Option<(u16, u16)>, extend_file: &str, path: &Path) -> Result<()> {
    let mut ini = Ini::new();
    ini.with_section(Some("META")).set("extend", extend_file);
    set_meta_subsystem_version(&mut ini, dll_exports, version);
    populate_dll_sections(&mut ini, dll_exports);
    flush_ini(ini, path)
}
