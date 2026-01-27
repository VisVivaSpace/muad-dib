//! NAIF body and frame name lookup.
//!
//! Provides a static lookup table for common NAIF body IDs and frame IDs.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Static lookup table for common NAIF body IDs.
static BODY_NAMES: LazyLock<HashMap<i32, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Barycenters (0-9)
    m.insert(0, "SOLAR SYSTEM BARYCENTER");
    m.insert(1, "MERCURY BARYCENTER");
    m.insert(2, "VENUS BARYCENTER");
    m.insert(3, "EARTH BARYCENTER");
    m.insert(4, "MARS BARYCENTER");
    m.insert(5, "JUPITER BARYCENTER");
    m.insert(6, "SATURN BARYCENTER");
    m.insert(7, "URANUS BARYCENTER");
    m.insert(8, "NEPTUNE BARYCENTER");
    m.insert(9, "PLUTO BARYCENTER");

    // Sun
    m.insert(10, "SUN");

    // Planets (100s)
    m.insert(199, "MERCURY");
    m.insert(299, "VENUS");
    m.insert(399, "EARTH");
    m.insert(499, "MARS");
    m.insert(599, "JUPITER");
    m.insert(699, "SATURN");
    m.insert(799, "URANUS");
    m.insert(899, "NEPTUNE");
    m.insert(999, "PLUTO");

    // Earth system
    m.insert(301, "MOON");

    // Mars system
    m.insert(401, "PHOBOS");
    m.insert(402, "DEIMOS");

    // Jupiter system
    m.insert(501, "IO");
    m.insert(502, "EUROPA");
    m.insert(503, "GANYMEDE");
    m.insert(504, "CALLISTO");
    m.insert(505, "AMALTHEA");

    // Saturn system
    m.insert(601, "MIMAS");
    m.insert(602, "ENCELADUS");
    m.insert(603, "TETHYS");
    m.insert(604, "DIONE");
    m.insert(605, "RHEA");
    m.insert(606, "TITAN");
    m.insert(607, "HYPERION");
    m.insert(608, "IAPETUS");

    // Uranus system
    m.insert(701, "ARIEL");
    m.insert(702, "UMBRIEL");
    m.insert(703, "TITANIA");
    m.insert(704, "OBERON");
    m.insert(705, "MIRANDA");

    // Neptune system
    m.insert(801, "TRITON");
    m.insert(802, "NEREID");

    // Pluto system
    m.insert(901, "CHARON");

    m
});

/// Static lookup table for common NAIF frame IDs.
static FRAME_NAMES: LazyLock<HashMap<i32, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Inertial frames
    m.insert(1, "J2000");
    m.insert(2, "B1950");
    m.insert(17, "ECLIPJ2000");
    m.insert(18, "ECLIPB1950");

    // Body-fixed frames (IAU)
    m.insert(10010, "IAU_SUN");
    m.insert(10011, "IAU_MERCURY");
    m.insert(10012, "IAU_VENUS");
    m.insert(10013, "IAU_EARTH");
    m.insert(10014, "IAU_MARS");
    m.insert(10015, "IAU_JUPITER");
    m.insert(10016, "IAU_SATURN");
    m.insert(10017, "IAU_URANUS");
    m.insert(10018, "IAU_NEPTUNE");
    m.insert(10019, "IAU_PLUTO");
    m.insert(10020, "IAU_MOON");

    m
});

/// Static lookup table for spacecraft IDs.
/// These are negative IDs that identify spacecraft in CK files.
static SPACECRAFT_NAMES: LazyLock<HashMap<i32, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Voyager
    m.insert(-31, "VOYAGER 1");
    m.insert(-32, "VOYAGER 2");

    // Pioneer
    m.insert(-20, "PIONEER 10");
    m.insert(-21, "PIONEER 11");

    // Galileo
    m.insert(-77, "GALILEO ORBITER");

    // Spitzer (SIRTF)
    m.insert(-79, "SPITZER");

    // Cassini
    m.insert(-82, "CASSINI");

    // Mars missions
    m.insert(-94, "MARS OBSERVER");
    m.insert(-53, "MARS PATHFINDER");
    m.insert(-76, "MARS SCIENCE LABORATORY");
    m.insert(-168, "JUNO");
    m.insert(-140, "DEEP IMPACT");
    m.insert(-98, "NEW HORIZONS");
    m.insert(-64, "OSIRIS-REX");
    m.insert(-236, "LUCY");
    m.insert(-234, "PSYCHE");

    // Mars orbiters
    m.insert(-41, "MARS EXPRESS");
    m.insert(-74, "MARS RECONNAISSANCE ORBITER");
    m.insert(-202, "MAVEN");

    // Space telescopes
    m.insert(-48, "HUBBLE SPACE TELESCOPE");
    m.insert(-170, "JWST");

    // NEAR
    m.insert(-93, "NEAR");

    // Messenger
    m.insert(-236, "MESSENGER");

    // Dawn
    m.insert(-203, "DAWN");

    // Solar probes
    m.insert(-96, "PARKER SOLAR PROBE");
    m.insert(-144, "SOLAR ORBITER");

    // Europa Clipper
    m.insert(-159, "EUROPA CLIPPER");

    m
});

