//! Find Solidity files to analyze
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};

use ignore::{WalkBuilder, WalkState, types::TypesBuilder};

use crate::{
    error::{Error, Result},
    prelude::OrPanic as _,
};

/// Find paths to Solidity files in the provided parent paths, in parallel.
///
/// An optional list of excluded paths (files or folders) can be provided too.
/// `.ignore`, `.gitignore` and `.nsignore` files are honored when filtering the files.
/// Global git ignore configurations as well as parent folder gitignores are not taken into account.
/// Hidden files are included.
/// Returned paths are canonicalized.
pub fn find_sol_files<T: AsRef<Path>>(
    paths: &[T],
    exclude: &[T],
    sort: bool,
) -> Result<Vec<PathBuf>> {
    // canonicalize exclude paths
    let exclude = exclude
        .iter()
        .map(|p| {
            dunce::canonicalize(p.as_ref()).map_err(|err| Error::IOError {
                path: p.as_ref().to_path_buf(),
                err,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let exclude = Arc::new(exclude.iter().map(PathBuf::as_path).collect::<Vec<_>>());

    // types filter to only consider Solidity files
    let mut builder = TypesBuilder::new();
    builder
        .add("solidity", "*.sol")
        .or_panic("glob should be valid");
    let types = builder
        .select("solidity")
        .build()
        .or_panic("types builder should build");

    // build the walker
    let mut walker: Option<WalkBuilder> = None;
    for path in paths {
        let path = dunce::canonicalize(path.as_ref()).map_err(|err| Error::IOError {
            path: path.as_ref().to_path_buf(),
            err,
        })?;
        if let Some(ext) = path.extension()
            && ext != "sol"
        {
            // if users submit paths to non-solidity files, we ignore them
            continue;
        }
        if let Some(ref mut w) = walker {
            w.add(path);
        } else {
            walker = Some(WalkBuilder::new(path));
        }
    }
    let Some(mut walker) = walker else {
        // no path was provided
        return Ok(Vec::new());
    };
    walker
        .hidden(false)
        .git_global(false)
        .git_exclude(false)
        .add_custom_ignore_filename(".nsignore")
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
            // skip path if excluded (don't descend into directories and skip files)
            if exclude.contains(&path) {
                return WalkState::Skip;
            }
            // descend into other directories
            if path.is_dir() {
                return WalkState::Continue;
            }
            // we found a suitable file
            tx.send(path.to_path_buf()).ok();
            WalkState::Continue
        })
    });

    drop(tx);
    // this cannot happen before tx is dropped safely
    let mut files = Vec::new();
    while let Ok(path) = rx.recv() {
        files.push(path);
    }
    if sort {
        files.sort_unstable();
    }
    Ok(files)
}
