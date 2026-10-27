use std::fmt;

use super::{DataFormatConfiguration, ReconstructibleTypeData};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardDeclaration {
    pub index: pdb::TypeIndex,
    pub kind: ForwardDeclarationKind,
    pub name: String,
}

impl ReconstructibleTypeData for ForwardDeclaration {
    fn reconstruct(
        &self,
        fmt_configuration: &DataFormatConfiguration,
        f: &mut impl std::fmt::Write,
    ) -> fmt::Result {
        if fmt_configuration.c_compatible {
            return writeln!(
                f,
                "typedef {} {} {};",
                if self.kind == ForwardDeclarationKind::Union {
                    "union"
                } else {
                    "struct"
                },
                self.name,
                self.name
            );
        }
        writeln!(
            f,
            "{} {};",
            match self.kind {
                ForwardDeclarationKind::Class => "class",
                ForwardDeclarationKind::Struct => "struct",
                ForwardDeclarationKind::Union => "union",
                ForwardDeclarationKind::Interface => "interface",
            },
            self.name
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardDeclarationKind {
    Class,
    Struct,
    Union,
    Interface,
}

impl ForwardDeclarationKind {
    pub fn from_class_kind(class_kind: pdb::ClassKind) -> Self {
        match class_kind {
            pdb::ClassKind::Class => Self::Class,
            pdb::ClassKind::Struct => Self::Struct,
            pdb::ClassKind::Interface => Self::Interface,
        }
    }
}
