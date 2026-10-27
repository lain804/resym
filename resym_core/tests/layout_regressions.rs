use std::path::Path;

use resym_core::{
    pdb_file::PdbFile,
    pdb_types::{type_size, DataFormatConfiguration, PrimitiveReconstructionFlavor, TypeForwarder},
};

const FIXTURE: &str = "tests/data/layout_regressions.pdb";

fn dump(c: bool) -> String {
    let pdb = PdbFile::load_from_file(Path::new(FIXTURE)).unwrap();
    pdb.reconstruct_types(
        None,
        PrimitiveReconstructionFlavor::Portable,
        true,
        false,
        &DataFormatConfiguration {
            print_access_specifiers: false,
            integers_as_hexadecimal: true,
            c_compatible: c,
        },
    )
    .unwrap()
}

#[test]
fn c_and_cpp_layout_regressions() {
    insta::assert_snapshot!("layout_cpp", dump(false));
    insta::assert_snapshot!("layout_c", dump(true));
}

#[test]
fn c_rejects_cpp_types() {
    let pdb = PdbFile::load_from_file(Path::new("tests/data/test.pdb")).unwrap();
    let error = pdb
        .reconstruct_types(
            Some("resym_test::StructTest"),
            PrimitiveReconstructionFlavor::Portable,
            true,
            false,
            &DataFormatConfiguration {
                c_compatible: true,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("C output cannot represent C++"));
}

#[test]
fn primitive_sizes_include_hresult_and_char8() {
    let pdb = PdbFile::load_from_file(Path::new(FIXTURE)).unwrap();
    let finder = pdb.type_information.finder();
    assert_eq!(
        type_size(&finder, &TypeForwarder::default(), pdb::TypeIndex(0x0008)).unwrap(),
        4
    );
    assert_eq!(
        type_size(&finder, &TypeForwarder::default(), pdb::TypeIndex(0x007c)).unwrap(),
        1
    );
}

#[test]
fn deterministic_export_and_single_type_dependencies() {
    let first = dump(false);
    assert_eq!(first, dump(false));
    let pdb = PdbFile::load_from_file(Path::new(FIXTURE)).unwrap();
    let output = pdb
        .reconstruct_types(
            Some("CallbackTable"),
            PrimitiveReconstructionFlavor::Portable,
            true,
            false,
            &DataFormatConfiguration {
                print_access_specifiers: false,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        output.find("enum CallbackResult").unwrap()
            < output.find("struct CallbackTable {").unwrap()
    );
    assert!(output.contains("exchange"));
    assert!(!output.contains("<unnamed-enum"));
}
