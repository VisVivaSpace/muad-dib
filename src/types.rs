//! Type-safe newtypes for DAF parsing.
//!
//! These types provide compile-time safety for commonly confused values
//! like DAF addresses vs byte offsets, and NAIF identifiers.

use std::fmt;

/// DAF double-word address (1-indexed).
///
/// DAF files use 1-indexed addresses where each address unit represents
/// 8 bytes (one double-precision float). To convert to a byte offset,
/// use `to_byte_offset()`.
///
/// # Example
///
/// ```
/// use muad_dib::types::DafAddress;
///
/// let addr = DafAddress(129);  // First data address after file record
/// assert_eq!(addr.to_byte_offset(), 1024);  // (129-1) * 8 = 1024 bytes
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DafAddress(pub u64);

impl DafAddress {
    /// Convert DAF address to byte offset.
    ///
    /// DAF addresses are 1-indexed double-word (8-byte) indices.
    /// Byte offset = (address - 1) * 8
    #[inline]
    pub fn to_byte_offset(self) -> u64 {
        (self.0 - 1) * 8
    }

    /// Create a DafAddress from a byte offset.
    ///
    /// Byte offset must be divisible by 8.
    #[inline]
    pub fn from_byte_offset(offset: u64) -> Self {
        debug_assert!(
            offset.is_multiple_of(8),
            "Byte offset must be divisible by 8"
        );
        DafAddress((offset / 8) + 1)
    }
}

impl From<u64> for DafAddress {
    fn from(value: u64) -> Self {
        DafAddress(value)
    }
}

impl From<i32> for DafAddress {
    fn from(value: i32) -> Self {
        DafAddress(value as u64)
    }
}

impl fmt::Display for DafAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// TDB seconds past J2000 epoch.
///
/// This newtype wraps epoch values as used in SPK and BPCK files.
/// J2000 epoch is January 1, 2000, 12:00:00 TDB.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct EpochTDB(pub f64);

impl EpochTDB {
    /// J2000 epoch (TDB = 0)
    pub const J2000: EpochTDB = EpochTDB(0.0);

    /// Create from TDB seconds past J2000.
    #[inline]
    pub fn from_tdb_seconds(seconds: f64) -> Self {
        EpochTDB(seconds)
    }

    /// Get as TDB seconds past J2000.
    #[inline]
    pub fn as_tdb_seconds(self) -> f64 {
        self.0
    }
}

impl From<f64> for EpochTDB {
    fn from(value: f64) -> Self {
        EpochTDB(value)
    }
}

impl fmt::Display for EpochTDB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} TDB", self.0)
    }
}

/// Spacecraft clock ticks.
///
/// SCLK (Spacecraft Clock) times are instrument-specific tick counts,
/// not convertible to TDB without a SCLK kernel for the specific spacecraft.
/// This type provides compile-time safety to prevent accidental confusion
/// with TDB seconds.
///
/// # Example
///
/// ```
/// use muad_dib::types::Sclk;
///
/// let sclk = Sclk::from_ticks(123456789.0);
/// assert_eq!(sclk.as_ticks(), 123456789.0);
/// println!("{}", sclk);  // "123456789 SCLK"
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Sclk(pub f64);

impl Sclk {
    /// Create from SCLK ticks.
    #[inline]
    pub fn from_ticks(ticks: f64) -> Self {
        Sclk(ticks)
    }

    /// Get as SCLK ticks.
    #[inline]
    pub fn as_ticks(self) -> f64 {
        self.0
    }
}

impl From<f64> for Sclk {
    fn from(value: f64) -> Self {
        Sclk(value)
    }
}

impl fmt::Display for Sclk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} SCLK", self.0)
    }
}

/// NAIF body/frame identifier.
///
/// NAIF IDs follow conventions:
/// - Planets: x99 (e.g., 399 = Earth)
/// - Barycenters: x (e.g., 3 = Earth-Moon barycenter)
/// - Moons: x0y (e.g., 301 = Moon)
/// - Spacecraft: negative (e.g., -82 = Cassini)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NaifId(pub i32);

impl NaifId {
    /// Sun
    pub const SUN: NaifId = NaifId(10);
    /// Solar System Barycenter
    pub const SSB: NaifId = NaifId(0);
    /// Earth-Moon Barycenter
    pub const EMB: NaifId = NaifId(3);
    /// Earth
    pub const EARTH: NaifId = NaifId(399);
    /// Moon
    pub const MOON: NaifId = NaifId(301);
    /// Mars Barycenter
    pub const MARS_BC: NaifId = NaifId(4);
    /// Mars
    pub const MARS: NaifId = NaifId(499);

