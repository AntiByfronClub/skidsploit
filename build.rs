#![allow(non_snake_case)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn collectSources(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collectSources(&path, out);
            } else if let Some(ext) = path.extension().and_then(|value| value.to_str()) {
                if ext.eq_ignore_ascii_case("cpp") {
                    out.push(path);
                }
            }
        }
    }
}

fn main() {
    if !cfg!(target_os = "windows") {
        return;
    }
    if env::var("TARGET").unwrap_or_default() != "x86_64-pc-windows-msvc" {
        panic!("skidsploit only builds for x86_64-pc-windows-msvc");
    }

    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let deps = Path::new(&manifest).join("src").join("deps");

    let mut sources = Vec::new();

    sources.push(deps.join("luau").join("Common").join("src").join("StringUtils.cpp"));
    sources.push(deps.join("luau").join("Common").join("src").join("TimeTrace.cpp"));

    collectSources(&deps.join("luau").join("Ast").join("src"), &mut sources);
    collectSources(&deps.join("luau").join("Compiler").join("src"), &mut sources);
    sources.push(deps.join("luau").join("Bytecode").join("src").join("BytecodeBuilder.cpp"));

    sources.push(deps.join("signer").join("signer.cpp"));
    sources.push(deps.join("signer").join("solver.cpp"));
    sources.push(deps.join("signer").join("bits.cpp"));

    for source in ["blake3.c", "blake3_dispatch.c", "blake3_portable.c"] {
        sources.push(deps.join("signer").join("blake3").join(source));
    }

    collectSources(&deps.join("bridge"), &mut sources);

    let mut build = cc::Build::new();
    build.cpp(true)
        .std("c++20")
        .flag("/EHsc")
        .warnings(false)
        .define("NOMINMAX", None)
        .define("WIN32_LEAN_AND_MEAN", None)
        .define("BLAKE3_NO_SSE2", None)
        .define("BLAKE3_NO_SSE41", None)
        .define("BLAKE3_NO_AVX2", None)
        .define("BLAKE3_NO_AVX512", None)
        .define("BLAKE3_NO_NEON", None)
        .include(deps.join("luau").join("Common").join("include"))
        .include(deps.join("luau").join("Ast").join("include"))
        .include(deps.join("luau").join("Compiler").join("include"))
        .include(deps.join("luau").join("Bytecode").join("include"))
        .include(&deps)
        .include(deps.join("crypt").join("include"))
        .files(sources)
        .compile("skidsploit_native");

    let cryptLib = deps.join("crypt").join("lib");
    println!("cargo:rustc-link-search=native={}", cryptLib.display());
    println!("cargo:rustc-link-lib=static=zstd_static");
    println!("cargo:rerun-if-changed=src/deps");
}
