mod frontend;
mod resymc_app;
mod resymc_options;
mod syntax_highlighting;

use anyhow::Result;
use resym_core::pdb_types::PrimitiveReconstructionFlavor;
use structopt::StructOpt;

use crate::resymc_app::ResymcApp;
use crate::resymc_options::{ResymcCommand, ResymcOptions};

const DEFAULT_PRIMITIVE_FLAVOR: PrimitiveReconstructionFlavor = PrimitiveReconstructionFlavor::Msvc;

fn main() -> Result<()> {
    env_logger::init();

    // Process command and options
    let opt = ResymcOptions::from_args();
    if opt.c_compatible {
        // Validate flag combinations before opening the input or output.
        opt.validate()?;
        let pdb = resym_core::pdb_file::PdbFile::load_from_file(&opt.pdb_path)?;
        let format = resym_core::pdb_types::DataFormatConfiguration {
            print_access_specifiers: false,
            integers_as_hexadecimal: opt.integers_as_hexadecimal,
            c_compatible: true,
        };
        let output = pdb.reconstruct_types(
            opt.dump.as_deref(),
            opt.primitive_types_flavor
                .unwrap_or(DEFAULT_PRIMITIVE_FLAVOR),
            opt.print_dependencies,
            opt.ignore_std_types,
            &format,
        )?;
        if let Some(path) = opt.output_file_path.or(opt.positional_output_file_path) {
            std::fs::write(path, output)?;
        } else if opt.highlight_syntax {
            let theme = resym_core::syntax_highlighting::CodeTheme::default();
            print!(
                "{}",
                crate::syntax_highlighting::highlight_code(&theme, &output, None).unwrap_or(output)
            );
        } else {
            print!("{output}");
        }
        return Ok(());
    }
    let command = opt.into_command()?;
    let app = ResymcApp::new()?;
    match command {
        ResymcCommand::List {
            pdb_path,
            type_name_filter,
            output_file_path,
            case_insensitive,
            use_regex,
            ignore_std_types,
        } => app.list_types_command(
            pdb_path,
            type_name_filter,
            case_insensitive,
            use_regex,
            ignore_std_types,
            output_file_path,
        ),
        ResymcCommand::Dump {
            pdb_path,
            type_name,
            output_file_path,
            primitive_types_flavor,
            print_header,
            print_dependencies,
            print_access_specifiers,
            integers_as_hexadecimal,
            ignore_std_types,
            highlight_syntax,
        } => app.dump_types_command(
            pdb_path,
            Some(type_name),
            primitive_types_flavor.unwrap_or(DEFAULT_PRIMITIVE_FLAVOR),
            print_header,
            print_dependencies,
            print_access_specifiers,
            integers_as_hexadecimal,
            ignore_std_types,
            highlight_syntax,
            output_file_path,
        ),
        ResymcCommand::DumpAll {
            pdb_path,
            output_file_path,
            primitive_types_flavor,
            print_header,
            print_access_specifiers,
            integers_as_hexadecimal,
            ignore_std_types,
            highlight_syntax,
        } => app.dump_types_command(
            pdb_path,
            None,
            primitive_types_flavor.unwrap_or(DEFAULT_PRIMITIVE_FLAVOR),
            print_header,
            false,
            print_access_specifiers,
            integers_as_hexadecimal,
            ignore_std_types,
            highlight_syntax,
            output_file_path,
        ),
        ResymcCommand::Diff {
            from_pdb_path,
            to_pdb_path,
            type_name,
            output_file_path,
            primitive_types_flavor,
            print_header,
            print_dependencies,
            print_access_specifiers,
            integers_as_hexadecimal,
            ignore_std_types,
            highlight_syntax,
        } => app.diff_type_command(
            from_pdb_path,
            to_pdb_path,
            type_name,
            primitive_types_flavor.unwrap_or(DEFAULT_PRIMITIVE_FLAVOR),
            print_header,
            print_dependencies,
            print_access_specifiers,
            integers_as_hexadecimal,
            ignore_std_types,
            highlight_syntax,
            output_file_path,
        ),
        ResymcCommand::ListModules {
            pdb_path,
            module_path_filter,
            output_file_path,
            case_insensitive,
            use_regex,
        } => app.list_modules_command(
            pdb_path,
            module_path_filter,
            case_insensitive,
            use_regex,
            output_file_path,
        ),
        ResymcCommand::DumpModule {
            pdb_path,
            module_id,
            output_file_path,
            primitive_types_flavor,
            print_header,
            print_access_specifiers,
            highlight_syntax,
        } => app.dump_module_command(
            pdb_path,
            module_id,
            primitive_types_flavor.unwrap_or(DEFAULT_PRIMITIVE_FLAVOR),
            print_header,
            print_access_specifiers,
            highlight_syntax,
            output_file_path,
        ),
        ResymcCommand::DiffModule {
            from_pdb_path,
            to_pdb_path,
            module_path,
            output_file_path,
            primitive_types_flavor,
            print_header,
            print_access_specifiers,
            highlight_syntax,
        } => app.diff_module_command(
            from_pdb_path,
            to_pdb_path,
            module_path,
            primitive_types_flavor.unwrap_or(DEFAULT_PRIMITIVE_FLAVOR),
            print_header,
            print_access_specifiers,
            highlight_syntax,
            output_file_path,
        ),
        ResymcCommand::ListSymbols {
            pdb_path,
            symbol_name_filter,
            output_file_path,
            case_insensitive,
            use_regex,
            ignore_std_types,
        } => app.list_symbols_command(
            pdb_path,
            symbol_name_filter,
            case_insensitive,
            use_regex,
            ignore_std_types,
            output_file_path,
        ),
        ResymcCommand::DumpSymbol {
            pdb_path,
            symbol_name,
            output_file_path,
            primitive_types_flavor,
            print_header,
            print_access_specifiers,
            highlight_syntax,
        } => app.dump_symbol_command(
            pdb_path,
            Some(symbol_name),
            primitive_types_flavor.unwrap_or(DEFAULT_PRIMITIVE_FLAVOR),
            print_header,
            print_access_specifiers,
            highlight_syntax,
            output_file_path,
        ),
        ResymcCommand::DumpAllSymbols {
            pdb_path,
            output_file_path,
            primitive_types_flavor,
            print_header,
            print_access_specifiers,
            highlight_syntax,
        } => app.dump_symbol_command(
            pdb_path,
            None,
            primitive_types_flavor.unwrap_or(DEFAULT_PRIMITIVE_FLAVOR),
            print_header,
            print_access_specifiers,
            highlight_syntax,
            output_file_path,
        ),
        ResymcCommand::DiffSymbol {
            from_pdb_path,
            to_pdb_path,
            symbol_name,
            output_file_path,
            primitive_types_flavor,
            print_header,
            print_access_specifiers,
            highlight_syntax,
        } => app.diff_symbol_command(
            from_pdb_path,
            to_pdb_path,
            symbol_name,
            primitive_types_flavor.unwrap_or(DEFAULT_PRIMITIVE_FLAVOR),
            print_header,
            print_access_specifiers,
            highlight_syntax,
            output_file_path,
        ),
    }
}
