fn main() {
    // On macOS, we need to link against the nginx object files
    // because dynamic libraries can't have undefined symbols
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-arg=-undefined");
        println!("cargo:rustc-link-arg=dynamic_lookup");
    }
}

