//! Camera tests: the spec's vectors (camera.md 8) and the modes, on synthetic scenes.

use alloc::vec;

use formats::collision::{CollisionMesh, CollisionTree, CollisionTriangle};
use formats::geometry::Affine;

use super::follow::*;
use super::framing::{Frame, Framing, OffsetFn, ZoneFrame};
use super::zone::box_zone;
use crate::collision::TriMesh;
use crate::collision::world::{CollisionWorld, Shape};
use crate::math::{Vec3, length, sub};

const DT: f32 = 1.0 / 60.0;

fn still(at: Vec3) -> Subject {
    Subject { position: at, ..Subject::default() }
}

fn close(a: Vec3, b: Vec3, tol: f32) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= tol)
}

fn settle(cam: &mut FollowCamera, subject: &Subject, scene: &CameraScene<'_>, frames: usize) -> CameraFrame {
    let mut f = None;
    for _ in 0..frames {
        f = Some(cam.update(DT, &CameraInput::default(), subject, scene));
    }
    f.unwrap()
}

#[test]
fn default_framing_vector() {
    // camera.md 8: no zone, player still facing +x on flat ground, settled: focus 100 ahead and 125 above the
    // ground; framed offset 450 back and 125 up; after the near-clip push the eye is at focus + (−546.4, 0, 151.8)
    // with near 100; the focus drawn 0.1 of the screen below centre.
    let mut cam = FollowCamera::new();
    let s = Subject { yaw: 0.0, ..still([100.0, 200.0, 0.0]) };
    cam.cut_to([-500.0, 200.0, 300.0], [100.0, 200.0, 80.0]);
    let f = settle(&mut cam, &s, &CameraScene::default(), 300);
    assert!(close(f.focus, [200.0, 200.0, 125.0], 0.05), "focus {:?}", f.focus);
    assert!(close(sub(cam.raw_eye, f.focus), [-450.0, 0.0, 125.0], 0.05), "{:?}", sub(cam.raw_eye, f.focus));
    assert!(close(sub(f.eye, f.focus), [-546.4, 0.0, 151.8], 0.05), "{:?}", sub(f.eye, f.focus));
    let ndc = f.project(f.focus, 4.0 / 3.0);
    assert!(ndc[0].abs() < 1e-3 && (ndc[1] + 0.1).abs() < 1e-3, "{ndc:?}");
    // Paris lens: near 70 gives focus + (−517.5, 0, 143.7).
    cam.set_lens(Lens { near: 70.0, ..cam.lens });
    let f = settle(&mut cam, &s, &CameraScene::default(), 2);
    // (The spec rounds 517.45 to 517.5.)
    assert!(close(sub(f.eye, f.focus), [-517.5, 0.0, 143.7], 0.1), "{:?}", sub(f.eye, f.focus));
    // In the air the ground height is kept: a jump does not lift the base point.
    let up = Subject { airborne: true, ..still([100.0, 200.0, 200.0]) };
    let f = settle(&mut cam, &up, &CameraScene::default(), 30);
    assert!((f.focus[2] - 125.0).abs() < 1.0, "{:?}", f.focus);
    assert!((clearance_radius(1.0, 1.2987, 100.0) - 90.5).abs() < 0.05);
    assert!((clearance_radius(1.1, 1.2987, 70.0) - 71.3).abs() < 0.05);
}

#[test]
fn orbit_vector() {
    // camera.md 8: right stick fully right from rest: 0.25 rad/s after 1/60 s; 2.5 rad/s after 0.1667 s.
    let mut cam = FollowCamera::new();
    assert!((cam.orbit_step(1.0, DT) - 0.25).abs() < 1e-6);
    for _ in 0..9 {
        cam.orbit_step(1.0, DT);
    }
    assert!((cam.orbit_speed - 2.5).abs() < 1e-5);
    // Half deflection: x·|x|·2.5.
    let mut cam = FollowCamera::new();
    for _ in 0..60 {
        cam.orbit_step(0.5, DT);
    }
    assert!((cam.orbit_speed - 0.625).abs() < 1e-5);
    // In the follow mode the stick turns the camera around the focus.
    let mut cam = FollowCamera::new();
    let s = still([0.0; 3]);
    cam.cut_to([-450.0, 0.0, 205.0], [0.0, 0.0, 80.0]);
    settle(&mut cam, &s, &CameraScene::default(), 2);
    let input = CameraInput { right: [1.0, 0.0], ..Default::default() };
    for _ in 0..60 {
        cam.update(DT, &input, &s, &CameraScene::default());
    }
    assert!(cam.yaw.abs() > 1.0);
}

