//! Tests must run on any machine: CI on Windows, macOS and Linux, and every contributor's computer.
//! A temporary folder comes from the system (`tempfile::tempdir()`, which honours `TMPDIR`), never
//! from a path on one developer's disk. Release 1.0.0's CI failed on every platform because test
//! files named such a path.

use std::path::Path;

/// Path prefixes that belong to one machine's disk layout, never to a portable test.
const MACHINE_PATHS: [&str; 3] = ["/media/", "/home/", "/Users/"];

#[test]
fn no_test_file_names_a_folder_on_one_developers_disk() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut offenders = Vec::new();
    for krate in std::fs::read_dir(&crates).unwrap() {
        let tests = krate.unwrap().path().join("tests");
        let Ok(files) = std::fs::read_dir(&tests) else {
            continue;
        };
        for file in files {
            let path = file.unwrap().path();
            // This file names the prefixes it looks for.
            if path.extension().is_none_or(|e| e != "rs")
                || path.file_name() == Path::new(file!()).file_name()
            {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (n, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or_default();
                if MACHINE_PATHS
                    .iter()
                    .any(|p| code.contains(&format!("\"{p}")))
                {
                    offenders.push(format!("{}:{}", path.display(), n + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "machine-specific paths in tests:\n{}",
        offenders.join("\n")
    );
}
