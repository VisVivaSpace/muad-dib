//! SPK segment type parsing functions.
//!
//! Each SPK type has a specific internal data structure. This module
//! provides parsing functions to convert raw `Vec<f64>` data into
//! type-specific structures.

use super::spk_types::*;

/// Parse SPK segment data based on segment type.
///
/// Returns `SpkData::Raw` for unsupported types.
pub fn parse_spk_data(spk_type: i32, data: Vec<f64>) -> SpkData {
    let parsed = match spk_type {
        2 => parse_type2(&data),
        3 => parse_type3(&data),
        5 => parse_type5(&data),
        8 => parse_type8(&data),
        9 => parse_type9(&data),
        13 => parse_type13(&data),
        _ => None,
    };

    parsed.unwrap_or(SpkData::Raw { spk_type, data })
}

// ============================================================================
// SPK Type 2: Chebyshev Position Only
// ============================================================================

/// Parse SPK Type 2 segment data.
///
/// Type 2 Layout:
/// ```text
/// Record 1: [MID, RADIUS, X_coeffs..., Y_coeffs..., Z_coeffs...]
/// Record 2: [MID, RADIUS, X_coeffs..., Y_coeffs..., Z_coeffs...]
/// ...
/// Record N: [MID, RADIUS, X_coeffs..., Y_coeffs..., Z_coeffs...]
/// INIT     - Initial epoch of first record
/// INTLEN   - Interval length (seconds)
/// RSIZE    - Record size (elements per record)
/// N        - Number of records
/// ```
fn parse_type2(data: &[f64]) -> Option<SpkData> {
    if data.len() < 4 {
        return None;
    }

    // Read directory from end of segment
    let n = data.len();
    let num_records = data[n - 1] as usize;
    let rsize = data[n - 2] as usize;
    let intlen = data[n - 3];
    let init = data[n - 4];

    if num_records == 0 || rsize < 8 {
        return None;
    }

    // Calculate polynomial degree
    // RSIZE = 2 (MID, RADIUS) + 3 * (degree + 1)
    // So: degree = (RSIZE - 2) / 3 - 1
    let coeffs_per_axis = (rsize - 2) / 3;
    if coeffs_per_axis == 0 {
        return None;
    }
    let degree = (coeffs_per_axis - 1) as u32;

    // Parse records
    let mut records = Vec::with_capacity(num_records);
    let data_end = n - 4; // Exclude directory

    for i in 0..num_records {
        let start = i * rsize;
        if start + rsize > data_end {
            break;
        }

        let midpoint = data[start];
        let radius = data[start + 1];

        let coeff_start = start + 2;
        let x_coeffs = data[coeff_start..coeff_start + coeffs_per_axis].to_vec();
        let y_coeffs = data[coeff_start + coeffs_per_axis..coeff_start + 2 * coeffs_per_axis].to_vec();
        let z_coeffs =
            data[coeff_start + 2 * coeffs_per_axis..coeff_start + 3 * coeffs_per_axis].to_vec();

        records.push(ChebyshevRecord {
            midpoint,
            radius,
            x_coeffs,
            y_coeffs,
            z_coeffs,
        });
    }

    Some(SpkData::Type2(Spk2Data {
        init_epoch: init,
        interval_length: intlen,
        degree,
        records,
    }))
}

// ============================================================================
// SPK Type 3: Chebyshev Position and Velocity
// ============================================================================