    /// Check if this is a spacecraft (negative ID).
    #[inline]
    pub fn is_spacecraft(self) -> bool {
        self.0 < 0
    }

    /// Check if this is a planet (x99 pattern).
    #[inline]
    pub fn is_planet(self) -> bool {
        self.0 > 0 && self.0 % 100 == 99
    }

    /// Check if this is a barycenter (single digit 1-9 or 0 for SSB).
    #[inline]
    pub fn is_barycenter(self) -> bool {
        self.0 >= 0 && self.0 <= 9
    }
}

impl From<i32> for NaifId {
    fn from(value: i32) -> Self {
        NaifId(value)
    }
}

impl fmt::Display for NaifId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daf_address_conversion() {
        let addr = DafAddress(129);
        assert_eq!(addr.to_byte_offset(), 1024);

        let addr2 = DafAddress::from_byte_offset(1024);
        assert_eq!(addr2, addr);
    }

    #[test]
    fn test_epoch_tdb() {
        let epoch = EpochTDB::from_tdb_seconds(1e9);
        assert!((epoch.as_tdb_seconds() - 1e9).abs() < 1e-10);
    }

    #[test]
    fn test_naif_id_classification() {
        assert!(NaifId(-82).is_spacecraft());
        assert!(!NaifId(-82).is_planet());

        assert!(NaifId(399).is_planet());
        assert!(!NaifId(399).is_spacecraft());

        assert!(NaifId(3).is_barycenter());
        assert!(NaifId(0).is_barycenter());
        assert!(!NaifId(399).is_barycenter());
    }

    #[test]
    fn test_display_daf_address() {
        let addr = DafAddress(129);
        assert_eq!(format!("{}", addr), "129");
    }

    #[test]
    fn test_display_epoch_tdb() {
        let epoch = EpochTDB(0.0);
        assert_eq!(format!("{}", epoch), "0 TDB");

        let epoch = EpochTDB(86400.0);
        assert_eq!(format!("{}", epoch), "86400 TDB");
    }

    #[test]
    fn test_display_naif_id() {
        assert_eq!(format!("{}", NaifId::EARTH), "399");
        assert_eq!(format!("{}", NaifId(-82)), "-82");
    }

    #[test]
    fn test_display_naif_id_edge_cases() {
        // Solar System Barycenter (ID 0)
        assert_eq!(format!("{}", NaifId::SSB), "0");
        assert_eq!(format!("{}", NaifId(0)), "0");

        // Large spacecraft ID
        assert_eq!(format!("{}", NaifId(-999999)), "-999999");
    }

    #[test]
    fn test_display_epoch_tdb_edge_cases() {
        // Negative epoch (before J2000)
        let before_j2000 = EpochTDB(-86400.0);
        assert_eq!(format!("{}", before_j2000), "-86400 TDB");

        // Very small epoch
        let tiny = EpochTDB(0.001);
        assert_eq!(format!("{}", tiny), "0.001 TDB");

        // Large epoch (~30 years in seconds)
        let far_future = EpochTDB(1e9);
        assert_eq!(format!("{}", far_future), "1000000000 TDB");
    }

    #[test]
    fn test_display_daf_address_edge_cases() {
        // Minimum valid address (1)
        let min_addr = DafAddress(1);
        assert_eq!(format!("{}", min_addr), "1");
        assert_eq!(min_addr.to_byte_offset(), 0);

        // Large address (> 1 billion, typical for large SPK files)
        let large_addr = DafAddress(1_500_000_000);
        assert_eq!(format!("{}", large_addr), "1500000000");

        // First data address after file record
        let first_data = DafAddress(129);
        assert_eq!(format!("{}", first_data), "129");
    }

    #[test]
    fn test_sclk_conversion() {
        let sclk = Sclk::from_ticks(123456789.0);
        assert!((sclk.as_ticks() - 123456789.0).abs() < 1e-10);
    }

    #[test]
    fn test_sclk_from_f64() {
        let sclk: Sclk = 1000.0.into();
        assert!((sclk.0 - 1000.0).abs() < 1e-10);
    }

    #[test]
    fn test_display_sclk() {
        let sclk = Sclk(123456789.0);
        assert_eq!(format!("{}", sclk), "123456789 SCLK");

        let sclk_zero = Sclk(0.0);
        assert_eq!(format!("{}", sclk_zero), "0 SCLK");
    }
}
