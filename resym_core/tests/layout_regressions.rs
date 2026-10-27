use std::{fs, path::Path, process::Command};

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

#[test]
fn generated_c_and_cpp_compile_and_preserve_bitfields() {
    // Set RESYM_TEST_CL in a configured x64 developer shell on Windows.
    // Linux CI uses its system C/C++ compilers.
    let windows = cfg!(windows);
    if windows && std::env::var_os("RESYM_TEST_CL").is_none() {
        eprintln!("Set RESYM_TEST_CL in an MSVC developer shell to run native compiler checks");
        return;
    }
    let dir = tempdir::TempDir::new("resym-layout-regression").unwrap();
    for c in [false, true] {
        let compiler = if windows {
            std::env::var("RESYM_TEST_CL").unwrap()
        } else {
            std::env::var(if c { "CC" } else { "CXX" })
                .unwrap_or_else(|_| if c { "cc" } else { "c++" }.to_owned())
        };
        let source = dir.path().join(if c { "layout.c" } else { "layout.cpp" });
        let exe = dir
            .path()
            .join(if windows { "layout.exe" } else { "layout" });
        let mut code = if c {
            String::new()
        } else {
            "#include <cstdint>\n#include <cstddef>\n".to_owned()
        };
        code.push_str(&dump(c));
        code.push_str(if c {
            "\n#define CHECK _Static_assert\n"
        } else {
            "\n#define CHECK static_assert\n"
        });
        code.push_str(
            r#"
CHECK(sizeof(PackedRecord) == 9, "packed size");
CHECK(offsetof(PackedRecord, value) == 1, "packed member");
CHECK(sizeof(AlignedEntry) == 16, "tail extent");
CHECK(sizeof(CacheLines) == 192, "cache lines");
CHECK(offsetof(CacheLines, second) == 64, "cache line member");
CHECK(offsetof(CacheLines, third) == 128, "cache line member");
CHECK(sizeof(QualifiedRecord) == 16, "qualified complete type");
CHECK(offsetof(QualifiedRecord, after) == 8, "qualified field size");
CHECK(sizeof(ArrayRecord) == 8, "array element count");
CHECK(sizeof(CallbackTable) == 16, "callback size");
CHECK(sizeof(AnonymousEnumRecord) == 4, "anonymous enum reference");
CHECK(sizeof(WideEnumRecord) == 8, "wide enum storage");
CHECK(sizeof(BitRecord) == 8, "bitfield allocation size");
CHECK(offsetof(BitRecord, after) == 4, "bitfield storage extent");
int main(void) {
    if (WideValue != (uint64_t)~0ULL || SignedMinusOne != -1 || SignedMinusTwo != -2 || SignedMinus128 != -128) return 1;
    BitRecord bits = {0};
    bits.first = 7; bits.second = 63; bits.third = 3;
    const unsigned char* bytes = (const unsigned char*)&bits;
    return !(bytes[0] == 0xe7 && bytes[1] == 0x1f && bytes[2] == 0 && bytes[3] == 0);
}
"#,
        );
        fs::write(&source, code).unwrap();
        let mut command = Command::new(&compiler);
        command.current_dir(dir.path());
        if windows {
            command.args([
                "/nologo",
                if c { "/TC" } else { "/TP" },
                if c { "/std:c11" } else { "/std:c++17" },
            ]);
            command.arg(format!("/Fe{}", exe.display())).arg(&source);
        } else {
            command
                .args([if c { "-std=c11" } else { "-std=c++17" }, "-o"])
                .arg(&exe)
                .arg(&source)
                .arg("-mms-bitfields");
        }
        let result = command.output().expect("run C/C++ compiler");
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            Command::new(&exe).status().unwrap().success(),
            "bitfield bytes differ from PDB layout"
        );
    }
}