/// Parse SPK Type 3 segment data.
///
/// Type 3 is similar to Type 2 but includes velocity coefficients.
/// ```text
/// Record: [MID, RADIUS, X_pos..., Y_pos..., Z_pos..., X_vel..., Y_vel..., Z_vel...]
/// Directory: [INIT, INTLEN, RSIZE, N]
/// ```
fn parse_type3(data: &[f64]) -> Option<SpkData> {
    if data.len() < 4 {
        return None;
    }

    // Read directory from end
    let n = data.len();
    let num_records = data[n - 1] as usize;
    let rsize = data[n - 2] as usize;
    let intlen = data[n - 3];
    let init = data[n - 4];

    if num_records == 0 || rsize < 14 {
        return None;
    }

    // RSIZE = 2 (MID, RADIUS) + 6 * (degree + 1)
    // degree = (RSIZE - 2) / 6 - 1
    let coeffs_per_axis = (rsize - 2) / 6;
    if coeffs_per_axis == 0 {
        return None;
    }
    let degree = (coeffs_per_axis - 1) as u32;

    let mut records = Vec::with_capacity(num_records);
    let data_end = n - 4;

    for i in 0..num_records {
        let start = i * rsize;
        if start + rsize > data_end {
            break;
        }

        let midpoint = data[start];
        let radius = data[start + 1];

        let coeff_start = start + 2;
        let x_coeffs = data[coeff_start..coeff_start + coeffs_per_axis].to_vec();
        let y_coeffs = data[coeff_start + coeffs_per_axis..coeff_start + 2 * coeffs_per_axis].to_vec();
        let z_coeffs =
            data[coeff_start + 2 * coeffs_per_axis..coeff_start + 3 * coeffs_per_axis].to_vec();
        let vx_coeffs =
            data[coeff_start + 3 * coeffs_per_axis..coeff_start + 4 * coeffs_per_axis].to_vec();
        let vy_coeffs =
            data[coeff_start + 4 * coeffs_per_axis..coeff_start + 5 * coeffs_per_axis].to_vec();
        let vz_coeffs =
            data[coeff_start + 5 * coeffs_per_axis..coeff_start + 6 * coeffs_per_axis].to_vec();

        records.push(ChebyshevRecordWithVelocity {
            midpoint,
            radius,
            x_coeffs,
            y_coeffs,
            z_coeffs,
            vx_coeffs,
            vy_coeffs,
            vz_coeffs,
        });
    }

    Some(SpkData::Type3(Spk3Data {
        init_epoch: init,
        interval_length: intlen,
        degree,
        records,
    }))
}

// ============================================================================
// SPK Type 5: Discrete States with Two-Body Propagation
// ============================================================================

/// Parse SPK Type 5 segment data.
///
/// Type 5 Layout:
/// ```text
/// State 1: [epoch, x, y, z, vx, vy, vz]
/// State 2: [epoch, x, y, z, vx, vy, vz]
/// ...
/// State N: [epoch, x, y, z, vx, vy, vz]
/// GM      - Gravitational parameter
/// N       - Number of states
/// ```
fn parse_type5(data: &[f64]) -> Option<SpkData> {
    if data.len() < 2 {
        return None;
    }

    let n = data.len();
    let num_states = data[n - 1] as usize;
    let gm = data[n - 2];

    if num_states == 0 {
        return None;
    }

    let record_size = 7; // epoch + 6 state components
    let expected_data = num_states * record_size + 2; // + GM + N
    if data.len() < expected_data {
        return None;
    }

    let mut states = Vec::with_capacity(num_states);
    for i in 0..num_states {
        let start = i * record_size;
        states.push(StateRecord {
            epoch: data[start],
            x: data[start + 1],
            y: data[start + 2],
            z: data[start + 3],
            vx: data[start + 4],
            vy: data[start + 5],
            vz: data[start + 6],
        });
    }

    Some(SpkData::Type5(Spk5Data { gm, states }))
}

// ============================================================================
// SPK Type 8: Lagrange Interpolation (Equal Time Steps)
// ============================================================================

/// Parse SPK Type 8 segment data.
///
/// Type 8 Layout:
/// ```text
/// State 1: [x, y, z, vx, vy, vz]
/// State 2: [x, y, z, vx, vy, vz]
/// ...
/// State N: [x, y, z, vx, vy, vz]
/// START_EPOCH - Epoch of first state
/// STEP_SIZE   - Time step between states
/// WINDOW_SIZE - Number of states for interpolation
/// N           - Number of states
/// ```
fn parse_type8(data: &[f64]) -> Option<SpkData> {
    if data.len() < 4 {
        return None;
    }

    let n = data.len();
    let num_states = data[n - 1] as usize;
    let window_size = data[n - 2] as u32;
    let step_size = data[n - 3];
    let start_epoch = data[n - 4];

    if num_states == 0 {
        return None;
    }

    let record_size = 6; // 6 state components
    let expected_data = num_states * record_size + 4;
    if data.len() < expected_data {
        return None;
    }

    let mut states = Vec::with_capacity(num_states);
    for i in 0..num_states {
        let start = i * record_size;
        let epoch = start_epoch + (i as f64) * step_size;
        states.push(StateRecord {
            epoch,
            x: data[start],
            y: data[start + 1],
            z: data[start + 2],
            vx: data[start + 3],
            vy: data[start + 4],
            vz: data[start + 5],
        });
    }

    Some(SpkData::Type8(Spk8Data {
        start_epoch,
        step_size,
        window_size,
        states,
    }))
}

