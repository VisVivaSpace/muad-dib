//! Interpolation algorithms for SPK and CK data.
//!
//! This module provides:
//! - `State` and `Pointing` types for interpolation results
//! - Chebyshev polynomial evaluation (SPK Types 2, 3)
//! - Lagrange interpolation (SPK Types 8, 9)
//! - Hermite interpolation (SPK Type 13)
//! - Two-body propagation (SPK Type 5)
//!
//! # State Evaluation
//!
//! ```ignore
//! use muad_dib::spice::interpolate::{State, chebyshev};
//!
//! // Evaluate Chebyshev polynomials at an epoch
//! let state = chebyshev::evaluate_type2(&spk2_data, epoch)?;
//! println!("Position: {:?} km", state.position);
//! println!("Velocity: {:?} km/s", state.velocity);
//! ```

pub mod chebyshev;
pub mod hermite;
pub mod lagrange;
pub mod twobody;

use serde::{Deserialize, Serialize};
use std::ops::{Add, Neg, Sub};

use crate::types::NaifId;

/// State vector: position and velocity with full relativity context.
///
/// A state vector is only meaningful when you know:
/// - **target**: What body this state describes
/// - **center**: The origin of the coordinate system (position is relative to this)
/// - **frame**: The reference frame defining the axes orientation
///
/// Position is in km, velocity is in km/s.
///
/// # Relativity Principle
///
/// All positions and velocities are *relative*. A position `[1e8, 0, 0]` km means
/// nothing without knowing "relative to what?" (center) and "in which frame?" (frame).
///
/// # State Arithmetic
///
/// States support arithmetic operations for chain traversals and relative motion:
///
/// - **Addition** (chain traversal): `(SSB→Earth) + (Earth→Moon) = SSB→Moon`
///   - Requires: same frame, `self.target == other.center`
///
/// - **Subtraction** (relative motion): `(Mars rel SSB) - (Earth rel SSB) = Mars rel Earth`
///   - Requires: same frame, same center
///
/// - **Negation** (reverse direction): `-(Earth→Moon) = Moon→Earth`
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Target body this state describes
    pub target: NaifId,
    /// Center body (origin of coordinates)
    pub center: NaifId,
    /// Reference frame code (NAIF frame ID, e.g., 1 = J2000)
    pub frame: i32,
    /// Position vector [x, y, z] in km
    pub position: [f64; 3],
    /// Velocity vector [vx, vy, vz] in km/s
    pub velocity: [f64; 3],
}

impl State {
    /// Create a new state with full context.
    ///
    /// # Arguments
    ///
    /// * `target` - Body this state describes
    /// * `center` - Origin of coordinates
    /// * `frame` - Reference frame code (e.g., 1 = J2000)
    /// * `position` - Position vector [x, y, z] in km
    /// * `velocity` - Velocity vector [vx, vy, vz] in km/s
    #[inline]
    pub fn new(
        target: NaifId,
        center: NaifId,
        frame: i32,
        position: [f64; 3],
        velocity: [f64; 3],
    ) -> Self {
        State {
            target,
            center,
            frame,
            position,
            velocity,
        }
    }

    /// Create a raw state without relativity context.
    ///
    /// Use this only for intermediate calculations where context will be
    /// added later. The target, center, and frame are set to placeholder
    /// values (NaifId(0), NaifId(0), 0).
    ///
    /// # Warning
    ///
    /// States created with this method should NOT be used in arithmetic
    /// operations or returned to users without adding proper context.
    #[inline]
    pub(crate) fn new_raw(position: [f64; 3], velocity: [f64; 3]) -> Self {
        State {
            target: NaifId(0),
            center: NaifId(0),
            frame: 0,
            position,
            velocity,
        }
    }

    /// Create a state with zero velocity.
    ///
    /// # Arguments
    ///
    /// * `target` - Body this state describes
    /// * `center` - Origin of coordinates
    /// * `frame` - Reference frame code
    /// * `position` - Position vector [x, y, z] in km
    #[inline]
    pub fn from_position(target: NaifId, center: NaifId, frame: i32, position: [f64; 3]) -> Self {
        State {
            target,
            center,
            frame,
            position,
            velocity: [0.0, 0.0, 0.0],
        }
    }

    /// Get the position magnitude (distance from origin).
    #[inline]
    pub fn distance(&self) -> f64 {
        let [x, y, z] = self.position;
        (x * x + y * y + z * z).sqrt()
    }

