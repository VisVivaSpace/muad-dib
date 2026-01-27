//! CK segment type parsing functions.
//!
//! Each CK type has a specific internal data structure. This module
//! provides parsing functions to convert raw `Vec<f64>` data into
//! type-specific structures.

use super::ck_types::*;

/// Parse CK segment data based on segment type and rates flag.
///
/// Returns `CkData::Raw` for unsupported types.
pub fn parse_ck_data(ck_type: i32, has_rates: bool, data: Vec<f64>) -> CkData {
    let parsed = match ck_type {
        1 => parse_type1(&data, has_rates),
        3 => parse_type3(&data, has_rates),
        _ => None,
    };

    parsed.unwrap_or(CkData::Raw {
        ck_type,
        has_rates,
        data,
    })
}

// ============================================================================
// CK Type 1: Discrete Pointing Instances
// ============================================================================

/// Parse CK Type 1 segment data.
///
/// Type 1 Layout:
/// ```text
/// Pointing records (4 or 7 elements each):
///   [q0, q1, q2, q3] or [q0, q1, q2, q3, av_x, av_y, av_z]
/// SCLK times (NPREC elements)
/// SCLK directory (INT((NPREC-1)/100) elements)
/// NPREC (1 element) - number of pointing records
/// ```
fn parse_type1(data: &[f64], has_rates: bool) -> Option<CkData> {
    if data.is_empty() {
        return None;
    }

    // Read NPREC from end
    let n = data.len();
    let nprec = data[n - 1] as usize;

    if nprec == 0 {
        return None;
    }

    // Calculate sizes
    let record_size = if has_rates { 7 } else { 4 };
    let pointing_data_size = nprec * record_size;
    let sclk_data_size = nprec;
    let dir_size = if nprec > 1 { (nprec - 1) / 100 } else { 0 };

    // Verify we have enough data
    let expected_size = pointing_data_size + sclk_data_size + dir_size + 1;
    if data.len() < expected_size {
        return None;
    }

    // Extract SCLK times (after pointing data)
    let sclk_start = pointing_data_size;
    let sclk_times: Vec<f64> = data[sclk_start..sclk_start + nprec].to_vec();

    // Parse pointing records
    let mut records = Vec::with_capacity(nprec);
    for (i, &sclk) in sclk_times.iter().enumerate() {
        let start = i * record_size;

        let record = if has_rates {
            PointingRecord {
                sclk,
                q0: data[start],
                q1: data[start + 1],
                q2: data[start + 2],
                q3: data[start + 3],
                av_x: Some(data[start + 4]),
                av_y: Some(data[start + 5]),
                av_z: Some(data[start + 6]),
            }
        } else {
            PointingRecord {
                sclk,
                q0: data[start],
                q1: data[start + 1],
                q2: data[start + 2],
                q3: data[start + 3],
                av_x: None,
                av_y: None,
                av_z: None,
            }
        };

        records.push(record);
    }

    Some(CkData::Type1(Ck1Data { has_rates, records }))
}

// ============================================================================
// CK Type 3: Linear Interpolation
// ============================================================================

