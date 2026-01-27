//! Build script for despice.
//!
//! Provides CSPICE library search path for tests when the `cspice` feature is enabled.
//! CSPICE is only used for validation tests, not production code.

fn main() {
    // Only configure CSPICE linking when feature is enabled
    #[cfg(feature = "cspice")]
    {
        let cspice_lib = std::env::var("CSPICE_LIB")
            .expect("CSPICE_LIB environment variable must be set when cspice feature is enabled");

        let path = std::path::Path::new(&cspice_lib);
        if !path.exists() {
            panic!("CSPICE library path does not exist: {}", cspice_lib);
        }

        println!("cargo:rustc-link-search=native={}", cspice_lib);
    }

    println!("cargo:rerun-if-env-changed=CSPICE_LIB");
}
