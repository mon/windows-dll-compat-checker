use anyhow::{Context, Result, anyhow};
use goblin::pe::export::ExportAddressTableEntry;
use goblin::pe::import::SyntheticImportLookupTableEntry;
use goblin::pe::options::ParseOptions;
use goblin::pe::utils;
use indexmap::IndexMap;

use crate::types::{DllExports, DllImports, Import, PeInput};

pub fn parse_exports(name: String, bytes: &[u8]) -> Result<DllExports> {
    let pe = goblin::pe::PE::parse(bytes)
        .with_context(|| format!("failed to parse PE file '{name}'"))?;

    let export_data = pe
        .export_data
        .ok_or_else(|| anyhow!("no export directory in '{name}'"))?;

    let base = export_data.export_directory_table.ordinal_base as u16;
    let mut exports: IndexMap<u16, Option<String>> = IndexMap::new();

    for (j, entry) in export_data.export_address_table.iter().enumerate() {
        let valid = !matches!(entry, ExportAddressTableEntry::ExportRVA(0));
        if valid {
            exports.insert(base + j as u16, None);
        }
    }

    let file_alignment = pe
        .header
        .optional_header
        .map(|oh| oh.windows_fields.file_alignment)
        .unwrap_or(0x200);

    let mut exports_by_name: IndexMap<String, u16> = IndexMap::new();

    for (&name_rva, &addr_table_idx) in export_data
        .export_name_pointer_table
        .iter()
        .zip(export_data.export_ordinal_table.iter())
    {
        let ordinal = base + addr_table_idx;
        let Some(offset) =
            utils::find_offset(name_rva as usize, &pe.sections, file_alignment, &ParseOptions::default())
        else {
            continue;
        };
        let Some(len) = bytes[offset..].iter().position(|&b| b == 0) else {
            continue;
        };
        let Ok(export_name) = std::str::from_utf8(&bytes[offset..offset + len]) else {
            continue;
        };
        let export_name = export_name.to_owned();
        exports.insert(ordinal, Some(export_name.clone()));
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
