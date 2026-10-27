use anyhow::{bail, Context, Result};
use resym_core::pdb_types::PrimitiveReconstructionFlavor;
use std::path::PathBuf;
use structopt::{clap::ArgGroup, StructOpt};

#[derive(Debug, StructOpt)]
#[structopt(name = "resymc", about = "Browse and reconstruct PDB types and symbols.",
    group = ArgGroup::with_name("mode").required(true).multiple(false))]
pub struct ResymcOptions {
    /// PDB to read (or the source PDB for a diff)
    pub pdb_path: PathBuf,
    /// Optional positional output filename (alternative to --output)
    #[structopt(name = "output-file")]
    pub positional_output_file_path: Option<PathBuf>,
    /// List matching types
    #[structopt(long, group = "mode")]
    pub list: bool,
    /// Reconstruct one type
    #[structopt(long, group = "mode", value_name = "TYPE")]
    pub dump: Option<String>,
    /// Reconstruct all types
    #[structopt(long, group = "mode")]
    pub dump_all: bool,
    /// Diff a type against --to
    #[structopt(long, group = "mode", value_name = "TYPE")]
    pub diff: Option<String>,
    /// List matching modules
    #[structopt(long, group = "mode")]
    pub list_modules: bool,
    /// Reconstruct a module by ID
    #[structopt(long, group = "mode", value_name = "ID")]
    pub dump_module: Option<usize>,
    /// Diff a module path against --to
    #[structopt(long, group = "mode", value_name = "MODULE")]
    pub diff_module: Option<String>,
    /// List matching symbols
    #[structopt(long, group = "mode")]
    pub list_symbols: bool,
    /// Reconstruct one symbol
    #[structopt(long, group = "mode", value_name = "SYMBOL")]
    pub dump_symbol: Option<String>,
    /// Reconstruct all symbols
    #[structopt(long, group = "mode")]
    pub dump_all_symbols: bool,
    /// Diff a symbol against --to
    #[structopt(long, group = "mode", value_name = "SYMBOL")]
    pub diff_symbol: Option<String>,
    /// Destination PDB for a diff
    #[structopt(long = "to", value_name = "PDB")]
    pub to_pdb_path: Option<PathBuf>,
    /// Filter for --list, --list-modules or --list-symbols
    #[structopt(long, default_value = "")]
    pub filter: String,
    /// Output file (defaults to stdout)
    #[structopt(short = "o", long = "output", value_name = "FILE")]
    pub output_file_path: Option<PathBuf>,
    /// Primitive spelling: portable, microsoft, raw, or msvc
    #[structopt(short = "f", long = "primitive-types-flavor")]
    pub primitive_types_flavor: Option<PrimitiveReconstructionFlavor>,
    /// Include the primitive-flavor preamble
    #[structopt(long)]
    pub print_header: bool,
    /// Include referenced types when dumping/diffing one type
    #[structopt(short = "d", long)]
    pub print_dependencies: bool,
    /// Include C++ access specifiers
    #[structopt(short = "a", long)]
    pub print_access_specifiers: bool,
    /// Print integer values in hexadecimal
    #[structopt(short = "x", long)]
    pub integers_as_hexadecimal: bool,
    /// Filter out std namespace types
    #[structopt(short = "s", long)]
    pub ignore_std_types: bool,
    /// Colorize stdout
    #[structopt(short = "H", long)]
    pub highlight_syntax: bool,
    /// Ignore case when filtering
    #[structopt(short = "i", long)]
    pub case_insensitive: bool,
    /// Treat the filter as a regular expression
    #[structopt(short = "r", long)]
    pub use_regex: bool,
    /// Emit C11 storage declarations for --dump/--dump-all; reject C++-only types
    #[structopt(long)]
    pub c_compatible: bool,
}