    /// Get the velocity magnitude (speed).
    #[inline]
    pub fn speed(&self) -> f64 {
        let [vx, vy, vz] = self.velocity;
        (vx * vx + vy * vy + vz * vz).sqrt()
    }
}

/// Chain traversal: `(center→A) + (A→target) = (center→target)`
///
/// Combines two states to compute cumulative position/velocity.
/// The first state's target must match the second state's center,
/// and both must be in the same reference frame.
///
/// # Panics
///
/// Panics if:
/// - Reference frames don't match
/// - Chain is invalid (`self.target != other.center`)
///
/// # Example
///
/// ```ignore
/// // Compute Moon position relative to SSB by chaining
/// let ssb_to_earth = kernel.state_of(EARTH, epoch, SSB)?;  // SSB→Earth
/// let earth_to_moon = kernel.state_of(MOON, epoch, EARTH)?; // Earth→Moon
/// let ssb_to_moon = ssb_to_earth + earth_to_moon;           // SSB→Moon
/// ```
impl Add for State {
    type Output = State;

    fn add(self, other: State) -> State {
        assert_eq!(
            self.frame, other.frame,
            "Cannot add states in different frames: {} vs {}",
            self.frame, other.frame
        );
        assert_eq!(
            self.target, other.center,
            "Invalid chain: self.target ({}) != other.center ({})",
            self.target, other.center
        );

        State {
            target: other.target,
            center: self.center,
            frame: self.frame,
            position: [
                self.position[0] + other.position[0],
                self.position[1] + other.position[1],
                self.position[2] + other.position[2],
            ],
            velocity: [
                self.velocity[0] + other.velocity[0],
                self.velocity[1] + other.velocity[1],
                self.velocity[2] + other.velocity[2],
            ],
        }
    }
}

/// Chain traversal (borrowed RHS).
impl Add<&State> for State {
    type Output = State;

    fn add(self, other: &State) -> State {
        assert_eq!(
            self.frame, other.frame,
            "Cannot add states in different frames: {} vs {}",
            self.frame, other.frame
        );
        assert_eq!(
            self.target, other.center,
            "Invalid chain: self.target ({}) != other.center ({})",
            self.target, other.center
        );

        State {
            target: other.target,
            center: self.center,
            frame: self.frame,
            position: [
                self.position[0] + other.position[0],
                self.position[1] + other.position[1],
                self.position[2] + other.position[2],
            ],
            velocity: [
                self.velocity[0] + other.velocity[0],
                self.velocity[1] + other.velocity[1],
                self.velocity[2] + other.velocity[2],
            ],
        }
    }
}

/// Relative motion: `(center→A) - (center→B) = (B→A)`
///
/// Computes the state of A relative to B when both are relative
/// to the same center and in the same reference frame.
///
/// # Panics
///
/// Panics if:
/// - Reference frames don't match
/// - Centers don't match
///
/// # Example
///
/// ```ignore
/// // Compute Mars position relative to Earth
/// let mars_from_ssb = kernel.state_of(MARS, epoch, SSB)?;   // SSB→Mars
/// let earth_from_ssb = kernel.state_of(EARTH, epoch, SSB)?; // SSB→Earth
/// let mars_from_earth = mars_from_ssb - earth_from_ssb;     // Earth→Mars
/// ```
impl Sub for State {
    type Output = State;

    fn sub(self, other: State) -> State {
        assert_eq!(
            self.frame, other.frame,
            "Cannot subtract states in different frames: {} vs {}",
            self.frame, other.frame
        );
        assert_eq!(
            self.center, other.center,
            "Cannot subtract states with different centers: {} vs {}",
            self.center, other.center
        );

        State {
            target: self.target,
            center: other.target,
            frame: self.frame,
            position: [
                self.position[0] - other.position[0],
                self.position[1] - other.position[1],
                self.position[2] - other.position[2],
            ],
            velocity: [
                self.velocity[0] - other.velocity[0],
                self.velocity[1] - other.velocity[1],
                self.velocity[2] - other.velocity[2],
            ],
        }
    }
}

/// Relative motion (borrowed RHS).
impl Sub<&State> for State {
    type Output = State;

    fn sub(self, other: &State) -> State {
        assert_eq!(
            self.frame, other.frame,
            "Cannot subtract states in different frames: {} vs {}",
            self.frame, other.frame
        );
        assert_eq!(
            self.center, other.center,
            "Cannot subtract states with different centers: {} vs {}",
            self.center, other.center
        );

        State {
            target: self.target,
            center: other.target,
            frame: self.frame,
            position: [
                self.position[0] - other.position[0],
                self.position[1] - other.position[1],
                self.position[2] - other.position[2],
            ],
            velocity: [
                self.velocity[0] - other.velocity[0],
                self.velocity[1] - other.velocity[1],
                self.velocity[2] - other.velocity[2],
            ],
        }
    }
}

