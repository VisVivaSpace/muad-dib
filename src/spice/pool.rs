//! Typed kernel pool access.
//!
//! Extension trait for `SpiceKernel` providing typed access to kernel pool variables.
//! This is equivalent to CSPICE's `gdpool()`, `gipool()`, `gcpool()` functions.
//!
//! # Example
//!
//! ```ignore
//! use muad_dib::kernel::SpiceKernel;
//! use muad_dib::spice::KernelPoolExt;
//!
//! let kernel = SpiceKernel::load("pck00010.tpc")?;
//!
//! // Get Earth's radii
//! if let Some(radii) = kernel.get_f64("BODY399_RADII") {
//!     println!("Earth radii: {:?}", radii);
//! }
//!
//! // Get a scalar value
//! if let Some(gm) = kernel.get_f64_scalar("BODY399_GM") {
//!     println!("Earth GM: {}", gm);
//! }
//!
//! // Check if variable exists
//! if kernel.pool_has("BODY399_RADII") {
//!     println!("Variable exists");
//! }
//! ```

use crate::kernel::SpiceKernel;

/// Extension trait for typed kernel pool access.
///
/// Provides methods for retrieving kernel pool variables with type safety.
pub trait KernelPoolExt {
    /// Get a variable as a vector of f64 values.
    ///
    /// Returns `None` if the variable doesn't exist or contains non-numeric values.
    fn get_f64(&self, name: &str) -> Option<Vec<f64>>;

    /// Get a scalar f64 value (first element of an array).
    ///
    /// Returns `None` if the variable doesn't exist, is empty, or contains non-numeric values.
    fn get_f64_scalar(&self, name: &str) -> Option<f64>;

    /// Get a variable as a vector of i32 values.
    ///
    /// Returns `None` if the variable doesn't exist or values can't be converted to i32.
    fn get_i32(&self, name: &str) -> Option<Vec<i32>>;

    /// Get a scalar i32 value (first element of an array).
    ///
    /// Returns `None` if the variable doesn't exist, is empty, or can't be converted to i32.
    fn get_i32_scalar(&self, name: &str) -> Option<i32>;

    /// Get a variable as a vector of strings (text or epoch values).
    ///
    /// Returns `None` if the variable doesn't exist. Numeric values are excluded.
    fn get_strings(&self, name: &str) -> Option<Vec<String>>;

    /// Check if a variable exists in the kernel pool.
    fn pool_has(&self, name: &str) -> bool;

    /// Get the count of values for a variable.
    ///
    /// Returns `None` if the variable doesn't exist.
    fn pool_count(&self, name: &str) -> Option<usize>;
}

impl KernelPoolExt for SpiceKernel {
    fn get_f64(&self, name: &str) -> Option<Vec<f64>> {
        self.pck_lookup(name)?.values_as_f64()
    }

    fn get_f64_scalar(&self, name: &str) -> Option<f64> {
        self.get_f64(name)?.first().copied()
    }

    fn get_i32(&self, name: &str) -> Option<Vec<i32>> {
        let floats = self.get_f64(name)?;
        Some(floats.into_iter().map(|f| f as i32).collect())
    }

    fn get_i32_scalar(&self, name: &str) -> Option<i32> {
        self.get_i32(name)?.first().copied()
    }

    fn get_strings(&self, name: &str) -> Option<Vec<String>> {
        let var = self.pck_lookup(name)?;
        let strings: Vec<String> = var.text_values().into_iter().map(String::from).collect();
        if strings.is_empty() {
            None
        } else {
            Some(strings)
        }
    }

    fn pool_has(&self, name: &str) -> bool {
        self.pck_lookup(name).is_some()
    }

    fn pool_count(&self, name: &str) -> Option<usize> {
        Some(self.pck_lookup(name)?.values.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_pck::{KernelValue, PCKBlock, PCKSource, PCKVariable};
    use crate::kernel::CoverageIndex;

    /// Create a test kernel with sample PCK data.
    fn make_test_kernel() -> SpiceKernel {
        let pck = PCKSource {
            filename: "test.tpc".to_string(),
            blocks: vec![PCKBlock::Data(vec![
                PCKVariable {
                    name: "BODY399_RADII".to_string(),
                    values: vec![
                        KernelValue::Numeric(6378.14),
                        KernelValue::Numeric(6378.14),
                        KernelValue::Numeric(6356.75),
                    ],
                },
                PCKVariable {
                    name: "BODY399_GM".to_string(),
                    values: vec![KernelValue::Numeric(398600.435)],
                },
                PCKVariable {
                    name: "FRAME_NAME".to_string(),
                    values: vec![KernelValue::Text("J2000".to_string())],
                },
                PCKVariable {
                    name: "DELTET/DELTA_AT".to_string(),
                    values: vec![
                        KernelValue::Numeric(10.0),
                        KernelValue::Epoch("@1972-JAN-1".to_string()),
                        KernelValue::Numeric(11.0),
                        KernelValue::Epoch("@1972-JUL-1".to_string()),
                    ],
                },
            ])],
        };

        SpiceKernel {
            daf_sources: Vec::new(),
            pck_sources: vec![pck],
            coverage_index: CoverageIndex::new(),
        }
    }

    #[test]
    fn test_get_f64() {
        let kernel = make_test_kernel();

        let radii = kernel.get_f64("BODY399_RADII").unwrap();
        assert_eq!(radii.len(), 3);
        assert!((radii[0] - 6378.14).abs() < 1e-10);
        assert!((radii[2] - 6356.75).abs() < 1e-10);
    }

    #[test]
    fn test_get_f64_scalar() {
        let kernel = make_test_kernel();

        let gm = kernel.get_f64_scalar("BODY399_GM").unwrap();
        assert!((gm - 398600.435).abs() < 1e-6);
    }

    #[test]
    fn test_get_i32() {
        let kernel = make_test_kernel();

        // Numeric values can be converted to i32
        let radii = kernel.get_i32("BODY399_RADII").unwrap();
        assert_eq!(radii[0], 6378);
    }

    #[test]
    fn test_get_strings() {
        let kernel = make_test_kernel();

        let names = kernel.get_strings("FRAME_NAME").unwrap();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "J2000");
    }

    #[test]
    fn test_pool_has() {
        let kernel = make_test_kernel();

        assert!(kernel.pool_has("BODY399_RADII"));
        assert!(kernel.pool_has("body399_radii")); // Case-insensitive
        assert!(!kernel.pool_has("NONEXISTENT_VAR"));
    }

    #[test]
    fn test_pool_count() {
        let kernel = make_test_kernel();

        assert_eq!(kernel.pool_count("BODY399_RADII"), Some(3));
        assert_eq!(kernel.pool_count("BODY399_GM"), Some(1));
        assert_eq!(kernel.pool_count("NONEXISTENT_VAR"), None);
    }

    #[test]
    fn test_get_f64_on_mixed_values() {
        let kernel = make_test_kernel();

        // DELTET/DELTA_AT has mixed values - should return None for get_f64
        let result = kernel.get_f64("DELTET/DELTA_AT");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_strings_on_epochs() {
        let kernel = make_test_kernel();

        // DELTET/DELTA_AT has epoch strings
        let epochs = kernel.get_strings("DELTET/DELTA_AT").unwrap();
        assert_eq!(epochs.len(), 2);
        assert_eq!(epochs[0], "@1972-JAN-1");
        assert_eq!(epochs[1], "@1972-JUL-1");
    }
}
