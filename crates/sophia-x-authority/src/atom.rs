use std::collections::BTreeMap;

use crate::XAtom;

pub const X_ATOM_MAX_NAME_LEN: usize = 256;
pub const X_ATOM_LAST_PREDEFINED: XAtom = 68;

pub const X_ATOM_PRIMARY: XAtom = 1;
pub const X_ATOM_ATOM: XAtom = 4;
pub const X_ATOM_STRING: XAtom = 31;
pub const X_ATOM_WM_NAME: XAtom = 39;
pub const X_ATOM_WM_CLASS: XAtom = 67;

pub const X_ATOM_NAME_PRIMARY: &str = "PRIMARY";
pub const X_ATOM_NAME_ATOM: &str = "ATOM";
pub const X_ATOM_NAME_STRING: &str = "STRING";
pub const X_ATOM_NAME_WM_NAME: &str = "WM_NAME";
pub const X_ATOM_NAME_WM_CLASS: &str = "WM_CLASS";
pub const X_ATOM_NAME_NET_WM_NAME: &str = "_NET_WM_NAME";
pub const X_ATOM_NAME_WM_PROTOCOLS: &str = "WM_PROTOCOLS";
pub const X_ATOM_NAME_UTF8_STRING: &str = "UTF8_STRING";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XAtomError {
    EmptyName,
    NameTooLong { len: usize, max: usize },
    InvalidName,
    AtomSpaceExhausted,
}

impl core::fmt::Display for XAtomError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for XAtomError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XAtomTable {
    names_by_atom: BTreeMap<XAtom, String>,
    atoms_by_name: BTreeMap<String, XAtom>,
    next_dynamic: XAtom,
}

impl Default for XAtomTable {
    fn default() -> Self {
        Self::new()
    }
}

impl XAtomTable {
    pub fn new() -> Self {
        let mut table = Self {
            names_by_atom: BTreeMap::new(),
            atoms_by_name: BTreeMap::new(),
            next_dynamic: X_ATOM_LAST_PREDEFINED + 1,
        };
        table.insert_predefined(X_ATOM_PRIMARY, X_ATOM_NAME_PRIMARY);
        table.insert_predefined(X_ATOM_ATOM, X_ATOM_NAME_ATOM);
        table.insert_predefined(X_ATOM_STRING, X_ATOM_NAME_STRING);
        table.insert_predefined(X_ATOM_WM_NAME, X_ATOM_NAME_WM_NAME);
        table.insert_predefined(X_ATOM_WM_CLASS, X_ATOM_NAME_WM_CLASS);
        table
    }

    pub fn intern(
        &mut self,
        name: impl AsRef<str>,
        only_if_exists: bool,
    ) -> Result<Option<XAtom>, XAtomError> {
        let name = validated_atom_name(name.as_ref())?;
        if let Some(atom) = self.atoms_by_name.get(name) {
            return Ok(Some(*atom));
        }
        if only_if_exists {
            return Ok(None);
        }

        let atom = self.next_dynamic;
        self.next_dynamic = self
            .next_dynamic
            .checked_add(1)
            .ok_or(XAtomError::AtomSpaceExhausted)?;
        self.names_by_atom.insert(atom, name.to_owned());
        self.atoms_by_name.insert(name.to_owned(), atom);
        Ok(Some(atom))
    }

    pub fn name(&self, atom: XAtom) -> Option<&str> {
        self.names_by_atom.get(&atom).map(String::as_str)
    }

    pub fn atom(&self, name: &str) -> Option<XAtom> {
        self.atoms_by_name.get(name).copied()
    }

    pub fn is_metadata_candidate_atom(&self, atom: XAtom) -> bool {
        self.name(atom).is_some_and(is_metadata_candidate_name)
    }

    fn insert_predefined(&mut self, atom: XAtom, name: &str) {
        self.names_by_atom.insert(atom, name.to_owned());
        self.atoms_by_name.insert(name.to_owned(), atom);
    }
}

pub fn is_metadata_candidate_name(name: &str) -> bool {
    matches!(
        name,
        X_ATOM_NAME_WM_CLASS
            | X_ATOM_NAME_WM_NAME
            | X_ATOM_NAME_NET_WM_NAME
            | X_ATOM_NAME_WM_PROTOCOLS
    )
}

fn validated_atom_name(name: &str) -> Result<&str, XAtomError> {
    if name.is_empty() {
        return Err(XAtomError::EmptyName);
    }
    if name.len() > X_ATOM_MAX_NAME_LEN {
        return Err(XAtomError::NameTooLong {
            len: name.len(),
            max: X_ATOM_MAX_NAME_LEN,
        });
    }
    if !name.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(XAtomError::InvalidName);
    }
    Ok(name)
}