#[test]
fn spring_vector() {
    // camera.md 4.2 and 8: the eye spring (15, −8) is over-damped: a 100-unit step is 88.6 after 1 s (60 frames),
    // within 1 % from frame 114, never more than 100.
    let c = super::consts::CameraConstants::SPEC;
    let [k, d] = c.eye_spring;
    let (mut x, mut v) = (0.0f32, 0.0f32);
    let mut first_within = None;
    let mut max_x = 0.0f32;
    for i in 1i32..=300 {
        (x, v) = eye_spring_step(x, v, 100.0, k, d, DT);
        max_x = max_x.max(x);
        if i == 60 {
            assert!((x - 88.6).abs() < 0.3, "after 1 s: {x}");
        }
        if first_within.is_none() && x >= 99.0 {
            first_within = Some(i);
        }
    }
    assert!(max_x <= 100.0, "overshoot {max_x}");
    assert!((first_within.unwrap() - 114).abs() <= 2, "{first_within:?}");
    // A zone with 885 = 0.5: (7.5, −8), slower.
    let (mut y, mut w) = (0.0f32, 0.0f32);
    for _ in 0..60 {
        (y, w) = eye_spring_step(y, w, 100.0, k * 0.5, d, DT);
    }
    assert!(y < 70.0, "{y}");
    // In the camera the eye moves with the focus and keeps its framed offset: a 100-unit move of the player moves
    // the eye 100 without overshoot.
    let mut cam = FollowCamera::new();
    let mut s = still([0.0; 3]);
    cam.cut_to([-450.0, 0.0, 205.0], [0.0, 0.0, 80.0]);
    settle(&mut cam, &s, &CameraScene::default(), 120);
    let start = cam.eye;
    s.position = [100.0, 0.0, 0.0];
    let mut max_x = f32::MIN;
    for _ in 0..300 {
        let f = cam.update(DT, &CameraInput::default(), &s, &CameraScene::default());
        max_x = max_x.max(f.eye[0] - start[0]);
    }
    assert!(max_x < 100.01, "overshoot {max_x}");
    assert!((cam.eye[0] - start[0] - 100.0).abs() < 0.5, "{}", cam.eye[0] - start[0]);
}

#[test]
fn zones_frame_the_focus() {
    // A zone over x in [0, 1000]: its frame looks along +y; the camera keeps 300 back and 200 up.
    let framing = Framing {
        distance: -300.0,
        offsets: [
            OffsetFn::IDENTITY,
            OffsetFn::IDENTITY,
            OffsetFn { c: [200.0, 1.0, 0.0], min: -f32::MAX, max: f32::MAX },
        ],
        ..Framing::DEFAULT
    };
    let zones = vec![box_zone(
        7,
        [0.0, -1000.0, -1000.0],
        [1000.0, 1000.0, 1000.0],
        framing,
        ZoneFrame::Fixed(Frame::level([0.0; 3], core::f32::consts::FRAC_PI_2)),
        1,
    )];
    let scene = CameraScene { zones: &zones, ..Default::default() };
    let mut cam = FollowCamera::new();
    let s = still([500.0, 100.0, 0.0]);
    let f = settle(&mut cam, &s, &scene, 300);
    assert_eq!(f.zone, Some(7));
    // Focus (600, 100, 125) (100 ahead of the player facing +x) in the frame: forward 100, left −600, up 125 →
    // framed eye at forward −200, left −600, up 325: (600, −200, 325). The first frame cuts to the framed
    // direction; then the near-clip push moves the eye out by 100 along the focus line.
    assert!(close(f.focus, [600.0, 100.0, 125.0], 0.05), "{:?}", f.focus);
    assert!(close(cam.raw_eye, [600.0, -200.0, 325.0], 0.1), "{:?}", cam.raw_eye);
    let e = sub(cam.raw_eye, f.focus);
    let k = (length(e) + 100.0) / length(e);
    assert!(close(f.eye, [600.0 + e[0] * k, 100.0 + e[1] * k, 125.0 + e[2] * k], 0.1), "{:?}", f.eye);
    // Outside the zone: the default framing again.
    let s = still([-500.0, 0.0, 0.0]);
    let f = settle(&mut cam, &s, &scene, 300);
    assert_eq!(f.zone, None);
}

/// A wall at x = −200 between the camera and a player at the origin facing +x.
fn wall_world() -> CollisionWorld {
    let positions = vec![[-200.0, -1000.0, -1000.0], [-200.0, 1000.0, -1000.0], [-200.0, 0.0, 1000.0]];
    let tri = CollisionTriangle { corners: [0, 1, 2], radius: 2000.0, center: [0.0; 3], first_edge: 0, edge_count: 0 };
    let mesh = CollisionMesh { radius: 2000.0, positions, triangles: vec![tri], edges: vec![] };
    let mut cw = CollisionWorld::new();
    let s = cw.add_shape(Shape { main: TriMesh::new(&mesh, &CollisionTree::default()), camera: None });
    cw.add_body(s, Affine::IDENTITY, 0, 0, true);
    cw.rebuild();
    cw
}

