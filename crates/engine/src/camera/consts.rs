//! The camera's constants (camera.md): the default framing and the smoothing settings read from the executable,
//! the spec's [R] values, and the port's tunables.

use formats::disc::Executable;

use super::framing::{Framing, OffsetFn};
use crate::physics::spring::SmoothParams;

/// Virtual address of the default framing block (camera.md 3).
pub const FRAMING_VA: u32 = 0x0027_5980;

/// The spec's camera values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraConstants {
    /// The default framing [X 0x275980].
    pub framing: Framing,
    /// Eye spring: strength 15, damping −8 [X 0x261920].
    pub eye_spring: [f32; 2],
    /// Focus offset spring toward a move's own focus point (8, −6) [R].
    pub focus_spring: [f32; 2],
    /// Focus offset spring back to zero when the player is relaxed (5, −3.5) [R].
    pub relax_spring: [f32; 2],
    /// Focus offset damping in the clamped state (−10) [R].
    pub clamped_damping: f32,
    /// Look-ahead: the adjustment springs (5, −3.5) toward a point this far ahead along the facing (100) [R].
    pub look_ahead: f32,
    /// Look-ahead spring (5, −3.5) [R].
    pub look_ahead_spring: [f32; 2],
    /// The base point's height above the ground height (125) [R].
    pub base_height: f32,
    /// Smoothing of the base point's height (1000, 10, 1.0) [X 0x269BC0].
    pub base_smooth: SmoothParams,
    /// The ground height is kept between these heights relative to the origin (−350, −50) [R].
    pub ground_limits: [f32; 2],
    /// The base point's height is kept between these heights relative to the origin (−325, 75) [R].
    pub base_limits: [f32; 2],
    /// On a hook the ground height is the hook's height minus this (300) [R].
    pub hook_drop: f32,
    /// On a cut the ground height is the origin's height minus this (75) [R].
    pub cut_drop: f32,
    /// Turning to the framed direction: smoothing (0.5, 0, 0.5), its fast speed scaled by the lock strength
    /// [X 0x275A58].
    pub lock_smooth: SmoothParams,
    /// Automatic orbit: the stick factor `clamp(1 − 2y − 2y², 1, 2)` (coefficients [X 0x275A40], limits
    /// [X 0x275A50]).
    pub orbit_toward: [f32; 5],
    /// Automatic orbit: the ramp after a manual orbit, `clamp(0 + 1·t, 0, 1)` [X 0x275A70].
    pub orbit_ramp: [f32; 3],
    /// Lock speed divisor while flattened (200) [R].
    pub lock_flattened_speed: f32,
    /// Idle pan: starts this long after the player last moved (0.75 s) and this long after the last manual orbit
    /// (10 s) [R].
    pub idle_pan_delay: [f32; 2],
    /// The aspect ratio the clearance radius uses (1.2987) [R].
    pub aspect: f32,
    /// The player's origin above its ground contact point, which the focus clamps are measured from (90,
    /// player-moves.md 1.1) [D].
    pub origin_height: f32,
    /// Cut when blocked: the probe points around the player: sideways, up, down (50, 75, 50) [R].
    pub probe_offsets: [f32; 3],
    /// Cut when blocked: the nearest clear point is searched within this distance (500) [R].
    pub cut_search: f32,
    /// Cut when blocked: pan samples at the tilts −90° to +90° in steps of 22.5° (1, 5, 10, 12, 15, 12, 10, 5, 1)
    /// [X 0x261928, stored as floats].
    pub cut_samples: [u32; 9],
    /// Cut when blocked: angle step of the search (about 16°) [R].
    pub cut_angle_step: f32,
    /// Smoothing of the automatic-follow factor (4, 0, 0.25) [X 0x275A80].
    pub follow_smooth: SmoothParams,
    /// Recentre smoothing (2.5, 0, 0.35, 15) [X 0x275A90].
    pub recentre_smooth: SmoothParams,
    /// Field-of-view smoothing (0.5, 0, 0.1, 10) [X 0x2618B8].
    pub fov_smooth: SmoothParams,
    /// View clearing: the eye's approach to a surface follows the smoothing (2500, 0, 0.25) [X 0x261958].
    pub squish_smooth: SmoothParams,
    /// View clearing: the check's own record, sphere radius 250 and extra speed allowance 1500 (third value unused)
    /// [X 0x26194C].
    pub squish_record: [f32; 3],
    /// Most orbit speed, manual and automatic (2.5 rad/s) [R].
    pub orbit_speed: f32,
    /// Most change of the manual orbit speed (15 rad/s²) [R].
    pub orbit_accel: f32,
    /// Player speed for full lock toward a path (600) [R].
    pub lock_full_speed: f32,
    /// Look-around: eye height above the anchor (70) [R].
    pub look_height: f32,
    /// Look-around: eye offset along the look axis, near (−25) [X 0x261C20].
    pub look_near: f32,
    /// Look-around: eye offset along the look axis, far (−150) [X 0x261C10].
    pub look_far: f32,
    /// View clearing: search box half-size around the previous eye (250) [X 0x261970].
    pub squish_box: f32,
    /// View clearing: recheck delay (0.5 s) [R].
    pub squish_recheck: f32,
    /// View clearing: cut after this much obstruction (2 s) [R].
    pub squish_cut: f32,
    /// Camera-fade: a blocking mesh stays faded this long (0.1 s) (collision.md 4).
    pub fade_time: f32,
    /// Field of view default (1.0) [R].
    pub fov: f32,
    /// Near clip default (100) [R].
    pub near: f32,
    /// Far clip default (20000) [R].
    pub far: f32,
    /// Fog start, end, maximum defaults (500, 20000, 0.5) [R].
    pub fog: [f32; 3],
    /// Dialogue picture panel: field of view (0.5) and distance (400) defaults (options 989, 990) [R].
    pub panel_lens: [f32; 2],
}

