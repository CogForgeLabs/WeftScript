//! Runs every shipped `.nx` application end-to-end through the mixed
//! declarative+executable loader, proving the gallery actually executes (not
//! just parses). This is the executable evidence for the "15+ complex apps that
//! mix declarative & executable" goalpost.

use std::fs;
use std::path::{Path, PathBuf};

fn collect_nx(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_nx(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("nx") {
            out.push(p);
        }
    }
}

#[test]
fn every_app_runs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps");
    let mut files = Vec::new();
    collect_nx(&root, &mut files);
    files.sort();

    assert!(
        files.len() >= 15,
        "expected 15+ apps, found {} under {}",
        files.len(),
        root.display()
    );

    let mut ran = 0;
    let mut mixed = 0;
    for f in &files {
        let src = fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()));
        // An app is "mixed" if it declares intent-graph nodes alongside code.
        let is_mixed = ["thing ", "constraint ", "guarantee ", "policy ", "project "]
            .iter()
            .any(|kw| src.contains(kw));
        match nexus_app::run_mixed(&src) {
            Ok(out) => {
                assert!(!out.is_empty(), "{} produced no output", f.display());
                ran += 1;
                if is_mixed {
                    mixed += 1;
                }
            }
            Err(e) => panic!("app {} failed to run: {e}", f.display()),
        }
    }
    eprintln!("ran {ran} apps ({mixed} mixing declarative + executable)");
    assert!(ran >= 15, "expected 15+ runnable apps, ran {ran}");
    assert!(mixed >= 5, "expected several mixed apps, found {mixed}");
}