impl ResymcOptions {
    pub fn validate(&self) -> Result<()> {
        if self.output_file_path.is_some() && self.positional_output_file_path.is_some() {
            bail!("provide the output filename once, positionally or with --output");
        }
        if self.c_compatible && !(self.dump_all || self.dump.is_some()) {
            bail!("--c-compatible requires --dump or --dump-all");
        }
        if self.c_compatible && self.print_access_specifiers {
            bail!("--c-compatible cannot be combined with --print-access-specifiers");
        }
        if self.to_pdb_path.is_some()
            && !(self.diff.is_some() || self.diff_module.is_some() || self.diff_symbol.is_some())
        {
            bail!("--to requires a diff mode");
        }
        Ok(())
    }
    pub fn into_command(mut self) -> Result<ResymcCommand> {
        self.validate()?;
        self.output_file_path = self.output_file_path.or(self.positional_output_file_path);
        if self.list {
            return Ok(ResymcCommand::List {
                pdb_path: self.pdb_path,
                type_name_filter: self.filter,
                output_file_path: self.output_file_path,
                case_insensitive: self.case_insensitive,
                use_regex: self.use_regex,
                ignore_std_types: self.ignore_std_types,
            });
        }
        if let Some(selection) = self.dump {
            return Ok(ResymcCommand::Dump {
                pdb_path: self.pdb_path,
                type_name: selection,
                output_file_path: self.output_file_path,
                primitive_types_flavor: self.primitive_types_flavor,
                print_header: self.print_header,
                print_dependencies: self.print_dependencies,
                print_access_specifiers: self.print_access_specifiers,
                integers_as_hexadecimal: self.integers_as_hexadecimal,
                ignore_std_types: self.ignore_std_types,
                highlight_syntax: self.highlight_syntax,
            });
        }
        if self.dump_all {
            return Ok(ResymcCommand::DumpAll {
                pdb_path: self.pdb_path,
                output_file_path: self.output_file_path,
                primitive_types_flavor: self.primitive_types_flavor,
                print_header: self.print_header,
                print_access_specifiers: self.print_access_specifiers,
                integers_as_hexadecimal: self.integers_as_hexadecimal,
                ignore_std_types: self.ignore_std_types,
                highlight_syntax: self.highlight_syntax,
            });
        }
        if let Some(selection) = self.diff {
            return Ok(ResymcCommand::Diff {
                from_pdb_path: self.pdb_path,
                to_pdb_path: self
                    .to_pdb_path
                    .context("--to <PDB> is required for a diff")?,
                type_name: selection,
                output_file_path: self.output_file_path,
                primitive_types_flavor: self.primitive_types_flavor,
                print_header: self.print_header,
                print_dependencies: self.print_dependencies,
                print_access_specifiers: self.print_access_specifiers,
                integers_as_hexadecimal: self.integers_as_hexadecimal,
                ignore_std_types: self.ignore_std_types,
                highlight_syntax: self.highlight_syntax,
            });
        }
        if self.list_modules {
            return Ok(ResymcCommand::ListModules {
                pdb_path: self.pdb_path,
                module_path_filter: self.filter,
                output_file_path: self.output_file_path,
                case_insensitive: self.case_insensitive,
                use_regex: self.use_regex,
            });
        }
        if let Some(selection) = self.dump_module {
            return Ok(ResymcCommand::DumpModule {
                pdb_path: self.pdb_path,
                module_id: selection,
                output_file_path: self.output_file_path,
                primitive_types_flavor: self.primitive_types_flavor,
                print_header: self.print_header,
                print_access_specifiers: self.print_access_specifiers,
                highlight_syntax: self.highlight_syntax,
            });
        }
        if let Some(selection) = self.diff_module {
            return Ok(ResymcCommand::DiffModule {
                from_pdb_path: self.pdb_path,
                to_pdb_path: self
                    .to_pdb_path
                    .context("--to <PDB> is required for a diff")?,
                module_path: selection,
                output_file_path: self.output_file_path,
                primitive_types_flavor: self.primitive_types_flavor,
                print_header: self.print_header,
                print_access_specifiers: self.print_access_specifiers,
                highlight_syntax: self.highlight_syntax,
            });
        }
        if self.list_symbols {
            return Ok(ResymcCommand::ListSymbols {
                pdb_path: self.pdb_path,
                symbol_name_filter: self.filter,
                output_file_path: self.output_file_path,
                case_insensitive: self.case_insensitive,
                use_regex: self.use_regex,
                ignore_std_types: self.ignore_std_types,
            });
        }
        if let Some(selection) = self.dump_symbol {
            return Ok(ResymcCommand::DumpSymbol {
                pdb_path: self.pdb_path,
                symbol_name: selection,
                output_file_path: self.output_file_path,
                primitive_types_flavor: self.primitive_types_flavor,
                print_header: self.print_header,
                print_access_specifiers: self.print_access_specifiers,
                highlight_syntax: self.highlight_syntax,
            });
        }
        if self.dump_all_symbols {
            return Ok(ResymcCommand::DumpAllSymbols {
                pdb_path: self.pdb_path,
                output_file_path: self.output_file_path,
                primitive_types_flavor: self.primitive_types_flavor,
                print_header: self.print_header,
                print_access_specifiers: self.print_access_specifiers,
                highlight_syntax: self.highlight_syntax,
            });
        }
        if let Some(selection) = self.diff_symbol {
            return Ok(ResymcCommand::DiffSymbol {
                from_pdb_path: self.pdb_path,
                to_pdb_path: self
                    .to_pdb_path
                    .context("--to <PDB> is required for a diff")?,
                symbol_name: selection,
                output_file_path: self.output_file_path,
                primitive_types_flavor: self.primitive_types_flavor,
                print_header: self.print_header,
                print_access_specifiers: self.print_access_specifiers,
                highlight_syntax: self.highlight_syntax,
            });
        }
        bail!("select an operation such as --dump-all")
    }
}

