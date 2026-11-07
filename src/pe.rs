use anyhow::{Context, Result, anyhow};
use goblin::pe::export::ExportAddressTableEntry;
use goblin::pe::import::SyntheticImportLookupTableEntry;
use goblin::pe::options::ParseOptions;
use goblin::pe::section_table::SectionTable;
use goblin::pe::utils;
use indexmap::IndexMap;

use crate::types::{DllExports, DllImports, Export, ForwardTarget, Import, PeInput};

fn read_str_at_rva<'a>(
    bytes: &'a [u8],
    sections: &[SectionTable],
    file_alignment: u32,
    rva: usize,
) -> Option<&'a str> {
    let offset = utils::find_offset(rva, sections, file_alignment, &ParseOptions::default())?;
    let len = bytes[offset..].iter().position(|&b| b == 0)?;
    std::str::from_utf8(&bytes[offset..offset + len]).ok()
}

/// Forwarder strings look like "NTDLL.RtlAllocateHeap" or "KERNELBASE.#42";
/// the module part omits the extension and the loader appends ".dll".
fn parse_forwarder(s: &str) -> Option<ForwardTarget> {
    let (dll, func) = s.rsplit_once('.')?;
    if dll.is_empty() || func.is_empty() {
        return None;
    }
    let import = if let Some(ord) = func.strip_prefix('#') {
        Import::ByOrdinal(ord.parse().ok()?)
    } else {
        Import::ByName(func.to_owned())
    };
    let dll = if dll.to_ascii_lowercase().ends_with(".dll") {
        dll.to_owned()
    } else {
        format!("{dll}.dll")
    };
    Some(ForwardTarget { dll: dll.into(), import })
}

pub fn parse_exports(name: String, bytes: &[u8]) -> Result<DllExports> {
    let pe = goblin::pe::PE::parse(bytes)
        .with_context(|| format!("failed to parse PE file '{name}'"))?;

    let export_data = pe
        .export_data
        .ok_or_else(|| anyhow!("no export directory in '{name}'"))?;

    let file_alignment = pe
        .header
        .optional_header
        .map(|oh| oh.windows_fields.file_alignment)
        .unwrap_or(0x200);

    let base = export_data.export_directory_table.ordinal_base as u16;
    let mut exports: IndexMap<u16, Export> = IndexMap::new();

    for (j, entry) in export_data.export_address_table.iter().enumerate() {
        let ordinal = base + j as u16;
        match entry {
            ExportAddressTableEntry::ExportRVA(0) => {}
            ExportAddressTableEntry::ExportRVA(_) => {
                exports.insert(ordinal, Export::default());
            }
            ExportAddressTableEntry::ForwarderRVA(rva) => {
                let forward = read_str_at_rva(bytes, &pe.sections, file_alignment, *rva as usize)
                    .and_then(parse_forwarder);
                exports.insert(ordinal, Export { name: None, forward });
            }
        }
    }

    let mut exports_by_name: IndexMap<String, u16> = IndexMap::new();

    for (&name_rva, &addr_table_idx) in export_data
        .export_name_pointer_table
        .iter()
        .zip(export_data.export_ordinal_table.iter())
    {
        let ordinal = base + addr_table_idx;
        let Some(export_name) = read_str_at_rva(bytes, &pe.sections, file_alignment, name_rva as usize)
        else {
            continue;
        };
        let export_name = export_name.to_owned();
        exports.entry(ordinal).or_default().name = Some(export_name.clone());
        exports_by_name.insert(export_name, ordinal);
    }

    let subsystem_version = pe.header.optional_header.map(|oh| (
        oh.windows_fields.major_subsystem_version,
        oh.windows_fields.minor_subsystem_version,
    ));
    let is_dll = pe.header.coff_header.characteristics & 0x2000 != 0;

    Ok(DllExports {
        name: name.into(),
        exports,
        exports_by_name,
        subsystem_version,
        is_dll,
    })
}

pub fn parse_pe_input(name: &str, bytes: &[u8]) -> Result<PeInput> {
    let pe = goblin::pe::PE::parse(bytes)
        .with_context(|| format!("failed to parse PE file '{name}'"))?;

    let oh = pe
        .header
        .optional_header
        .with_context(|| format!("no optional header in '{name}'"))?;

    let subsystem_version = (
        oh.windows_fields.major_subsystem_version,
        oh.windows_fields.minor_subsystem_version,
    );

    let mut imports = Vec::new();

    if let Some(import_data) = &pe.import_data {
        for entry in &import_data.import_data {
            let mut import_list = Vec::new();

            if let Some(ref lookup_table) = entry.import_lookup_table {
                for item in lookup_table {
                    let import = match item {
                        SyntheticImportLookupTableEntry::OrdinalNumber(ord) => {
                            Import::ByOrdinal(*ord)
                        }
                        SyntheticImportLookupTableEntry::HintNameTableRVA((_, hint_entry)) => {
                            Import::ByName(hint_entry.name.to_owned())
                        }
                    };
                    import_list.push(import);
                }
            }

            imports.push(DllImports {
                dll_name: entry.name.into(),
                imports: import_list,
            });
        }
    }

    let is_dll = pe.header.coff_header.characteristics & 0x2000 != 0;

    Ok(PeInput {
        name: name.into(),
        imports,
        subsystem_version,
        is_dll,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_name(s: &str) -> Option<(String, String)> {
        parse_forwarder(s).map(|f| {
            let func = match f.import {
                Import::ByName(n) => n,
                Import::ByOrdinal(o) => format!("#{o}"),
            };
            (f.dll.to_string(), func)
        })
    }

    #[test]
    fn plain_forward() {
        assert_eq!(
            by_name("NTDLL.RtlAllocateHeap"),
            Some(("NTDLL.dll".into(), "RtlAllocateHeap".into()))
        );
    }

    #[test]
    fn api_set_forward_splits_at_last_dot() {
        assert_eq!(
            by_name("api-ms-win-core-heap-l1-1-0.HeapAlloc"),
            Some(("api-ms-win-core-heap-l1-1-0.dll".into(), "HeapAlloc".into()))
        );
    }

    #[test]
    fn ordinal_forward() {
        assert_eq!(by_name("KERNELBASE.#42"), Some(("KERNELBASE.dll".into(), "#42".into())));
    }

    #[test]
    fn explicit_dll_extension_not_doubled() {
        assert_eq!(by_name("foo.dll.Func"), Some(("foo.dll".into(), "Func".into())));
    }

    #[test]
    fn malformed_forwards_rejected() {
        assert!(parse_forwarder("NoDotHere").is_none());
        assert!(parse_forwarder(".Func").is_none());
        assert!(parse_forwarder("dll.").is_none());
        assert!(parse_forwarder("dll.#notanumber").is_none());
    }
}
