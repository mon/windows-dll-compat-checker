use case_insensitive_string::CaseInsensitiveString;
use indexmap::IndexMap;

#[derive(Clone, PartialEq, Debug)]
pub enum Import {
    ByName(String),
    ByOrdinal(u16),
}

/// Target of a forwarder export, e.g. kernel32's HeapAlloc -> NTDLL.RtlAllocateHeap
#[derive(Clone, PartialEq, Debug)]
pub struct ForwardTarget {
    pub dll: CaseInsensitiveString,
    pub import: Import,
}

#[derive(Clone, PartialEq, Default, Debug)]
pub struct Export {
    pub name: Option<String>,
    pub forward: Option<ForwardTarget>,
}

#[derive(Clone)]
pub struct DllExports {
    pub name: CaseInsensitiveString,
    pub exports: IndexMap<u16, Export>,
    pub exports_by_name: IndexMap<String, u16>,
    pub subsystem_version: Option<(u16, u16)>,
    pub is_dll: bool,
}

impl PartialEq for DllExports {
    fn eq(&self, other: &Self) -> bool {
        (&self.name, &self.exports) == (&other.name, &other.exports)
    }
}

pub struct DllImports {
    pub dll_name: CaseInsensitiveString,
    pub imports: Vec<Import>,
}

pub struct PeInput {
    pub name: CaseInsensitiveString,
    pub imports: Vec<DllImports>,
    pub subsystem_version: (u16, u16),
    pub is_dll: bool,
}