/// Reverses direction of state vector.
///
/// Swaps target and center, negates position and velocity.
/// `-(center→target)` becomes `(target→center)`.
///
/// # Example
///
/// ```ignore
/// let earth_to_moon = kernel.state_of(MOON, epoch, EARTH)?; // Earth→Moon
/// let moon_to_earth = -earth_to_moon;                        // Moon→Earth
/// ```
impl Neg for State {
    type Output = State;

    fn neg(self) -> State {
        State {
            target: self.center,
            center: self.target,
            frame: self.frame,
            position: [-self.position[0], -self.position[1], -self.position[2]],
            velocity: [-self.velocity[0], -self.velocity[1], -self.velocity[2]],
        }
    }
}

impl Default for State {
    fn default() -> Self {
        State {
            target: NaifId(0),
            center: NaifId(0),
            frame: 0,
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
        }
    }
}

/// Pointing data: quaternion and optional angular velocity with frame context.
///
/// A quaternion describes the rotation from a reference frame to the
/// instrument/body frame. The `frame` field identifies that reference frame.
///
/// Quaternion uses SPICE convention: scalar-first [q0, q1, q2, q3].
/// Angular velocity is in radians per second.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pointing {
    /// Reference frame code (NAIF frame ID)
    pub frame: i32,
    /// Quaternion [q0, q1, q2, q3] (scalar-first)
    pub quaternion: [f64; 4],
    /// Angular velocity [wx, wy, wz] in rad/s, if available
    pub angular_velocity: Option<[f64; 3]>,
}

impl Pointing {
    /// Create a new pointing with full context.
    ///
    /// # Arguments
    ///
    /// * `frame` - Reference frame code
    /// * `quaternion` - Rotation quaternion [q0, q1, q2, q3]
    /// * `angular_velocity` - Optional angular velocity [wx, wy, wz] in rad/s
    #[inline]
    pub fn new(frame: i32, quaternion: [f64; 4], angular_velocity: Option<[f64; 3]>) -> Self {
        Pointing {
            frame,
            quaternion,
            angular_velocity,
        }
    }

    /// Create a raw pointing without frame context.
    ///
    /// Use this only for intermediate calculations where frame will be
    /// added later. The frame is set to 0.
    #[inline]
    pub(crate) fn new_raw(quaternion: [f64; 4], angular_velocity: Option<[f64; 3]>) -> Self {
        Pointing {
            frame: 0,
            quaternion,
            angular_velocity,
        }
    }

    /// Create pointing from quaternion only.
    ///
    /// # Arguments
    ///
    /// * `frame` - Reference frame code
    /// * `quaternion` - Rotation quaternion [q0, q1, q2, q3]
    #[inline]
    pub fn from_quaternion(frame: i32, quaternion: [f64; 4]) -> Self {
        Pointing {
            frame,
            quaternion,
            angular_velocity: None,
        }
    }

    /// Get the scalar component of the quaternion.
    #[inline]
    pub fn scalar(&self) -> f64 {
        self.quaternion[0]
    }

    /// Get the vector component of the quaternion.
    #[inline]
    pub fn vector(&self) -> [f64; 3] {
        [self.quaternion[1], self.quaternion[2], self.quaternion[3]]
    }

    /// Check if the quaternion is normalized.
    pub fn is_normalized(&self) -> bool {
        let [q0, q1, q2, q3] = self.quaternion;
        let norm_sq = q0 * q0 + q1 * q1 + q2 * q2 + q3 * q3;
        (norm_sq - 1.0).abs() < 1e-10
    }

    /// Normalize the quaternion.
    pub fn normalize(&self) -> Pointing {
        let [q0, q1, q2, q3] = self.quaternion;
        let norm = (q0 * q0 + q1 * q1 + q2 * q2 + q3 * q3).sqrt();

        if norm < 1e-15 {
            return *self;
        }

        Pointing {
            frame: self.frame,
            quaternion: [q0 / norm, q1 / norm, q2 / norm, q3 / norm],
            angular_velocity: self.angular_velocity,
        }
    }
}