#[derive(Debug)]
pub enum ResymcCommand {
    List {
        pdb_path: PathBuf,
        type_name_filter: String,
        output_file_path: Option<PathBuf>,
        case_insensitive: bool,
        use_regex: bool,
        ignore_std_types: bool,
    },
    Dump {
        pdb_path: PathBuf,
        type_name: String,
        output_file_path: Option<PathBuf>,
        primitive_types_flavor: Option<PrimitiveReconstructionFlavor>,
        print_header: bool,
        print_dependencies: bool,
        print_access_specifiers: bool,
        integers_as_hexadecimal: bool,
        ignore_std_types: bool,
        highlight_syntax: bool,
    },
    DumpAll {
        pdb_path: PathBuf,
        output_file_path: Option<PathBuf>,
        primitive_types_flavor: Option<PrimitiveReconstructionFlavor>,
        print_header: bool,
        print_access_specifiers: bool,
        integers_as_hexadecimal: bool,
        ignore_std_types: bool,
        highlight_syntax: bool,
    },
    Diff {
        from_pdb_path: PathBuf,
        to_pdb_path: PathBuf,
        type_name: String,
        output_file_path: Option<PathBuf>,
        primitive_types_flavor: Option<PrimitiveReconstructionFlavor>,
        print_header: bool,
        print_dependencies: bool,
        print_access_specifiers: bool,
        integers_as_hexadecimal: bool,
        ignore_std_types: bool,
        highlight_syntax: bool,
    },
    ListModules {
        pdb_path: PathBuf,
        module_path_filter: String,
        output_file_path: Option<PathBuf>,
        case_insensitive: bool,
        use_regex: bool,
    },
    DumpModule {
        pdb_path: PathBuf,
        module_id: usize,
        output_file_path: Option<PathBuf>,
        primitive_types_flavor: Option<PrimitiveReconstructionFlavor>,
        print_header: bool,
        print_access_specifiers: bool,
        highlight_syntax: bool,
    },
    DiffModule {
        from_pdb_path: PathBuf,
        to_pdb_path: PathBuf,
        module_path: String,
        output_file_path: Option<PathBuf>,
        primitive_types_flavor: Option<PrimitiveReconstructionFlavor>,
        print_header: bool,
        print_access_specifiers: bool,
        highlight_syntax: bool,
    },
    ListSymbols {
        pdb_path: PathBuf,
        symbol_name_filter: String,
        output_file_path: Option<PathBuf>,
        case_insensitive: bool,
        use_regex: bool,
        ignore_std_types: bool,
    },
    DumpSymbol {
        pdb_path: PathBuf,
        symbol_name: String,
        output_file_path: Option<PathBuf>,
        primitive_types_flavor: Option<PrimitiveReconstructionFlavor>,
        print_header: bool,
        print_access_specifiers: bool,
        highlight_syntax: bool,
    },
    DumpAllSymbols {
        pdb_path: PathBuf,
        output_file_path: Option<PathBuf>,
        primitive_types_flavor: Option<PrimitiveReconstructionFlavor>,
        print_header: bool,
        print_access_specifiers: bool,
        highlight_syntax: bool,
    },
    DiffSymbol {
        from_pdb_path: PathBuf,
        to_pdb_path: PathBuf,
        symbol_name: String,
        output_file_path: Option<PathBuf>,
        primitive_types_flavor: Option<PrimitiveReconstructionFlavor>,
        print_header: bool,
        print_access_specifiers: bool,
        highlight_syntax: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operation_flags_are_exclusive_and_required() {
        assert!(ResymcOptions::from_iter_safe(["resymc", "input.pdb"]).is_err());
        assert!(
            ResymcOptions::from_iter_safe(["resymc", "--dump-all", "--list", "input.pdb"]).is_err()
        );
        assert!(ResymcOptions::from_iter_safe(["resymc", "dump-all", "input.pdb"]).is_err());
    }
    #[test]
    fn dump_all_and_c_mode_parse() {
        let options = ResymcOptions::from_iter_safe([
            "resymc",
            "--dump-all",
            "--c-compatible",
            "input.pdb",
            "-o",
            "types.h",
        ])
        .unwrap();
        assert!(options.c_compatible);
        assert_eq!(
            options.output_file_path.as_deref(),
            Some(std::path::Path::new("types.h"))
        );
        assert!(matches!(
            options.into_command().unwrap(),
            ResymcCommand::DumpAll { .. }
        ));
    }

    #[test]
    fn positional_output_is_preserved() {
        let options =
            ResymcOptions::from_iter_safe(["resymc", "--dump-all", "input.pdb", "types.h"])
                .unwrap();
        assert!(matches!(
            options.into_command().unwrap(),
            ResymcCommand::DumpAll {
                output_file_path: Some(_),
                ..
            }
        ));
        let options = ResymcOptions::from_iter_safe([
            "resymc",
            "--dump-all",
            "input.pdb",
            "types.h",
            "-o",
            "other.h",
        ])
        .unwrap();
        assert!(options.into_command().is_err());
    }
    #[test]
    fn dump_with_dependencies_parses() {
        let options =
            ResymcOptions::from_iter_safe(["resymc", "--dump", "Type", "-d", "input.pdb"]).unwrap();
        assert!(matches!(
            options.into_command().unwrap(),
            ResymcCommand::Dump {
                print_dependencies: true,
                ..
            }
        ));
    }
    #[test]
    fn diff_requires_destination() {
        let options =
            ResymcOptions::from_iter_safe(["resymc", "--diff", "Type", "input.pdb"]).unwrap();
        assert!(options.into_command().is_err());
        let options = ResymcOptions::from_iter_safe([
            "resymc",
            "--diff",
            "Type",
            "input.pdb",
            "--to",
            "other.pdb",
        ])
        .unwrap();
        assert!(matches!(
            options.into_command().unwrap(),
            ResymcCommand::Diff { .. }
        ));
    }
    #[test]
    fn invalid_c_combinations_are_rejected() {
        for arguments in [
            vec!["resymc", "--list", "--c-compatible", "input.pdb"],
            vec!["resymc", "--dump-all", "--c-compatible", "-a", "input.pdb"],
            vec!["resymc", "--dump-all", "--to", "other.pdb", "input.pdb"],
        ] {
            assert!(ResymcOptions::from_iter_safe(arguments)
                .unwrap()
                .into_command()
                .is_err());
        }
    }
}
