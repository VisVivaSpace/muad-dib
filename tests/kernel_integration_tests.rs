//! Integration tests for the SpiceKernel high-level API with real kernel files.
//!
//! These tests verify that the kernel API correctly loads, indexes, and queries
//! real SPICE kernel files of all supported types (SPK, CK, BPC, TPC).

#![cfg(feature = "test-data")]

use muad_dib::kernel::SpiceKernel;
use muad_dib::types::NaifId;

// =============================================================================
// SPK Loading and Query Tests
// =============================================================================

/// Load test.bsp and verify body IDs and coverage are accessible.
#[test]
fn test_load_spk_and_query_bodies() {
    let kernel = SpiceKernel::load("test_data/test.bsp").expect("Failed to load test.bsp");

    let bodies = kernel.spk_bodies();
    assert!(!bodies.is_empty(), "test.bsp should contain SPK bodies");

    // Every body with an ID should have coverage
    for body in &bodies {
        let coverage = kernel.spk_coverage(*body);
        assert!(coverage.is_some(), "Body {} should have coverage", body);
        let intervals = coverage.unwrap();
        assert!(
            !intervals.is_empty(),
            "Body {} should have at least one coverage interval",
            body
        );
    }
}

/// Load de440s.bsp and verify it contains expected solar system bodies.
#[test]
fn test_load_de440s_planetary_bodies() {
    let kernel = SpiceKernel::load("test_data/de440s.bsp").expect("Failed to load de440s.bsp");

    let bodies = kernel.spk_bodies();
    assert!(
        !bodies.is_empty(),
        "de440s.bsp should contain planetary bodies"
    );

    // DE440s should contain major solar system bodies
    let expected_bodies = [
        (1, "Mercury Barycenter"),
        (2, "Venus Barycenter"),
        (3, "Earth-Moon Barycenter"),
        (4, "Mars Barycenter"),
        (5, "Jupiter Barycenter"),
        (6, "Saturn Barycenter"),
        (7, "Uranus Barycenter"),
        (8, "Neptune Barycenter"),
        (9, "Pluto Barycenter"),
        (10, "Sun"),
        (301, "Moon"),
        (399, "Earth"),
    ];

    for (id, name) in &expected_bodies {
        assert!(
            bodies.contains(&NaifId(*id)),
            "de440s.bsp should contain {} (NAIF ID {})",
            name,
            id
        );
    }
}

// =============================================================================
// CK Loading and Query Tests
// =============================================================================

/// Load test.bc and verify instrument IDs and coverage are accessible.
#[test]
fn test_load_ck_and_query_instruments() {
    let kernel = SpiceKernel::load("test_data/test.bc").expect("Failed to load test.bc");

    let instruments = kernel.ck_instruments();
    assert!(
        !instruments.is_empty(),
        "test.bc should contain CK instruments"
    );

    // Every instrument should have coverage
    for inst in &instruments {
        let coverage = kernel.ck_coverage(*inst);
        assert!(
            coverage.is_some(),
            "Instrument {} should have coverage",
            inst
        );
        let intervals = coverage.unwrap();
        assert!(
            !intervals.is_empty(),
            "Instrument {} should have at least one coverage interval",
            inst
        );
    }
}

// =============================================================================
// BPC Loading and Query Tests
// =============================================================================

/// Load earth_latest_high_prec.bpc and verify it defines frame 3000.
#[test]
fn test_load_bpc_and_query_frames() {
    let kernel =
        SpiceKernel::load("test_data/earth_latest_high_prec.bpc").expect("Failed to load BPC");

    let frames = kernel.bpck_frames();
    assert!(!frames.is_empty(), "BPC should contain frames");
    assert!(
        frames.contains(&NaifId(3000)),
        "earth BPC should define frame 3000 (ITRF93)"
    );
}

// =============================================================================
// Mixed Kernel Loading Tests
// =============================================================================

