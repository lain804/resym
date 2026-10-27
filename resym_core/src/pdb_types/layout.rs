//! Reconstruct byte layout from PDB member offsets, including anonymous aggregates.
//!
//! PDBs describe member offsets and extents, not the original pragma/alignas source.
//! Scoped packing plus explicit gaps preserves storage layout without pretending to
//! recover an undocumented calling convention or original over-alignment.
use std::{collections::HashSet, fmt};

use pdb::TypeIndex;

use super::{
    find_unnamed_structs_in_unions, find_unnamed_unions_in_struct, resolve_complete_type_index,
    type_size, DataFormatConfiguration, Field, TypeForwarder,
};
use crate::error::Result;

pub(super) fn type_alignment(
    finder: &pdb::TypeFinder,
    forwarder: &TypeForwarder,
    index: TypeIndex,
) -> Result<u64> {
    fn visit(
        finder: &pdb::TypeFinder,
        forwarder: &TypeForwarder,
        index: TypeIndex,
        active: &mut HashSet<TypeIndex>,
    ) -> Result<u64> {
        let index = resolve_complete_type_index(forwarder, index);
        if !active.insert(index) {
            return Ok(1);
        }
        let result = match finder.find(index)?.parse()? {
            pdb::TypeData::Array(data) => visit(finder, forwarder, data.element_type, active)?,
            pdb::TypeData::Modifier(data) => {
                visit(finder, forwarder, data.underlying_type, active)?
            }
            pdb::TypeData::Bitfield(data) => {
                visit(finder, forwarder, data.underlying_type, active)?
            }
            pdb::TypeData::Enumeration(data) => {
                visit(finder, forwarder, data.underlying_type, active)?
            }
            pdb::TypeData::Class(data) => {
                aggregate(finder, forwarder, data.fields, data.size, active)?
            }
            pdb::TypeData::Union(data) => {
                aggregate(finder, forwarder, Some(data.fields), data.size, active)?
            }
            _ => (type_size(finder, forwarder, index)? as u64)
                .clamp(1, 8)
                .next_power_of_two(),
        };
        active.remove(&index);
        Ok(result)
    }
    fn aggregate(
        finder: &pdb::TypeFinder,
        forwarder: &TypeForwarder,
        mut list: Option<TypeIndex>,
        size: u64,
        active: &mut HashSet<TypeIndex>,
    ) -> Result<u64> {
        let mut alignment = 1;
        let mut members = Vec::new();
        while let Some(index) = list {
            let pdb::TypeData::FieldList(data) = finder.find(index)?.parse()? else {
                break;
            };
            for member in data.fields {
                if let pdb::TypeData::Member(member) = member {
                    let align = visit(finder, forwarder, member.field_type, active)?;
                    alignment = alignment.max(align);
                    members.push((member.offset, align));
                }
            }
            list = data.continuation;
        }
        while alignment > 1
            && (size % alignment != 0
                || members.iter().any(|(off, a)| off % alignment.min(*a) != 0))
        {
            alignment /= 2;
        }
        Ok(alignment)
    }
    visit(finder, forwarder, index, &mut HashSet::new())
}

pub(super) fn packing(fields: &[Field], size: u64) -> u64 {
    [8, 4, 2, 1]
        .into_iter()
        .find(|pack| {
            let alignment = fields
                .iter()
                .map(|f| f.alignment.min(*pack))
                .max()
                .unwrap_or(1);
            size % alignment == 0
                && fields
                    .iter()
                    .all(|f| f.offset % f.alignment.min(*pack) == 0)
        })
        .unwrap_or(1)
}

enum Part<'a, 'p> {
    Field(&'a Field<'p>),
    Aggregate {
        union: bool,
        parts: Vec<Part<'a, 'p>>,
    },
}

impl Part<'_, '_> {
    fn start(&self) -> u64 {
        match self {
            Self::Field(f) => f.offset,
            Self::Aggregate { parts, .. } => parts.iter().map(Self::start).min().unwrap_or(0),
        }
    }
    fn end(&self) -> u64 {
        match self {
            Self::Field(f) => f.offset + f.size as u64,
            Self::Aggregate { parts, .. } => parts.iter().map(Self::end).max().unwrap_or(0),
        }
    }
    fn alignment(&self, pack: u64) -> u64 {
        match self {
            Self::Field(f) => f.alignment.min(pack),
            Self::Aggregate { parts, .. } => {
                parts.iter().map(|p| p.alignment(pack)).max().unwrap_or(1)
            }
        }
    }
    fn storage_end(&self, pack: u64) -> u64 {
        if matches!(self, Self::Field(_)) {
            return self.end();
        }
        let align = self.alignment(pack);
        let span = self.end() - self.start();
        self.start() + span.div_ceil(align) * align
    }
}

