//! Opaque handles to a program's files, nodes, types, symbols and
//! signatures.
//!
//! A handle is a small copyable value that names something inside one
//! [`crate::api::Program`]; it carries that program's identity, so handing it
//! to another program is an error rather than a wrong answer. Nothing about
//! how the checker stores the thing a handle names is part of the contract.

use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_PROGRAM: AtomicU32 = AtomicU32::new(1);

pub(crate) fn next_program_identity() -> u32 {
    NEXT_PROGRAM.fetch_add(1, Ordering::Relaxed)
}

/// A source file of one program.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SourceFileId {
    pub(crate) program: u32,
    pub(crate) index: u32,
}

/// A syntax node of one program's source file. Stable for the program's
/// lifetime: the same node always has the same id.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId {
    pub(crate) program: u32,
    pub(crate) file: u32,
    pub(crate) node: u32,
}

/// A type one program's checker produced.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TypeId {
    pub(crate) program: u32,
    pub(crate) index: u32,
}

/// A symbol of one program.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SymbolId {
    pub(crate) program: u32,
    pub(crate) index: u32,
}

/// A call or construct signature of one program.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SignatureId {
    pub(crate) program: u32,
    pub(crate) index: u32,
}

impl SourceFileId {
    /// The file's position in [`crate::api::Program::source_files`].
    pub fn index(self) -> u32 {
        self.index
    }
}

impl NodeId {
    pub fn source_file(self) -> SourceFileId {
        SourceFileId { program: self.program, index: self.file }
    }

    /// The node's index within its file, stable for the program's lifetime.
    pub fn index(self) -> u32 {
        self.node
    }
}

impl TypeId {
    pub fn index(self) -> u32 {
        self.index
    }
}

impl SymbolId {
    pub fn index(self) -> u32 {
        self.index
    }
}

impl SignatureId {
    pub fn index(self) -> u32 {
        self.index
    }
}