impl CameraConstants {
    /// The spec's values.
    pub const SPEC: CameraConstants = CameraConstants {
        framing: Framing::DEFAULT,
        eye_spring: [15.0, -8.0],
        focus_spring: [8.0, -6.0],
        relax_spring: [5.0, -3.5],
        clamped_damping: -10.0,
        look_ahead: 100.0,
        look_ahead_spring: [5.0, -3.5],
        base_height: 125.0,
        base_smooth: SmoothParams { fast: 1000.0, slow: 10.0, fast_time: 1.0, max_speed_change: None },
        ground_limits: [-350.0, -50.0],
        base_limits: [-325.0, 75.0],
        hook_drop: 300.0,
        cut_drop: 75.0,
        lock_smooth: SmoothParams { fast: 0.5, slow: 0.0, fast_time: 0.5, max_speed_change: None },
        orbit_toward: [1.0, -2.0, -2.0, 1.0, 2.0],
        orbit_ramp: [0.0, 1.0, 0.0],
        lock_flattened_speed: 200.0,
        idle_pan_delay: [0.75, 10.0],
        aspect: 1.2987,
        origin_height: 90.0,
        probe_offsets: [50.0, 75.0, 50.0],
        cut_search: 500.0,
        cut_samples: [1, 5, 10, 12, 15, 12, 10, 5, 1],
        cut_angle_step: 16.0 * core::f32::consts::PI / 180.0,
        follow_smooth: SmoothParams { fast: 4.0, slow: 0.0, fast_time: 0.25, max_speed_change: None },
        recentre_smooth: SmoothParams { fast: 2.5, slow: 0.0, fast_time: 0.35, max_speed_change: Some(15.0) },
        fov_smooth: SmoothParams { fast: 0.5, slow: 0.0, fast_time: 0.1, max_speed_change: Some(10.0) },
        squish_smooth: SmoothParams { fast: 2500.0, slow: 0.0, fast_time: 0.25, max_speed_change: None },
        squish_record: [250.0, 1500.0, 0.0],
        orbit_speed: 2.5,
        orbit_accel: 15.0,
        lock_full_speed: 600.0,
        look_height: 70.0,
        look_near: -25.0,
        look_far: -150.0,
        squish_box: 250.0,
        squish_recheck: 0.5,
        squish_cut: 2.0,
        fade_time: 0.1,
        fov: 1.0,
        near: 100.0,
        far: 20000.0,
        fog: [500.0, 20000.0, 0.5],
        panel_lens: [0.5, 400.0],
    };