/// Look up a body name by NAIF ID.
/// Returns the name if found, otherwise None.
pub fn body_name(id: i32) -> Option<&'static str> {
    BODY_NAMES.get(&id).copied()
}

/// Look up a frame name by NAIF ID.
/// Returns the name if found, otherwise None.
pub fn frame_name(id: i32) -> Option<&'static str> {
    FRAME_NAMES.get(&id).copied()
}

/// Look up a spacecraft name by NAIF ID.
/// Returns the name if found, otherwise None.
pub fn spacecraft_name(id: i32) -> Option<&'static str> {
    SPACECRAFT_NAMES.get(&id).copied()
}

/// Derive spacecraft ID from instrument code.
/// CK instrument codes are typically SC_ID * 1000 + instrument_number.
/// For example, Cassini (-82) has instruments -82000, -82001, etc.
fn spacecraft_id_from_instrument(instrument_code: i32) -> i32 {
    instrument_code / 1000
}

/// Format an object ID with optional name.
/// If numeric_only is true, returns just the ID.
/// Otherwise returns "NAME (ID)" if name is known, or just "ID" if not.
pub fn format_id(id: i32, numeric_only: bool) -> String {
    if numeric_only {
        id.to_string()
    } else if let Some(name) = body_name(id) {
        format!("{} ({})", name, id)
    } else if let Some(name) = frame_name(id) {
        format!("{} ({})", name, id)
    } else {
        id.to_string()
    }
}

/// Format an instrument ID with optional spacecraft name.
/// If numeric_only is true, returns just the ID.
/// Otherwise attempts to derive the spacecraft from the instrument code.
pub fn format_instrument_id(instrument_code: i32, numeric_only: bool) -> String {
    if numeric_only {
        instrument_code.to_string()
    } else {
        // Try to derive spacecraft from instrument code
        let sc_id = spacecraft_id_from_instrument(instrument_code);
        if let Some(sc_name) = spacecraft_name(sc_id) {
            format!("{} ({})", sc_name, instrument_code)
        } else {
            // Fall back to just the ID
            instrument_code.to_string()
        }
    }
}

/// Format a frame ID with optional name.
/// If numeric_only is true, returns just the ID.
/// Otherwise returns "NAME (ID)" if name is known, or just "ID" if not.
pub fn format_frame_id(frame_id: i32, numeric_only: bool) -> String {
    if numeric_only {
        frame_id.to_string()
    } else if let Some(name) = frame_name(frame_id) {
        format!("{} ({})", name, frame_id)
    } else {
        frame_id.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body_lookup() {
        assert_eq!(body_name(399), Some("EARTH"));
        assert_eq!(body_name(301), Some("MOON"));
        assert_eq!(body_name(999999), None);
    }

    #[test]
    fn test_frame_lookup() {
        assert_eq!(frame_name(1), Some("J2000"));
        assert_eq!(frame_name(999999), None);
    }

    #[test]
    fn test_format_id() {
        assert_eq!(format_id(399, false), "EARTH (399)");
        assert_eq!(format_id(399, true), "399");
        assert_eq!(format_id(12345, false), "12345");
    }

    #[test]
    fn test_spacecraft_lookup() {
        assert_eq!(spacecraft_name(-82), Some("CASSINI"));
        assert_eq!(spacecraft_name(-31), Some("VOYAGER 1"));
        assert_eq!(spacecraft_name(999), None);
    }

    #[test]
    fn test_format_instrument_id() {
        // Cassini instrument -82000 should show CASSINI
        assert_eq!(format_instrument_id(-82000, false), "CASSINI (-82000)");
        assert_eq!(format_instrument_id(-82000, true), "-82000");

        // Unknown spacecraft
        assert_eq!(format_instrument_id(-99000, false), "-99000");
    }

    #[test]
    fn test_format_frame_id() {
        assert_eq!(format_frame_id(1, false), "J2000 (1)");
        assert_eq!(format_frame_id(1, true), "1");
        assert_eq!(format_frame_id(99999, false), "99999");
    }
}