// ============================================================================
// SPK Type 9: Lagrange Interpolation (Unequal Time Steps)
// ============================================================================

/// Parse SPK Type 9 segment data.
///
/// Type 9 Layout:
/// ```text
/// State 1: [x, y, z, vx, vy, vz]
/// State 2: [x, y, z, vx, vy, vz]
/// ...
/// State N: [x, y, z, vx, vy, vz]
/// Epoch 1
/// Epoch 2
/// ...
/// Epoch N
/// Epoch directory (floor((N-1)/100) elements, if N > 100)
/// WINDOW_SIZE - Number of states for interpolation
/// N           - Number of states
/// ```
fn parse_type9(data: &[f64]) -> Option<SpkData> {
    if data.len() < 2 {
        return None;
    }

    let n = data.len();
    let num_states = data[n - 1] as usize;
    let window_size = data[n - 2] as u32;

    if num_states == 0 {
        return None;
    }

    let state_size = 6;
    // Epoch directory size: floor((N-1)/100) elements when N > 100
    let epoch_dir_size = if num_states > 100 { (num_states - 1) / 100 } else { 0 };
    let expected_data = num_states * state_size + num_states + epoch_dir_size + 2;
    if data.len() < expected_data {
        return None;
    }

    // Read epochs (after states, before epoch directory and final directory)
    let epochs_start = num_states * state_size;
    let epochs_end = epochs_start + num_states;
    let epochs: Vec<f64> = data[epochs_start..epochs_end].to_vec();

    if epochs.len() != num_states {
        return None;
    }

    let mut states = Vec::with_capacity(num_states);
    for (i, &epoch) in epochs.iter().enumerate() {
        let start = i * state_size;
        states.push(StateRecord {
            epoch,
            x: data[start],
            y: data[start + 1],
            z: data[start + 2],
            vx: data[start + 3],
            vy: data[start + 4],
            vz: data[start + 5],
        });
    }

    Some(SpkData::Type9(Spk9Data {
        window_size,
        states,
    }))
}

// ============================================================================
// SPK Type 13: Hermite Interpolation (Unequal Time Steps)
// ============================================================================

