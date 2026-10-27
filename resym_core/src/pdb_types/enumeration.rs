use std::fmt;

use super::{DataFormatConfiguration, NeededTypeSet, ReconstructibleTypeData};
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum<'p> {
    pub index: pdb::TypeIndex,
    pub name: String,
    pub underlying_type_name: String,
    pub unsigned_width: Option<u8>,
    pub values: Vec<EnumValue<'p>>,
}

impl<'p> Enum<'p> {
    pub fn unsigned_width(finder: &pdb::TypeFinder, index: pdb::TypeIndex) -> Result<Option<u8>> {
        Ok(match finder.find(index)?.parse()? {
            pdb::TypeData::Primitive(data) => match data.kind {
                pdb::PrimitiveKind::UChar | pdb::PrimitiveKind::U8 => Some(8),
                pdb::PrimitiveKind::UShort | pdb::PrimitiveKind::U16 => Some(16),
                pdb::PrimitiveKind::ULong | pdb::PrimitiveKind::U32 => Some(32),
                pdb::PrimitiveKind::UQuad | pdb::PrimitiveKind::U64 => Some(64),
                _ => None,
            },
            _ => None,
        })
    }
    pub fn add_fields(
        &mut self,
        type_finder: &pdb::TypeFinder<'p>,
        type_index: pdb::TypeIndex,
        needed_types: &mut NeededTypeSet,
    ) -> Result<()> {
        match type_finder.find(type_index)?.parse()? {
            pdb::TypeData::FieldList(data) => {
                for field in &data.fields {
                    self.add_field(type_finder, field, needed_types);
                }

                if let Some(continuation) = data.continuation {
                    // recurse
                    self.add_fields(type_finder, continuation, needed_types)?;
                }
            }
            other => {
                log::warn!(
                    "trying to Enum::add_fields() got {} -> {:?}",
                    type_index,
                    other
                );
            }
        }

        Ok(())
    }

    fn add_field(
        &mut self,
        _: &pdb::TypeFinder<'p>,
        field: &pdb::TypeData<'p>,
        _: &mut NeededTypeSet,
    ) {
        // ignore everything else even though that's sad
        if let pdb::TypeData::Enumerate(ref data) = field {
            // pdb 0.8 decodes signed LF_CHAR as U8. Small positive numeric
            // leaves are U16, so this conversion is unambiguous here.
            let value = match data.value {
                pdb::Variant::U8(value) => pdb::Variant::I8(value as i8),
                other => other,
            };
            let signed = match value {
                pdb::Variant::I8(v) => Some(i64::from(v)),
                pdb::Variant::I16(v) => Some(i64::from(v)),
                pdb::Variant::I32(v) => Some(i64::from(v)),
                pdb::Variant::I64(v) => Some(v),
                _ => None,
            };
            let value = match (self.unsigned_width, signed) {
                (Some(8), Some(v)) => pdb::Variant::U8(v as u8),
                (Some(16), Some(v)) => pdb::Variant::U16(v as u16),
                (Some(32), Some(v)) => pdb::Variant::U32(v as u32),
                (Some(64), Some(v)) => pdb::Variant::U64(v as u64),
                _ => value,
            };
            self.values.push(EnumValue {
                name: data.name,
                value,
            });
        }
    }
}

impl ReconstructibleTypeData for Enum<'_> {
    fn reconstruct(
        &self,
        fmt_configuration: &DataFormatConfiguration,
        f: &mut impl std::fmt::Write,
    ) -> fmt::Result {
        if fmt_configuration.c_compatible {
            // C11 cannot spell a fixed underlying enum. Keep storage explicit;
            // use ordinary enumerators where possible, never macros that would
            // accidentally replace same-named aggregate members.
            writeln!(f, "typedef {} {};", self.underlying_type_name, self.name)?;
            if matches!(
                self.underlying_type_name.as_str(),
                "int"
                    | "unsigned int"
                    | "long"
                    | "unsigned long"
                    | "int32_t"
                    | "uint32_t"
                    | "LONG"
                    | "ULONG"
                    | "__int32"
                    | "unsigned __int32"
            ) {
                writeln!(f, "enum {{")?;
            } else {
                for value in &self.values {
                    writeln!(
                        f,
                        "static const {} {} = ({}) ({});",
                        self.name,
                        value.name,
                        self.name,
                        match value.value {
                            pdb::Variant::U8(v) => format!("{v}U"),
                            pdb::Variant::U16(v) => format!("{v}U"),
                            pdb::Variant::U32(v) => format!("{v}U"),
                            pdb::Variant::U64(v) => format!("{v}ULL"),
                            pdb::Variant::I8(v) => format!("{v}"),
                            pdb::Variant::I16(v) => format!("{v}"),
                            pdb::Variant::I32(v) => format!("{v}"),
                            pdb::Variant::I64(v) => format!("{v}LL"),
                        }
                    )?;
                }
                return Ok(());
            }
        } else {
            writeln!(f, "enum {} : {} {{", self.name, self.underlying_type_name)?;
        }

        for value in &self.values {
            writeln!(
                f,
                "  {} = {},",
                value.name.to_string(),
                match value.value {
                    pdb::Variant::U8(v) => {
                        if fmt_configuration.integers_as_hexadecimal {
                            format!("0x{v:02x}")
                        } else {
                            format!("{v}")
                        }
                    }
                    pdb::Variant::U16(v) => {
                        if fmt_configuration.integers_as_hexadecimal {
                            format!("0x{v:04x}")
                        } else {
                            format!("{v}")
                        }
                    }
                    pdb::Variant::U32(v) => {
                        if fmt_configuration.integers_as_hexadecimal {
                            format!("0x{v:08x}")
                        } else {
                            format!("{v}")
                        }
                    }
                    pdb::Variant::U64(v) => {
                        if fmt_configuration.integers_as_hexadecimal {
                            format!("0x{v:016x}")
                        } else {
                            format!("{v}")
                        }
                    }
                    pdb::Variant::I8(v) => format!("{v}"),
                    pdb::Variant::I16(v) => format!("{v}"),
                    pdb::Variant::I32(v) => format!("{v}"),
                    pdb::Variant::I64(v) => format!("{v}"),
                }
            )?;
        }
        writeln!(f, "}};")?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumValue<'p> {
    name: pdb::RawString<'p>,
    value: pdb::Variant,
}
