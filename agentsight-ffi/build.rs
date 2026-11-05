use std::{env, path::PathBuf, process::Command};

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let bpf = root.parent().unwrap().join("bpf");
    let target = env::var("TARGET").unwrap();
    let host = env::var("HOST").unwrap();
    assert!(
        target == host && target.contains("linux"),
        "native Linux builds only; build separately on each target architecture"
    );
    for path in ["src", "cbindgen.toml", "build.rs"] {
        println!("cargo:rerun-if-changed={path}");
    }
    for path in [
        "process.c",
        "process.h",
        "process_core.h",
        "process_output.h",
        "process.bpf.c",
        "process_ext",
        "container_info.h",
        "Makefile",
    ] {
        println!("cargo:rerun-if-changed={}", bpf.join(path).display());
    }
    assert!(
        Command::new("make")
            .current_dir(&bpf)
            .arg("ffi-core")
            .status()
            .expect("make must be installed")
            .success(),
        "C/BPF core build failed"
    );
    println!(
        "cargo:rustc-link-search=native={}",
        bpf.join(".output-ffi").display()
    );
    println!("cargo:rustc-link-lib=static=agentsight_core");
    println!("cargo:rustc-link-lib=static=bpf");
    println!("cargo:rustc-link-lib=elf");
    println!("cargo:rustc-link-lib=z");
    println!("cargo:rustc-link-arg=-Wl,--exclude-libs,ALL");
    println!("cargo:rustc-link-arg=-Wl,-z,defs");
    let config = cbindgen::Config::from_file(root.join("cbindgen.toml")).unwrap();
    let bindings = cbindgen::Builder::new()
        .with_crate(&root)
        .with_config(config)
        .generate()
        .expect("generate C ABI header");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("agentsight.h");
    bindings.write_to_file(&out);
    // Check names in addition to C/C++ compilation tests of the generated ABI.
    let header = std::fs::read_to_string(&out).unwrap();
    for name in [
        "agentsight_abi_version",
        "agentsight_init",
        "agentsight_set_config",
        "agentsight_start",
        "agentsight_stop",
        "agentsight_free",
        "agentsight_get_eventfd",
        "agentsight_poll_events",
        "agentsight_free_event",
        "agentsight_get_last_error",
        "agentsight_get_stats",
    ] {
        assert!(
            header.contains(&format!("{name}(")),
            "cbindgen omitted {name}"
        );
    }
    std::fs::create_dir_all(root.join("include")).unwrap();
    bindings.write_to_file(root.join("include/agentsight.h"));
}
