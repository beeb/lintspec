use std::process::Command;

use divan::Bencher;
use temp_dir::TempDir;

fn main() {
    divan::main();
}

#[cfg(all(feature = "cli", feature = "solar"))]
#[divan::bench]
fn e2e_uniswap(bencher: Bencher) {
    use std::sync::Arc;

    use lintspec::{
        cli::run,
        config::{BaseConfig, Config, OutputConfig},
    };

    let d = TempDir::new().unwrap();
    let mut git = Command::new("git");
    git.args([
        "clone",
        "--depth=1",
        "https://github.com/Uniswap/v4-core.git",
    ])
    .current_dir(d.path())
    .env("GIT_TERMINAL_PROMPT", "0");
    let res = git.output();
    if let Err(err) = res {
        drop(d);
        panic!("{err:#?}")
    }

    let config = Config::builder()
        .lintspec(
            BaseConfig::builder()
                .paths(vec![d.child("v4-core/src")])
                .build(),
        )
        .output(
            OutputConfig::builder()
                .compact(true)
                .out(d.child("lintspec.out"))
                .build(),
        )
        .build();

    let d = Arc::new(d);
    bencher.bench(move || {
        let res = run(&config);
        let d = d.clone();
        if let Err(err) = res {
            drop(d);
            panic!("{err:#?}")
        } else {
            d // keep temp dir around
        }
    });
}