#[test]
fn view_clearing_keeps_the_clearance() {
    // The eye starts in front of a wall at x = −200 and the framing wants it 450 behind the focus: the squish lets
    // it approach no closer than the clearance radius (90.5) to the wall, and the player stays visible, so the
    // camera does not cut.
    let cw = wall_world();
    let scene = CameraScene { collision: Some(&cw), ..Default::default() };
    let mut cam = FollowCamera::new();
    let s = still([0.0; 3]);
    cam.cut_to([-50.0, 0.0, 250.0], [0.0, 0.0, 125.0]);
    // The first frame is a cut: the framed eye is behind the wall, so the cut searches a clear direction.
    let f = cam.update(DT, &CameraInput::default(), &s, &scene);
    assert!(cam.raw_eye[0] > -200.0, "{:?}", cam.raw_eye);
    let mut min_gap = f32::MAX;
    for _ in 0..240 {
        let f = cam.update(DT, &CameraInput::default(), &s, &scene);
        min_gap = min_gap.min(cam.raw_eye[0] + 200.0);
        assert!(!f.cut);
    }
    let _ = f;
    assert!(min_gap > 85.0, "{min_gap}");
    // A camera put behind the wall cuts back to a clear place once the player has been hidden for 2 s.
    let mut cam = FollowCamera::new();
    cam.cut_to([-450.0, 0.0, 250.0], [0.0, 0.0, 125.0]);
    cam.update(DT, &CameraInput::default(), &s, &scene);
    cam.raw_eye = [-450.0, 0.0, 250.0];
    let mut cleared = false;
    for _ in 0..200 {
        cam.update(DT, &CameraInput::default(), &s, &scene);
        if cam.raw_eye[0] > -200.0 {
            cleared = true;
            break;
        }
    }
    assert!(cleared, "{:?}", cam.raw_eye);
}

#[test]
fn modes_panels_and_letterbox() {
    let mut cam = FollowCamera::new();
    let s = Subject { yaw: 0.0, ..still([10.0, 20.0, 0.0]) };
    // Look-around: 70 above the anchor, 25 back along the look axis when near.
    cam.set_mode(Mode::Look { yaw: 0.0, pitch: 0.0, zoom: 0.0 });
    let f = cam.update(DT, &CameraInput::default(), &s, &CameraScene::default());
    assert!(f.cut);
    assert!(close(f.eye, [-15.0, 20.0, 70.0], 1e-3), "{:?}", f.eye);
    cam.set_mode(Mode::Look { yaw: 0.0, pitch: 0.0, zoom: 1.0 });
    let f = cam.update(DT, &CameraInput::default(), &s, &CameraScene::default());
    assert!(close(f.eye, [-140.0, 20.0, 70.0], 1e-3));
    // A cutscene placement: eye at its position, looking along its forward column.
    let placement = Affine { cols: [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]], t: [5.0, 6.0, 7.0] };
    cam.set_mode(Mode::Animation { placement });
    let f = cam.update(DT, &CameraInput::default(), &s, &CameraScene::default());
    assert_eq!((f.eye, f.target), ([5.0, 6.0, 7.0], [5.0, 7.0, 7.0]));
    // Panels and letterbox: the letterbox does not change the field of view.
    let panel = PanelSubject {
        position: [0.0; 3],
        yaw: 0.0,
        fov: None,
        distance: None,
        screen: [0.5, 0.5],
        light_offset: [0.0; 3],
    };
    let scene = CameraScene { panels: [Some(panel), None], letterbox: true, ..Default::default() };
    cam.set_mode(Mode::Follow);
    let f = cam.update(DT, &CameraInput::default(), &s, &scene);
    let p = f.panels[0].unwrap();
    assert!(f.letterbox && f.fov == 1.0);
    assert_eq!((p.fov, length(sub(p.eye, p.target))), (0.5, 400.0));
    // The field of view eases toward a new lens.
    cam.set_lens(Lens { fov: 1.1, ..cam.lens });
    let f = cam.update(DT, &CameraInput::default(), &s, &scene);
    assert!(f.fov > 1.0 && f.fov < 1.1);
    let f = settle(&mut cam, &s, &scene, 60);
    assert!((f.fov - 1.1).abs() < 1e-4);
}

#[test]
fn projection_matrices() {
    let f = CameraFrame {
        eye: [0.0, -10.0, 0.0],
        target: [0.0; 3],
        up: [0.0, 0.0, 1.0],
        fov: 1.0,
        near: 1.0,
        far: 100.0,
        fog: [0.0; 3],
        fog_color: None,
        focus: [0.0; 3],
        faded: vec![],
        letterbox: false,
        panels: [None, None],
        cut: false,
        zone: None,
    };
    assert_eq!(f.project([0.0; 3], 1.0), [0.0, 0.0]);
    // A point up and to the right lands up and right on screen.
    let p = f.project([1.0, 0.0, 1.0], 1.0);
    assert!(p[0] > 0.0 && p[1] > 0.0);
    let m = f.projection(1.0, true);
    assert_eq!(m[11], -1.0);
}