/// Parse CK Type 3 segment data.
///
/// Type 3 Layout:
/// ```text
/// Pointing records (4 or 7 elements each)
/// SCLK times (NPREC elements)
/// SCLK directory (INT((NPREC-1)/100) elements)
/// Interval start times (NUMINT elements)
/// Start times directory (INT((NUMINT-1)/100) elements)
/// NUMINT (1 element) - number of intervals
/// NPREC (1 element) - number of pointing records
/// ```
fn parse_type3(data: &[f64], has_rates: bool) -> Option<CkData> {
    if data.len() < 2 {
        return None;
    }

    // Read NPREC and NUMINT from end
    let n = data.len();
    let nprec = data[n - 1] as usize;
    let numint = data[n - 2] as usize;

    if nprec == 0 || numint == 0 {
        return None;
    }

    // Calculate sizes
    let record_size = if has_rates { 7 } else { 4 };
    let pointing_data_size = nprec * record_size;
    let sclk_data_size = nprec;
    let sclk_dir_size = if nprec > 1 { (nprec - 1) / 100 } else { 0 };
    let interval_start_size = numint;
    let interval_dir_size = if numint > 1 { (numint - 1) / 100 } else { 0 };

    // Verify we have enough data
    let expected_size = pointing_data_size
        + sclk_data_size
        + sclk_dir_size
        + interval_start_size
        + interval_dir_size
        + 2;
    if data.len() < expected_size {
        return None;
    }

    // Extract SCLK times
    let sclk_start = pointing_data_size;
    let sclk_times: Vec<f64> = data[sclk_start..sclk_start + nprec].to_vec();

    // Extract interval start times
    let interval_start_offset = sclk_start + nprec + sclk_dir_size;
    let interval_starts_sclk: Vec<f64> =
        data[interval_start_offset..interval_start_offset + numint].to_vec();

    // Parse pointing records
    let mut records = Vec::with_capacity(nprec);
    for (i, &sclk) in sclk_times.iter().enumerate() {
        let start = i * record_size;

        let record = if has_rates {
            PointingRecord {
                sclk,
                q0: data[start],
                q1: data[start + 1],
                q2: data[start + 2],
                q3: data[start + 3],
                av_x: Some(data[start + 4]),
                av_y: Some(data[start + 5]),
                av_z: Some(data[start + 6]),
            }
        } else {
            PointingRecord {
                sclk,
                q0: data[start],
                q1: data[start + 1],
                q2: data[start + 2],
                q3: data[start + 3],
                av_x: None,
                av_y: None,
                av_z: None,
            }
        };

        records.push(record);
    }

    // Convert interval start SCLK times to record indices
    // Find the index of the record with the closest matching SCLK time
    let interval_starts: Vec<usize> = interval_starts_sclk
        .iter()
        .map(|&start_sclk| {
            sclk_times
                .iter()
                .position(|&t| (t - start_sclk).abs() < 1e-9)
                .unwrap_or(0)
        })
        .collect();

    Some(CkData::Type3(Ck3Data {
        has_rates,
        records,
        interval_starts,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_type1_no_rates() {
        // 2 pointing records, no angular velocity
        let data = vec![
            // Pointing records (4 elements each)
            1.0, 0.0, 0.0, 0.0, // Record 1: identity quaternion
            0.707, 0.707, 0.0, 0.0, // Record 2: 90 degree rotation
            // SCLK times
            1000.0, 2000.0, // No directory for 2 records
            // NPREC
            2.0,
        ];

        let result = parse_ck_data(1, false, data);
        if let CkData::Type1(ck1) = result {
            assert!(!ck1.has_rates);
            assert_eq!(ck1.records.len(), 2);
            assert_eq!(ck1.records[0].sclk, 1000.0);
            assert_eq!(ck1.records[0].quaternion(), [1.0, 0.0, 0.0, 0.0]);
            assert!(ck1.records[0].angular_velocity().is_none());
        } else {
            panic!("Expected Type1");
        }
    }

    #[test]
    fn test_parse_type1_with_rates() {
        // 2 pointing records with angular velocity
        let data = vec![
            // Pointing records (7 elements each)
            1.0, 0.0, 0.0, 0.0, 0.1, 0.2, 0.3, // Record 1
            0.707, 0.707, 0.0, 0.0, 0.0, 0.0, 0.1, // Record 2
            // SCLK times
            1000.0, 2000.0, // NPREC
            2.0,
        ];

        let result = parse_ck_data(1, true, data);
        if let CkData::Type1(ck1) = result {
            assert!(ck1.has_rates);
            assert_eq!(ck1.records.len(), 2);
            assert_eq!(ck1.records[0].angular_velocity(), Some([0.1, 0.2, 0.3]));
        } else {
            panic!("Expected Type1");
        }
    }

    #[test]
    fn test_parse_type3() {
        // 3 pointing records in 2 intervals
        let data = vec![
            // Pointing records (4 elements each)
            1.0, 0.0, 0.0, 0.0, // Record 0
            0.707, 0.707, 0.0, 0.0, // Record 1
            0.5, 0.5, 0.5, 0.5, // Record 2
            // SCLK times
            1000.0, 2000.0, 3000.0, // Interval start times
            1000.0, 2000.0, // NUMINT and NPREC
            2.0, 3.0,
        ];

        let result = parse_ck_data(3, false, data);
        if let CkData::Type3(ck3) = result {
            assert!(!ck3.has_rates);
            assert_eq!(ck3.records.len(), 3);
            assert_eq!(ck3.interval_starts.len(), 2);
            assert_eq!(ck3.interval_starts[0], 0); // First interval starts at record 0
            assert_eq!(ck3.interval_starts[1], 1); // Second interval starts at record 1
        } else {
            panic!("Expected Type3");
        }
    }

    #[test]
    fn test_parse_unknown_type() {
        let data = vec![1.0, 2.0, 3.0];
        let result = parse_ck_data(99, false, data.clone());

        if let CkData::Raw {
            ck_type,
            has_rates,
            data: d,
        } = result
        {
            assert_eq!(ck_type, 99);
            assert!(!has_rates);
            assert_eq!(d, data);
        } else {
            panic!("Expected Raw");
        }
    }
}
