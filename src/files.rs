use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
};

use ignore::{types::TypesBuilder, WalkBuilder, WalkState};

use crate::error::Result;

pub fn find_sol_files<T: AsRef<Path>>(paths: &[T], exclude: &[T]) -> Result<Vec<PathBuf>> {
    let exclude = exclude
        .iter()
        .map(|p| dunce::canonicalize(p.as_ref()).map_err(Into::into))
        .collect::<Result<Vec<_>>>()?;
    let exclude = Arc::new(exclude.iter().map(|p| p.as_path()).collect::<Vec<_>>());

    let types = TypesBuilder::new()
        .add_defaults()
        .negate("all")
        .select("solidity")
        .build()
        .expect("types builder should build");
    let mut walker: Option<WalkBuilder> = None;
    for path in paths {
        let path = dunce::canonicalize(path.as_ref())?;
        if let Some(ext) = path.extension() {
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
        return Ok(Vec::new());
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
        let exclude = Arc::clone(&exclude);
        // function executed for each DirEntry
        Box::new(move |result| {
            let Ok(entry) = result else {
                return WalkState::Continue;
            };
            let path = entry.path();
            if exclude.contains(&path) {
                return WalkState::Skip;
            }
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
    Ok(files)
}