    /// The spec's values with the [X] ones read from the player's executable after its SHA-256 check.
    pub fn from_executable(exe: &Executable) -> formats::Result<CameraConstants> {
        if !exe.is_supported() {
            return Err(formats::Error::UnknownExecutable);
        }
        let f = |va: u32| -> formats::Result<f32> {
            let b = exe.bytes_at(va, 4)?;
            Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let smooth = |va: u32, rate: bool| -> formats::Result<SmoothParams> {
            Ok(SmoothParams {
                fast: f(va)?,
                slow: f(va + 4)?,
                fast_time: f(va + 8)?,
                max_speed_change: if rate { Some(f(va + 12)?) } else { None },
            })
        };
        // The framing block: distance; three coefficient vectors at 16-byte strides from +16; three (min, max)
        // pairs from +64; the focus's screen position at +88; adjust, follow and lock-on factors at +96; focus
        // scale at +112.
        let b = FRAMING_VA;
        let coeff = |o: u32| -> formats::Result<[f32; 3]> { Ok([f(b + o)?, f(b + o + 4)?, f(b + o + 8)?]) };
        let mut offsets = [OffsetFn::IDENTITY; 3];
        for (i, o) in offsets.iter_mut().enumerate() {
            let i = i as u32;
            *o = OffsetFn { c: coeff(16 + 16 * i)?, min: f(b + 64 + 8 * i)?, max: f(b + 68 + 8 * i)? };
        }
        let framing = Framing {
            distance: f(b)?,
            offsets,
            screen: [f(b + 88)?, f(b + 92)?],
            adjust: f(b + 96)?,
            follow: f(b + 100)?,
            lock_on: f(b + 104)?,
            focus_scale: coeff(112)?,
            eye_spring: [f(b + 128)?, f(b + 132)?],
            ..Framing::DEFAULT
        };
        Ok(CameraConstants {
            framing,
            eye_spring: [f(0x261920)?, f(0x261924)?],
            follow_smooth: smooth(0x275A80, false)?,
            recentre_smooth: smooth(0x275A90, true)?,
            fov_smooth: smooth(0x2618B8, true)?,
            squish_smooth: smooth(0x261958, false)?,
            squish_record: [f(0x26194C)?, f(0x261950)?, f(0x261954)?],
            squish_box: -f(0x261970)?,
            base_smooth: smooth(0x269BC0, false)?,
            lock_smooth: smooth(0x275A58, false)?,
            orbit_toward: [f(0x275A40)?, f(0x275A44)?, f(0x275A48)?, f(0x275A50)?, f(0x275A54)?],
            orbit_ramp: [f(0x275A70)?, f(0x275A74)?, f(0x275A78)?],
            cut_samples: {
                let b = exe.bytes_at(0x261928, 36)?;
                let mut n = [0u32; 9];
                for (k, v) in n.iter_mut().enumerate() {
                    // Stored as floats.
                    *v = f32::from_le_bytes([b[k * 4], b[k * 4 + 1], b[k * 4 + 2], b[k * 4 + 3]]) as u32;
                }
                n
            },
            look_near: f(0x261C20)?,
            look_far: f(0x261C10)?,
            ..CameraConstants::SPEC
        })
    }
}

/// Values the camera spec leaves open, chosen for this port.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraTunables {
    /// Look-around turn rate (rad/s).
    pub look_rate: f32,
    /// Look-around pitch limit.
    pub look_pitch: f32,
    /// Distance the player must move to end a manual framing.
    pub manual_release: f32,
    /// Conversation view: eye offset in the anchor's frame (forward, left, up).
    pub conversation_offset: [f32; 3],
    /// Height of a speaker's head above its origin, for the picture panels.
    pub panel_head: f32,
}

impl Default for CameraTunables {
    fn default() -> Self {
        CameraTunables {
            look_rate: 2.0,
            look_pitch: 1.2,
            manual_release: 400.0,
            conversation_offset: [300.0, -150.0, 120.0],
            panel_head: 80.0,
        }
    }
}