impl Default for Pointing {
    fn default() -> Self {
        // Identity quaternion (no rotation)
        Pointing {
            frame: 0,
            quaternion: [1.0, 0.0, 0.0, 0.0],
            angular_velocity: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test constants for body IDs
    const SSB: NaifId = NaifId(0);
    const EARTH: NaifId = NaifId(399);
    const MOON: NaifId = NaifId(301);
    const MARS: NaifId = NaifId(499);
    const J2000: i32 = 1;

    #[test]
    fn test_state_distance() {
        let state = State::new(EARTH, SSB, J2000, [3.0, 4.0, 0.0], [0.0, 0.0, 0.0]);
        assert!((state.distance() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_state_speed() {
        let state = State::new(EARTH, SSB, J2000, [0.0, 0.0, 0.0], [3.0, 4.0, 0.0]);
        assert!((state.speed() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_state_add_chain() {
        // Chain traversal: SSB→Earth + Earth→Moon = SSB→Moon
        let ssb_to_earth = State::new(EARTH, SSB, J2000, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        let earth_to_moon = State::new(MOON, EARTH, J2000, [4.0, 5.0, 6.0], [0.4, 0.5, 0.6]);
        let ssb_to_moon = ssb_to_earth + earth_to_moon;

        // Check position/velocity arithmetic
        assert!((ssb_to_moon.position[0] - 5.0).abs() < 1e-10);
        assert!((ssb_to_moon.velocity[2] - 0.9).abs() < 1e-10);

        // Check metadata propagation
        assert_eq!(ssb_to_moon.target, MOON);
        assert_eq!(ssb_to_moon.center, SSB);
        assert_eq!(ssb_to_moon.frame, J2000);
    }

    #[test]
    fn test_state_sub_relative() {
        // Relative motion: (SSB→Mars) - (SSB→Earth) = Earth→Mars
        let ssb_to_mars = State::new(MARS, SSB, J2000, [5.0, 7.0, 9.0], [0.5, 0.7, 0.9]);
        let ssb_to_earth = State::new(EARTH, SSB, J2000, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        let earth_to_mars = ssb_to_mars - ssb_to_earth;

        // Check position/velocity arithmetic
        assert!((earth_to_mars.position[0] - 4.0).abs() < 1e-10);
        assert!((earth_to_mars.position[1] - 5.0).abs() < 1e-10);
        assert!((earth_to_mars.velocity[2] - 0.6).abs() < 1e-10);

        // Check metadata propagation
        assert_eq!(earth_to_mars.target, MARS);
        assert_eq!(earth_to_mars.center, EARTH);
        assert_eq!(earth_to_mars.frame, J2000);
    }

    #[test]
    fn test_state_negate() {
        // Negation swaps target and center: -(SSB→Earth) = Earth→SSB
        let ssb_to_earth = State::new(EARTH, SSB, J2000, [1.0, -2.0, 3.0], [-0.1, 0.2, -0.3]);
        let earth_to_ssb = -ssb_to_earth;

        // Check position/velocity negation
        assert!((earth_to_ssb.position[0] + 1.0).abs() < 1e-10);
        assert!((earth_to_ssb.position[1] - 2.0).abs() < 1e-10);
        assert!((earth_to_ssb.velocity[0] - 0.1).abs() < 1e-10);

        // Check metadata swap
        assert_eq!(earth_to_ssb.target, SSB);
        assert_eq!(earth_to_ssb.center, EARTH);
        assert_eq!(earth_to_ssb.frame, J2000);
    }

    #[test]
    fn test_state_zero_operations() {
        // Zero state at SSB with Earth as target
        let zero = State::new(EARTH, SSB, J2000, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        let earth_to_moon = State::new(MOON, EARTH, J2000, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);

        // zero + state (SSB→Earth + Earth→Moon = SSB→Moon)
        let sum = zero + earth_to_moon;
        assert!((sum.position[0] - 1.0).abs() < 1e-10);
        assert!((sum.velocity[2] - 0.3).abs() < 1e-10);

        // state - state (same state subtracted from itself)
        let ssb_to_earth = State::new(EARTH, SSB, J2000, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        let diff = ssb_to_earth - ssb_to_earth;
        assert!(diff.distance() < 1e-10);
        assert!(diff.speed() < 1e-10);

        // -zero = zero
        let zero_state = State::default();
        let neg_zero = -zero_state;
        assert!(neg_zero.distance() < 1e-10);
    }

    #[test]
    fn test_state_reference_operations() {
        // Chain traversal with borrowed RHS
        let ssb_to_earth = State::new(EARTH, SSB, J2000, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        let earth_to_moon = State::new(MOON, EARTH, J2000, [4.0, 5.0, 6.0], [0.4, 0.5, 0.6]);

        let sum = ssb_to_earth + &earth_to_moon;
        assert!((sum.position[0] - 5.0).abs() < 1e-10);

        // Relative motion with borrowed RHS
        let ssb_to_mars = State::new(MARS, SSB, J2000, [4.0, 5.0, 6.0], [0.4, 0.5, 0.6]);
        let ssb_to_earth2 = State::new(EARTH, SSB, J2000, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        let diff = ssb_to_mars - &ssb_to_earth2;
        assert!((diff.position[0] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_state_large_magnitudes() {
        // AU-scale distances (~1.5e8 km for Earth-Sun distance)
        let ssb_to_earth = State::new(
            EARTH,
            SSB,
            J2000,
            [1.5e8, 0.0, 0.0], // 1 AU in km (approximately)
            [0.0, 29.78, 0.0], // Earth orbital velocity km/s
        );
        let ssb_to_mars = State::new(
            MARS,
            SSB,
            J2000,
            [2.28e8, 0.0, 0.0], // Mars distance in km
            [0.0, 24.07, 0.0],  // Mars orbital velocity km/s
        );

        // Relative state Earth to Mars
        let earth_to_mars = ssb_to_mars - ssb_to_earth;
        assert!((earth_to_mars.position[0] - 0.78e8).abs() < 1e3);
        assert!((earth_to_mars.velocity[1] - (-5.71)).abs() < 0.01);

        // Verify magnitudes are preserved correctly
        assert!((ssb_to_earth.distance() - 1.5e8).abs() < 1.0);
        assert!((ssb_to_earth.speed() - 29.78).abs() < 0.01);
    }

    #[test]
    #[should_panic(expected = "Cannot add states in different frames")]
    fn test_state_add_frame_mismatch() {
        let s1 = State::new(EARTH, SSB, 1, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        let s2 = State::new(MOON, EARTH, 2, [4.0, 5.0, 6.0], [0.4, 0.5, 0.6]);
        let _ = s1 + s2; // Should panic
    }

    #[test]
    #[should_panic(expected = "Invalid chain")]
    fn test_state_add_invalid_chain() {
        // SSB→Earth + SSB→Moon is invalid (Earth != SSB)
        let s1 = State::new(EARTH, SSB, J2000, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        let s2 = State::new(MOON, SSB, J2000, [4.0, 5.0, 6.0], [0.4, 0.5, 0.6]);
        let _ = s1 + s2; // Should panic
    }

    #[test]
    #[should_panic(expected = "Cannot subtract states in different frames")]
    fn test_state_sub_frame_mismatch() {
        let s1 = State::new(MARS, SSB, 1, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        let s2 = State::new(EARTH, SSB, 2, [4.0, 5.0, 6.0], [0.4, 0.5, 0.6]);
        let _ = s1 - s2; // Should panic
    }

    #[test]
    #[should_panic(expected = "Cannot subtract states with different centers")]
    fn test_state_sub_center_mismatch() {
        // (SSB→Mars) - (Earth→Moon) is invalid (SSB != Earth)
        let s1 = State::new(MARS, SSB, J2000, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        let s2 = State::new(MOON, EARTH, J2000, [4.0, 5.0, 6.0], [0.4, 0.5, 0.6]);
        let _ = s1 - s2; // Should panic
    }

    #[test]
    fn test_pointing_normalized() {
        let p = Pointing::from_quaternion(J2000, [1.0, 0.0, 0.0, 0.0]);
        assert!(p.is_normalized());

        let unnorm = Pointing::from_quaternion(J2000, [2.0, 0.0, 0.0, 0.0]);
        assert!(!unnorm.is_normalized());

        let normalized = unnorm.normalize();
        assert!(normalized.is_normalized());
        assert!((normalized.quaternion[0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_pointing_components() {
        let p = Pointing::new(J2000, [0.5, 0.5, 0.5, 0.5], Some([0.1, 0.2, 0.3]));

        assert!((p.scalar() - 0.5).abs() < 1e-10);
        assert_eq!(p.vector(), [0.5, 0.5, 0.5]);
        assert!(p.angular_velocity.is_some());
        assert_eq!(p.frame, J2000);
    }

    #[test]
    fn test_default_values() {
        let state = State::default();
        assert_eq!(state.distance(), 0.0);
        assert_eq!(state.target, NaifId(0));
        assert_eq!(state.center, NaifId(0));
        assert_eq!(state.frame, 0);

        let pointing = Pointing::default();
        assert!(pointing.is_normalized());
        assert_eq!(pointing.scalar(), 1.0);
        assert_eq!(pointing.frame, 0);
    }
}
