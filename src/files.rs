use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

use ignore::{types::TypesBuilder, WalkBuilder, WalkState};

pub fn find_sol_files<T: AsRef<Path>>(paths: &[T]) -> Vec<PathBuf> {
    let types = TypesBuilder::new()
        .add_defaults()
        .negate("all")
        .select("solidity")
        .build()
        .expect("types builder should build");
    let mut walker: Option<WalkBuilder> = None;
    for path in paths {
        if let Some(ext) = path.as_ref().extension() {
            // if users submit paths to non-solidity files, we ignore them
            if ext != "sol" {
                continue;
            }
        }
        if let Some(ref mut w) = walker {
            w.add(path);
        } else {
            walker = Some(WalkBuilder::new(path));
        }
    }
    let Some(mut walker) = walker else {
        return vec![];
    };
    walker
        .hidden(false)
        .parents(false)
        .git_global(false)
        .git_exclude(false)
        .types(types);
    let walker = walker.build_parallel();

    let (tx, rx) = mpsc::channel::<PathBuf>();
    walker.run(|| {
        let tx = tx.clone();
        // function executed for each DirEntry
        Box::new(move |result| {
            let Ok(entry) = result else {
                return WalkState::Continue;
            };
            let path = entry.path();
            if path.is_dir() {
                return WalkState::Continue;
            }
            tx.send(path.to_path_buf())
                .expect("channel receiver should never be dropped before end of function scope");
            WalkState::Continue
        })
    });

    drop(tx);
    // this cannot happen before tx is dropped safely
    let mut files = Vec::new();
    while let Ok(path) = rx.recv() {
        files.push(path);
    }
    files
}