fn parts<'a, 'p>(fields: &'a [Field<'p>], union: bool) -> Vec<Part<'a, 'p>> {
    if fields.is_empty() {
        return Vec::new();
    }
    let ranges = if union {
        find_unnamed_structs_in_unions(fields)
    } else {
        find_unnamed_unions_in_struct(fields)
    };
    ranges
        .into_iter()
        .map(|range| {
            if range.is_empty() {
                Part::Field(&fields[range.start])
            } else {
                Part::Aggregate {
                    union: !union,
                    parts: parts(&fields[range], !union),
                }
            }
        })
        .collect()
}

fn padding(
    f: &mut impl fmt::Write,
    depth: usize,
    offset: u64,
    count: u64,
    serial: &mut usize,
) -> fmt::Result {
    if count != 0 {
        writeln!(
            f,
            "{}/* {offset:#06x} */ unsigned char __resym_padding_{}[{count}];",
            "  ".repeat(depth),
            *serial
        )?;
        *serial += 1;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Recursive rendering state, including the scoped packing.
fn render(
    config: &DataFormatConfiguration,
    items: &[Part],
    union: bool,
    base: u64,
    depth: usize,
    pack: u64,
    f: &mut impl fmt::Write,
    serial: &mut usize,
) -> fmt::Result {
    let mut end = base;
    let mut previous_bitfield: Option<&Field> = None;
    for (index, item) in items.iter().enumerate() {
        // Anonymous aggregate boundaries were lost in CodeView. A union's
        // natural tail padding must not push the following recorded member.
        let limit = if union {
            u64::MAX
        } else {
            items.get(index + 1).map(Part::start).unwrap_or(u64::MAX)
        };
        let child_pack = [8, 4, 2, 1]
            .into_iter()
            .find(|candidate| {
                *candidate <= pack
                    && item.storage_end(*candidate) <= limit
                    && item.start() % item.alignment(*candidate) == 0
            })
            .unwrap_or(pack);
        if !union && item.start() > end {
            padding(f, depth, end, item.start() - end, serial)?;
        }
        match item {
            Part::Field(field) => {
                if let Some((position, _)) = field.bitfield_info {
                    let preceding = previous_bitfield
                        .filter(|last| last.offset == field.offset)
                        .and_then(|last| last.bitfield_info)
                        .map(|(p, n)| p + n)
                        .unwrap_or(0);
                    if position > preceding {
                        writeln!(
                            f,
                            "{}/* {:#06x} */ {} : {}; /* BitPos={} */",
                            "  ".repeat(depth),
                            field.offset,
                            field.type_left,
                            position - preceding,
                            preceding
                        )?;
                    }
                }
                writeln!(
                    f,
                    "{}/* {:#06x} */ {}{} {}{};{}",
                    "  ".repeat(depth),
                    field.offset,
                    if config.print_access_specifiers {
                        field.access.to_string()
                    } else {
                        String::new()
                    },
                    field.type_left,
                    field.name,
                    if config.c_compatible {
                        field.type_right.replace("()", "(void)")
                    } else {
                        field.type_right.clone()
                    },
                    field
                        .bitfield_info
                        .map(|(pos, _)| format!(" /* BitPos={pos} */"))
                        .unwrap_or_default()
                )?;
                previous_bitfield = field.bitfield_info.map(|_| *field);
            }
            Part::Aggregate { union, parts } => {
                if child_pack != pack {
                    writeln!(f, "#pragma pack(push, {child_pack})")?;
                }
                writeln!(
                    f,
                    "{}{} {{",
                    "  ".repeat(depth),
                    if *union { "union" } else { "struct" }
                )?;
                render(
                    config,
                    parts,
                    *union,
                    item.start(),
                    depth + 1,
                    child_pack,
                    f,
                    serial,
                )?;
                writeln!(f, "{}}};", "  ".repeat(depth))?;
                if child_pack != pack {
                    writeln!(f, "#pragma pack(pop)")?;
                }
                previous_bitfield = None;
            }
        }
        end = end.max(item.storage_end(child_pack));
    }
    Ok(())
}

pub(super) fn fields(
    config: &DataFormatConfiguration,
    fields: &[Field],
    union: bool,
    size: u64,
    f: &mut impl fmt::Write,
) -> fmt::Result {
    let items = parts(fields, union);
    let mut serial = 0;
    let pack = packing(fields, size);
    render(config, &items, union, 0, 1, pack, f, &mut serial)?;
    let end = items.iter().map(|p| p.storage_end(pack)).max().unwrap_or(0);
    let align = fields
        .iter()
        .map(|field| field.alignment.min(pack))
        .max()
        .unwrap_or(1);
    let rounded_end = end.div_ceil(align) * align;
    if union && size > rounded_end {
        padding(f, 1, 0, size, &mut serial)?;
    } else if size > rounded_end {
        padding(f, 1, end, size - end, &mut serial)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_aggregates_do_not_index_an_empty_field_list() {
        assert!(super::parts(&[], false).is_empty());
        assert!(super::parts(&[], true).is_empty());
    }
}