/// Load SPK + CK + TPC together via builder and verify all data types accessible.
#[test]
fn test_load_mixed_kernel_types() {
    let kernel = SpiceKernel::builder()
        .file("test_data/test.bsp")
        .file("test_data/test.bc")
        .file("test_data/test.tpc")
        .build()
        .expect("Failed to build mixed kernel");

    assert!(!kernel.spk_bodies().is_empty(), "Should have SPK bodies");
    assert!(
        !kernel.ck_instruments().is_empty(),
        "Should have CK instruments"
    );
    assert!(
        !kernel.pck_body_ids().is_empty(),
        "Should have PCK body IDs"
    );
}

// =============================================================================
// Text PCK (TPC) Tests
// =============================================================================

/// Load test.tpc and verify kernel pool queries work.
#[test]
fn test_load_tpc_and_query_pool() {
    let kernel = SpiceKernel::load("test_data/test.tpc").expect("Failed to load test.tpc");

    let body_ids = kernel.pck_body_ids();
    assert!(!body_ids.is_empty(), "TPC should contain body IDs");

    // Earth should be present (body 399)
    assert!(
        body_ids.contains(&399),
        "TPC should contain Earth (399), got: {:?}",
        body_ids
    );

    // Look up Earth radii
    let radii = kernel.pck_lookup("BODY399_RADII");
    assert!(radii.is_some(), "Should find BODY399_RADII in TPC");
}

// =============================================================================
// Segment Filtering Tests
// =============================================================================

/// Load test.bsp and filter segments by body using spk_segments_for.
#[test]
fn test_segment_iteration_and_filtering() {
    let kernel = SpiceKernel::load("test_data/test.bsp").expect("Failed to load test.bsp");

    let bodies = kernel.spk_bodies();
    assert!(!bodies.is_empty());

    let target = bodies[0];
    let filtered: Vec<_> = kernel.spk_segments_for(target).collect();
    assert!(
        !filtered.is_empty(),
        "Should find segments for body {}",
        target
    );

    // All returned segments should match the target
    for seg in &filtered {
        assert_eq!(
            NaifId(seg.target_code),
            target,
            "Filtered segment should have target_code matching filter"
        );
    }
}

/// Load test.bsp and verify SpkSegmentView metadata.
#[test]
fn test_spk_view_metadata() {
    let kernel = SpiceKernel::load("test_data/test.bsp").expect("Failed to load test.bsp");

    let bodies = kernel.spk_bodies();
    assert!(!bodies.is_empty());

    let target = bodies[0];
    let views: Vec<_> = kernel.spk_views_for(target).collect();
    assert!(!views.is_empty(), "Should have views for body {}", target);

    let view = &views[0];
    assert_eq!(view.target(), target);
    assert!(view.spk_type() > 0, "SPK type should be positive");
    assert!(
        view.final_epoch() > view.initial_epoch(),
        "Final epoch should be after initial epoch"
    );

    // The midpoint epoch should be covered
    let mid = (view.initial_epoch() + view.final_epoch()) / 2.0;
    assert!(
        view.covers_epoch(mid),
        "View should cover its own midpoint epoch"
    );
}

// =============================================================================
// Coverage Index Tests
// =============================================================================

/// Load de440s.bsp and verify coverage queries return reasonable epoch ranges.
#[test]
fn test_coverage_index_with_real_data() {
    let kernel = SpiceKernel::load("test_data/de440s.bsp").expect("Failed to load de440s.bsp");

    let test_bodies = [(399, "Earth"), (301, "Moon"), (4, "Mars Barycenter")];

    for (id, name) in &test_bodies {
        let body = NaifId(*id);
        let coverage = kernel.spk_coverage(body);
        assert!(
            coverage.is_some(),
            "de440s should have coverage for {} ({})",
            name,
            id
        );

        let intervals = coverage.unwrap();
        assert!(
            !intervals.is_empty(),
            "{} should have at least one interval",
            name
        );

        // Coverage should span a reasonable time range (DE440s covers ~1549-2650 CE)
        let first = &intervals[0];
        // Epochs in TDB seconds past J2000
        // J2000 = 2000-01-01 12:00:00 TDB, so negative epochs are before J2000
        assert!(
            first.end > first.start,
            "{} coverage end should be after start",
            name
        );
    }
}