/// Parse SPK Type 13 segment data.
///
/// Type 13 has the same layout as Type 9 but uses Hermite interpolation.
/// Includes an epoch directory when N > 100.
fn parse_type13(data: &[f64]) -> Option<SpkData> {
    if data.len() < 2 {
        return None;
    }

    let n = data.len();
    let num_states = data[n - 1] as usize;
    let window_size = data[n - 2] as u32;

    if num_states == 0 {
        return None;
    }

    let state_size = 6;
    // Epoch directory size: floor((N-1)/100) elements when N > 100
    let epoch_dir_size = if num_states > 100 { (num_states - 1) / 100 } else { 0 };
    let expected_data = num_states * state_size + num_states + epoch_dir_size + 2;
    if data.len() < expected_data {
        return None;
    }

    // Read epochs (after states, before epoch directory and final directory)
    let epochs_start = num_states * state_size;
    let epochs_end = epochs_start + num_states;
    let epochs: Vec<f64> = data[epochs_start..epochs_end].to_vec();

    if epochs.len() != num_states {
        return None;
    }

    let mut states = Vec::with_capacity(num_states);
    for (i, &epoch) in epochs.iter().enumerate() {
        let start = i * state_size;
        states.push(StateRecord {
            epoch,
            x: data[start],
            y: data[start + 1],
            z: data[start + 2],
            vx: data[start + 3],
            vy: data[start + 4],
            vz: data[start + 5],
        });
    }

    Some(SpkData::Type13(Spk13Data {
        window_size,
        states,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_type2() {
        // Minimal Type 2 data: 1 record with degree 1 (2 coefficients per axis)
        // Record size = 2 (MID, RADIUS) + 3 * 2 = 8
        let data = vec![
            // Record 1
            100.0,    // MID
            50.0,     // RADIUS
            1.0, 2.0, // X coefficients
            3.0, 4.0, // Y coefficients
            5.0, 6.0, // Z coefficients
            // Directory
            0.0,   // INIT
            100.0, // INTLEN
            8.0,   // RSIZE
            1.0,   // N
        ];

        let result = parse_type2(&data);
        assert!(result.is_some());

        if let Some(SpkData::Type2(spk2)) = result {
            assert_eq!(spk2.init_epoch, 0.0);
            assert_eq!(spk2.interval_length, 100.0);
            assert_eq!(spk2.degree, 1); // 2 coefficients = degree 1
            assert_eq!(spk2.records.len(), 1);

            let rec = &spk2.records[0];
            assert_eq!(rec.midpoint, 100.0);
            assert_eq!(rec.radius, 50.0);
            assert_eq!(rec.x_coeffs, vec![1.0, 2.0]);
            assert_eq!(rec.y_coeffs, vec![3.0, 4.0]);
            assert_eq!(rec.z_coeffs, vec![5.0, 6.0]);
        } else {
            panic!("Expected Type2");
        }
    }

    #[test]
    fn test_parse_type5() {
        // Type 5 data: 2 states with GM
        let data = vec![
            // State 1
            0.0,
            1.0,
            2.0,
            3.0,
            0.1,
            0.2,
            0.3, // epoch, x, y, z, vx, vy, vz
            // State 2
            100.0,
            4.0,
            5.0,
            6.0,
            0.4,
            0.5,
            0.6, // epoch, x, y, z, vx, vy, vz
            // Directory
            398600.4418, // GM (Earth)
            2.0,         // N
        ];

        let result = parse_type5(&data);
        assert!(result.is_some());

        if let Some(SpkData::Type5(spk5)) = result {
            assert!((spk5.gm - 398600.4418).abs() < 1e-4);
            assert_eq!(spk5.states.len(), 2);
            assert_eq!(spk5.states[0].epoch, 0.0);
            assert_eq!(spk5.states[1].epoch, 100.0);
        } else {
            panic!("Expected Type5");
        }
    }

    #[test]
    fn test_parse_type8() {
        // Type 8 data: 2 equally spaced states
        let data = vec![
            // State 1
            1.0, 2.0, 3.0, 0.1, 0.2, 0.3, // x, y, z, vx, vy, vz
            // State 2
            4.0, 5.0, 6.0, 0.4, 0.5, 0.6, // Directory
            0.0,   // START_EPOCH
            100.0, // STEP_SIZE
            2.0,   // WINDOW_SIZE
            2.0,   // N
        ];

        let result = parse_type8(&data);
        assert!(result.is_some());

        if let Some(SpkData::Type8(spk8)) = result {
            assert_eq!(spk8.start_epoch, 0.0);
            assert_eq!(spk8.step_size, 100.0);
            assert_eq!(spk8.window_size, 2);
            assert_eq!(spk8.states.len(), 2);
            assert_eq!(spk8.states[0].epoch, 0.0);
            assert_eq!(spk8.states[1].epoch, 100.0);
        } else {
            panic!("Expected Type8");
        }
    }

    #[test]
    fn test_parse_type9() {
        // Type 9 data: 2 unequally spaced states
        let data = vec![
            // State 1
            1.0, 2.0, 3.0, 0.1, 0.2, 0.3, // x, y, z, vx, vy, vz
            // State 2
            4.0, 5.0, 6.0, 0.4, 0.5, 0.6, // Epochs
            0.0,   // Epoch 1
            150.0, // Epoch 2
            // Directory
            2.0, // WINDOW_SIZE
            2.0, // N
        ];

        let result = parse_type9(&data);
        assert!(result.is_some());

        if let Some(SpkData::Type9(spk9)) = result {
            assert_eq!(spk9.window_size, 2);
            assert_eq!(spk9.states.len(), 2);
            assert_eq!(spk9.states[0].epoch, 0.0);
            assert_eq!(spk9.states[1].epoch, 150.0);
        } else {
            panic!("Expected Type9");
        }
    }

    #[test]
    fn test_parse_unknown_type() {
        let data = vec![1.0, 2.0, 3.0];
        let result = parse_spk_data(99, data.clone());

        if let SpkData::Raw { spk_type, data: d } = result {
            assert_eq!(spk_type, 99);
            assert_eq!(d, data);
        } else {
            panic!("Expected Raw");
        }
    }
}
