//! Renders a small demo scene with the OpenGL backend on a headless EGL context and checks the image. Skipped (with
//! a message) where no headless OpenGL 4.3 context is available.

#[cfg(target_os = "linux")]
#[test]
fn gl_renders_demo_scene() {
    use render::demo::{DemoParams, DemoScene};
    use render::gl::egl::HeadlessGl;
    use render::gl::{GlBackend, GlTarget};
    use render::{Backend, Renderer};

    let ctx = match HeadlessGl::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipped: no headless OpenGL context ({e})");
            return;
        }
    };
    let (w, h) = (160, 90);
    // SAFETY: `ctx` made a 4.3 core context current on this thread and outlives the backend.
    let backend = match unsafe {
        GlBackend::new(&mut |n| ctx.get_proc_address(n), GlTarget::Offscreen { width: w, height: h })
    } {
        Ok(b) => b,
        // A context without a working backend (a shader that does not compile) is a failure, not a skip.
        Err(e) => panic!("{e}"),
    };
    let mut r = Renderer::new(backend);
    let params = DemoParams { grid_x: 12, grid_z: 8, mesh_variants: 20, animate: false };
    let scene = DemoScene::build(&mut r, &params).unwrap();
    let clear = [0.0, 0.0, 1.0, 1.0];
    let mut stats = None;
    for _ in 0..4 {
        stats = Some(r.render(&scene.camera(0.0), clear).unwrap());
    }
    let s = stats.unwrap();
    assert!(s.visible > 0);
    assert_eq!(s.api_draw_calls, s.batches);
    assert_eq!(r.backend().target_size(), (w, h));

    let px = r.backend().read_pixels();
    assert_eq!(px.len(), (w * h * 4) as usize);
    // The top row is sky (clear colour); a good part of the image is covered by boxes.
    assert_eq!(&px[0..4], &[0, 0, 255, 255]);
    let covered = px.chunks(4).filter(|p| p[..3] != [0, 0, 255]).count();
    assert!(covered > (w * h / 10) as usize, "only {covered} pixels drawn");

    // Same input, same image.
    r.render(&scene.camera(0.0), clear).unwrap();
    assert_eq!(px, r.backend().read_pixels());
    drop(r);
    drop(ctx);
}

/// Skinning, morphing and the zero-weight rule on the GPU: a quad skinned to one bone moves with it; vertices
/// with no weights stay; morphed positions replace the mesh's.
#[cfg(target_os = "linux")]
#[test]
fn gl_skins_and_morphs() {
    use render::gl::egl::HeadlessGl;
    use render::gl::{GlBackend, GlTarget};
    use render::math::{Mat4, Vec3};
    use render::{BoneMatrix, Camera, CullMode, Material, MeshData, Renderer, SkinVertex, TextureData, Vertex};

    let ctx = match HeadlessGl::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipped: no headless OpenGL context ({e})");
            return;
        }
    };
    let (w, h) = (64, 64);
    // SAFETY: `ctx` made a 4.3 core context current on this thread and outlives the backend.
    let backend = match unsafe {
        GlBackend::new(&mut |n| ctx.get_proc_address(n), GlTarget::Offscreen { width: w, height: h })
    } {
        Ok(b) => b,
        // A context without a working backend (a shader that does not compile) is a failure, not a skip.
        Err(e) => panic!("{e}"),
    };
    let mut r = Renderer::new(backend);
    r.set_cull_mode(CullMode::None);
    // A unit quad around the origin, facing +z; the camera looks at it from +z with a 1-unit half height.
    let n = [0.0, 0.0, 1.0];
    let white = [255; 4];
    let quad = |s: f32| {
        [
            Vertex::new([-s, -s, 0.0], [0.0, 0.0], n, white),
            Vertex::new([s, -s, 0.0], [1.0, 0.0], n, white),
            Vertex::new([s, s, 0.0], [1.0, 1.0], n, white),
            Vertex::new([-s, s, 0.0], [0.0, 1.0], n, white),
        ]
    };
    let v = quad(0.25);
    let idx = [0u16, 1, 2, 0, 2, 3];
    let one = SkinVertex::new([0, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]);
    let skinned = r.add_skinned_mesh(&MeshData { vertices: &v, indices: &idx }, &[one; 4]).unwrap();
    let unweighted = r.add_skinned_mesh(&MeshData { vertices: &v, indices: &idx }, &[SkinVertex::NONE; 4]).unwrap();
    let t = r.add_texture(TextureData::rgba8(1, 1, vec![255; 4]).unwrap()).unwrap();
    r.commit().unwrap();
    let mat = r.add_material(Material { lighting: render::Lighting::Unlit, ..Material::opaque(t) }).unwrap();
    let cam =
        Camera { eye: Vec3::new(0.0, 0.0, 2.0), target: Vec3::ZERO, up: Vec3::Y, fov_y: 1.0, near: 0.1, far: 10.0 };
    let (half_x, half_y) = (w as usize / 2, h as usize / 2);
    let lit = |px: &[u8], x: usize, y: usize| px[(y * w as usize + x) * 4] > 128;
    let clear = [0.0, 0.0, 0.0, 1.0];

    // Rigid: the quad covers the centre.
    let id = r.add_instance(skinned, mat, Mat4::IDENTITY).unwrap();
    r.render(&cam, clear).unwrap();
    let px = r.backend().read_pixels();
    assert!(lit(&px, half_x, half_y));

    // A bone that moves 0.6 to the right: the centre is empty, the right side covered.
    let mut bone = BoneMatrix::IDENTITY;
    bone.rows[0][3] = 0.6;
    r.set_bones(id, &[bone]).unwrap();
    r.set_bounding_sphere(id, Some((Vec3::new(0.6, 0.0, 0.0), 0.5))).unwrap();
    r.render(&cam, clear).unwrap();
    let px = r.backend().read_pixels();
    assert!(!lit(&px, half_x, half_y));
    // x = 0.6 at distance 2 with a 1-radian field of view: 0.6 / (2 tan 0.5) of the half width right of centre.
    let right = half_x + (0.6 / (2.0 * 0.5463) * half_x as f32) as usize;
    assert!(lit(&px, right, half_y), "skinned quad not at x {right}");

    // Morphed positions (a bigger quad, still skinned by the bone): covers the centre again and the right.
    let big = quad(0.8);
    let pos: Vec<[f32; 3]> = big.iter().map(|v| v.pos).collect();
    r.set_morph(id, &pos, &[n; 4]).unwrap();
    r.set_bounding_sphere(id, Some((Vec3::new(0.6, 0.0, 0.0), 1.0))).unwrap();
    r.render(&cam, clear).unwrap();
    let px = r.backend().read_pixels();
    assert!(lit(&px, half_x, half_y) && lit(&px, right, half_y));

    // No weights: the bone does not move the vertices.
    r.remove_instance(id).unwrap();
    let id = r.add_instance(unweighted, mat, Mat4::IDENTITY).unwrap();
    r.set_bones(id, &[bone]).unwrap();
    r.render(&cam, clear).unwrap();
    let px = r.backend().read_pixels();
    assert!(lit(&px, half_x, half_y) && !lit(&px, right, half_y));
    drop(r);
    drop(ctx);
}

/// Outlines on the GPU: an edge whose two far corners lie on the same side of it on screen draws as a line; a
/// crease (corners on both sides) does not. UI quads: one draw, flat and textured.
#[cfg(target_os = "linux")]
#[test]
fn gl_outlines_and_ui() {
    use render::gl::egl::HeadlessGl;
    use render::gl::{GlBackend, GlTarget};
    use render::math::{Mat4, Vec3};
    use render::ui2d::{UiLayer, UiQuad};
    use render::{Camera, Material, Pass, Renderer, TextureData};

    let ctx = match HeadlessGl::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipped: no headless OpenGL context ({e})");
            return;
        }
    };
    let (w, h) = (64, 64);
    // SAFETY: `ctx` made a 4.3 core context current on this thread and outlives the backend.
    let backend = match unsafe {
        GlBackend::new(&mut |n| ctx.get_proc_address(n), GlTarget::Offscreen { width: w, height: h })
    } {
        Ok(b) => b,
        // A context without a working backend (a shader that does not compile) is a failure, not a skip.
        Err(e) => panic!("{e}"),
    };
    let mut r = Renderer::new(backend);
    // A vertical edge through the centre; far corners both to the right (silhouette) or one on each side (crease).
    let sil = r.add_outline_mesh(&[[[0.5, 0.2, 0.0], [0.0, -0.5, 0.0], [0.0, 0.5, 0.0], [0.5, -0.2, 0.0]]]).unwrap();
    let crease =
        r.add_outline_mesh(&[[[0.5, 0.2, 0.0], [0.0, -0.5, 0.0], [0.0, 0.5, 0.0], [-0.5, -0.2, 0.0]]]).unwrap();
    let t = r.add_texture(TextureData::rgba8(1, 1, vec![255; 4]).unwrap()).unwrap();
    let mut ui = UiLayer::new(&mut r, Some(TextureData::rgba8(2, 2, [0, 255, 0, 255].repeat(4)).unwrap()), 4).unwrap();
    r.commit().unwrap();
    let line =
        r.add_material(Material { tint: [255, 0, 0, 255], ..Material::opaque(t).with_pass(Pass::Outline) }).unwrap();
    let cam =
        Camera { eye: Vec3::new(0.0, 0.0, 2.0), target: Vec3::ZERO, up: Vec3::Y, fov_y: 1.0, near: 0.1, far: 10.0 };
    let red = |px: &[u8], x: usize, y: usize| {
        let p = &px[(y * w as usize + x) * 4..][..4];
        p[0] > 128 && p[1] < 64
    };
    let id = r.add_instance(sil, line, Mat4::IDENTITY).unwrap();
    r.set_outline_width(4.125 * 448.0 / 64.0); // 4 pixels at this size
    r.render(&cam, [0.0, 0.0, 0.0, 1.0]).unwrap();
    let px = r.backend().read_pixels();
    assert!(red(&px, 32, 32) && red(&px, 31, 32), "silhouette edge not drawn");
    assert!(!red(&px, 40, 32));
    r.remove_instance(id).unwrap();
    r.add_instance(crease, line, Mat4::IDENTITY).unwrap();
    r.render(&cam, [0.0, 0.0, 0.0, 1.0]).unwrap();
    let px = r.backend().read_pixels();
    assert!(!red(&px, 32, 32), "crease drawn");

    // UI: a flat blue quad under a textured green one, in list order.
    let quads = [
        UiQuad { rect: [0.0, 0.0, 32.0, 32.0], uv: [0.0; 4], textured: false, color: [0.0, 0.0, 1.0, 1.0] },
        UiQuad { rect: [16.0, 16.0, 32.0, 32.0], uv: [0.0, 0.0, 1.0, 1.0], textured: true, color: [1.0; 4] },
    ];
    ui.set_quads(&mut r, &quads).unwrap();
    let s = r.render(&cam, [0.0, 0.0, 0.0, 1.0]).unwrap();
    assert_eq!(s.draw_commands, 2); // the crease outline and the UI
    let px = r.backend().read_pixels();
    let at = |x: usize, y: usize| px[(y * w as usize + x) * 4..][..3].to_vec();
    assert_eq!(at(4, 4), vec![0, 0, 255]);
    assert_eq!(at(24, 24), vec![0, 255, 0]);
    assert_eq!(at(60, 60), vec![0, 0, 0]);
    drop(r);
    drop(ctx);
}

/// Picture panels (own camera, viewport and clear), the iris wipe and a Sly 2 blend mode on the GPU.
#[cfg(target_os = "linux")]
#[test]
fn gl_panels_iris_and_blends() {
    use render::gl::egl::HeadlessGl;
    use render::gl::{GlBackend, GlTarget};
    use render::iris::IrisShape;
    use render::math::{Mat4, Vec3};
    use render::{BlendMode, Camera, Lighting, Material, MeshData, Panel, Pass, Renderer, TextureData, Vertex};

    let ctx = match HeadlessGl::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipped: no headless OpenGL context ({e})");
            return;
        }
    };
    let (w, h) = (64, 64);
    // SAFETY: `ctx` made a 4.3 core context current on this thread and outlives the backend.
    let backend = match unsafe {
        GlBackend::new(&mut |n| ctx.get_proc_address(n), GlTarget::Offscreen { width: w, height: h })
    } {
        Ok(b) => b,
        // A context without a working backend (a shader that does not compile) is a failure, not a skip.
        Err(e) => panic!("{e}"),
    };
    let mut r = Renderer::new(backend);
    let n = [0.0, 0.0, 1.0];
    let c = [255; 4];
    let v = [
        Vertex::new([-3.0, -3.0, 0.0], [0.0, 0.0], n, c),
        Vertex::new([3.0, -3.0, 0.0], [1.0, 0.0], n, c),
        Vertex::new([3.0, 3.0, 0.0], [1.0, 1.0], n, c),
        Vertex::new([-3.0, 3.0, 0.0], [0.0, 1.0], n, c),
    ];
    let quad = r.add_mesh(&MeshData { vertices: &v, indices: &[0, 1, 2, 0, 2, 3] }).unwrap();
    let t = r.add_texture(TextureData::rgba8(1, 1, vec![255; 4]).unwrap()).unwrap();
    // A 20 x 20 square (set 3, white) in a mask whose set 0 is blue.
    let pts = [[-10.0, -10.0], [10.0, -10.0], [10.0, 10.0], [-10.0, 10.0]];
    let sq: &[[u16; 3]] = &[[0, 1, 2], [0, 2, 3]];
    let colors = [[0, 0, 255, 255], [0; 4], [0; 4], [255, 255, 255, 255], [0; 4]];
    let iris = IrisShape::new(&mut r, &pts, [&[], &[], &[], sq, &[]], colors, [-10.0, -10.0, 10.0, 10.0]).unwrap();
    r.commit().unwrap();
    let green =
        r.add_material(Material { tint: [0, 255, 0, 255], lighting: Lighting::Unlit, ..Material::opaque(t) }).unwrap();
    r.add_instance(quad, green, Mat4::IDENTITY).unwrap();
    let cam =
        Camera { eye: Vec3::new(0.0, 0.0, 2.0), target: Vec3::ZERO, up: Vec3::Y, fov_y: 1.0, near: 0.1, far: 10.0 };
    let black = [0.0, 0.0, 0.0, 1.0];

    // A panel in the top right whose camera looks away from the quad: only its clear colour.
    let away = Camera { target: Vec3::new(0.0, 0.0, 10.0), ..cam };
    r.set_panels(&[Panel { camera: away, rect: [40.0, 0.0, 24.0, 24.0], clear: Some([1.0, 0.0, 0.0, 1.0]) }]).unwrap();
    r.render(&cam, black).unwrap();
    let px = r.backend().read_pixels();
    let at = |px: &[u8], x: usize, y: usize| px[(y * w as usize + x) * 4..][..3].to_vec();
    assert_eq!(at(&px, 50, 10), vec![255, 0, 0], "panel clear");
    assert_eq!(at(&px, 10, 10), vec![0, 255, 0]);
    assert_eq!(at(&px, 50, 40), vec![0, 255, 0], "panel spilled below its rectangle");
    // The same panel looking at the quad: the world again inside it.
    r.set_panels(&[Panel { camera: cam, rect: [40.0, 0.0, 24.0, 24.0], clear: Some([1.0, 0.0, 0.0, 1.0]) }]).unwrap();
    r.render(&cam, black).unwrap();
    assert_eq!(at(&r.backend().read_pixels(), 50, 10), vec![0, 255, 0]);
    r.set_panels(&[]).unwrap();

    // Iris 40 % closed around (16, 48), one pixel per unit: the square white, outside the box the blue of set 0
    // darkened by 40 %.
    iris.set(&mut r, [16.0, 48.0], 1.0, 0.4).unwrap();
    r.render(&cam, black).unwrap();
    let px = r.backend().read_pixels();
    assert_eq!(at(&px, 16, 48), vec![255, 255, 255]);
    assert_eq!(at(&px, 2, 2), vec![0, 0, 153]);
    assert_eq!(at(&px, 50, 48), vec![0, 0, 153]);
    iris.hide(&mut r).unwrap();

    // Reverse additive over the green quad: src + dst * src_alpha = red + green / 2.
    let over = r
        .add_material(Material {
            tint: [255, 0, 0, 128],
            blend: BlendMode::ReverseAdd,
            lighting: Lighting::Unlit,
            ..Material::opaque(t).with_pass(Pass::Blend)
        })
        .unwrap();
    r.add_instance(quad, over, Mat4::translation(Vec3::new(0.0, 0.0, 0.5))).unwrap();
    r.render(&cam, black).unwrap();
    let p = at(&r.backend().read_pixels(), 32, 32);
    assert!(p[0] == 255 && (126..=130).contains(&p[1]) && p[2] == 0, "{p:?}");
    drop(r);
    drop(ctx);
}

/// A projected shadow (rendering.md 9.2): an invisible caster above a receiving floor darkens the floor under it.
#[cfg(target_os = "linux")]
#[test]
fn gl_projected_shadow() {
    use render::gl::egl::HeadlessGl;
    use render::gl::{GlBackend, GlTarget};
    use render::math::{Mat4, Vec3};
    use render::{Camera, Lighting, Material, MeshData, Pass, Projector, Renderer, TextureData, Vertex};

    let ctx = match HeadlessGl::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipped: no headless OpenGL context ({e})");
            return;
        }
    };
    let (w, h) = (64, 64);
    // SAFETY: `ctx` made a 4.3 core context current on this thread and outlives the backend.
    let backend = match unsafe {
        GlBackend::new(&mut |n| ctx.get_proc_address(n), GlTarget::Offscreen { width: w, height: h })
    } {
        Ok(b) => b,
        // A context without a working backend (a shader that does not compile) is a failure, not a skip.
        Err(e) => panic!("{e}"),
    };
    let mut r = Renderer::new(backend);
    let quad = |r: &mut Renderer<GlBackend>, s: f32| {
        let n = [0.0, 0.0, 1.0];
        let c = [255; 4];
        let v = [
            Vertex::new([-s, -s, 0.0], [0.0, 0.0], n, c),
            Vertex::new([s, -s, 0.0], [1.0, 0.0], n, c),
            Vertex::new([s, s, 0.0], [1.0, 1.0], n, c),
            Vertex::new([-s, s, 0.0], [0.0, 1.0], n, c),
        ];
        r.add_mesh(&MeshData { vertices: &v, indices: &[0, 1, 2, 0, 2, 3] }).unwrap()
    };
    let floor = quad(&mut r, 500.0);
    let small = quad(&mut r, 20.0);
    let t = r.add_texture(TextureData::rgba8(1, 1, vec![255; 4]).unwrap()).unwrap();
    r.commit().unwrap();
    let green =
        r.add_material(Material { tint: [0, 255, 0, 255], lighting: Lighting::Unlit, ..Material::opaque(t) }).unwrap();
    // The caster is fully transparent in the frame; the silhouette uses its shape only.
    let clear = r
        .add_material(Material {
            tint: [255, 255, 255, 0],
            lighting: Lighting::Unlit,
            ..Material::opaque(t).with_pass(Pass::Blend)
        })
        .unwrap();
    let f = r.add_instance(floor, green, Mat4::IDENTITY).unwrap();
    r.set_receiver(f, 1).unwrap();
    let c = r.add_instance(small, clear, Mat4::translation(Vec3::new(0.0, 0.0, 50.0))).unwrap();
    r.set_shadow_caster(c, true).unwrap();
    r.set_projectors(&[Projector {
        origin: Vec3::new(0.0, 0.0, 100.0),
        near_radius: 100.0,
        far_radius: 100.0,
        near_distance: 0.0,
        far_distance: 500.0,
        strength: 0.5,
        receivers: 1,
    }])
    .unwrap();
    let cam =
        Camera { eye: Vec3::new(0.0, 0.0, 300.0), target: Vec3::ZERO, up: Vec3::Y, fov_y: 1.0, near: 1.0, far: 1000.0 };
    r.render(&cam, [0.0, 0.0, 0.0, 1.0]).unwrap();
    let px = r.backend().read_pixels();
    let at = |x: usize, y: usize| px[(y * w as usize + x) * 4..][..3].to_vec();
    let mid = at(32, 32);
    assert!(mid[0] == 0 && (120..=136).contains(&mid[1]), "shadow under the caster: {mid:?}");
    assert_eq!(at(2, 2), vec![0, 255, 0], "no shadow away from it");
    // Without the projector the floor is lit everywhere.
    r.set_projectors(&[]).unwrap();
    r.render(&cam, [0.0, 0.0, 0.0, 1.0]).unwrap();
    assert_eq!(r.backend().read_pixels()[(32 * 64 + 32) * 4 + 1], 255);
    drop(r);
    drop(ctx);
}

/// A movie frame on the GPU: the planes upload and the shader converts them (BT.601, video range).
#[cfg(target_os = "linux")]
#[test]
fn gl_video_frame() {
    use render::gl::egl::HeadlessGl;
    use render::gl::{GlBackend, GlTarget};
    use render::math::Vec3;
    use render::video::{VideoFrame, VideoLayer, yuv_to_rgb};
    use render::{Camera, Renderer};

    let ctx = match HeadlessGl::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipped: no headless OpenGL context ({e})");
            return;
        }
    };
    let (w, h) = (64, 64);
    // SAFETY: `ctx` made a 4.3 core context current on this thread and outlives the backend.
    let backend = match unsafe {
        GlBackend::new(&mut |n| ctx.get_proc_address(n), GlTarget::Offscreen { width: w, height: h })
    } {
        Ok(b) => b,
        // A context without a working backend (a shader that does not compile) is a failure, not a skip.
        Err(e) => panic!("{e}"),
    };
    let mut r = Renderer::new(backend);
    let layer = VideoLayer::new(&mut r).unwrap();
    r.commit().unwrap();
    // 8 x 4 picture: left half red, right half blue (constant per chroma block).
    let (red, blue) = ((81u8, 90u8, 240u8), (41u8, 240u8, 110u8));
    let mut y = vec![0u8; 32];
    for row in 0..4 {
        for col in 0..8 {
            y[row * 8 + col] = if col < 4 { red.0 } else { blue.0 };
        }
    }
    let u: Vec<u8> = (0..8).map(|i| if i % 4 < 2 { red.1 } else { blue.1 }).collect();
    let v: Vec<u8> = (0..8).map(|i| if i % 4 < 2 { red.2 } else { blue.2 }).collect();
    r.set_video_frame(Some(VideoFrame { width: 8, height: 4, y: &y, u: &u, v: &v })).unwrap();
    layer.show(&mut r, [0.0, 0.0, 64.0, 64.0], [1.0; 4]).unwrap();
    let cam = Camera::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO);
    r.render(&cam, [0.0, 0.0, 0.0, 1.0]).unwrap();
    let px = r.backend().read_pixels();
    let at = |x: usize, y: usize| px[(y * w as usize + x) * 4..][..3].to_vec();
    let close = |a: Vec<u8>, b: [u8; 3]| a.iter().zip(b).all(|(&p, q)| (p as i32 - q as i32).abs() <= 6);
    assert!(close(at(8, 32), yuv_to_rgb(red.0, red.1, red.2)), "{:?}", at(8, 32));
    assert!(close(at(56, 32), yuv_to_rgb(blue.0, blue.1, blue.2)), "{:?}", at(56, 32));
    // The next frame keeps the picture without a new upload.
    r.render(&cam, [0.0, 0.0, 0.0, 1.0]).unwrap();
    assert_eq!(r.backend().read_pixels(), px);
    drop(r);
    drop(ctx);
}

/// A picture panel drawn into a panel image and shown in its frame: the picture inside the shape (under the bands),
/// the frame's scene outside the round end.
#[cfg(target_os = "linux")]
#[test]
fn gl_panel_image_in_frame() {
    use render::gl::egl::HeadlessGl;
    use render::gl::{GlBackend, GlTarget};
    use render::math::Vec3;
    use render::panel::{PanelFrames, Side};
    use render::{Camera, Renderer};

    let ctx = match HeadlessGl::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipped: no headless OpenGL context ({e})");
            return;
        }
    };
    let (w, h) = (320, 224);
    // SAFETY: `ctx` made a 4.3 core context current on this thread and outlives the backend.
    let backend = match unsafe {
        GlBackend::new(&mut |n| ctx.get_proc_address(n), GlTarget::Offscreen { width: w, height: h })
    } {
        Ok(b) => b,
        // A context without a working backend (a shader that does not compile) is a failure, not a skip.
        Err(e) => panic!("{e}"),
    };
    let mut r = Renderer::new(backend);
    let frames = PanelFrames::new(&mut r).unwrap();
    r.commit().unwrap();
    let cam = Camera::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO);
    // Half scale: the panel is 80 x 62.7 pixels at the bottom left.
    let rect = PanelFrames::rect((w, h), Side::Left, 0.5);
    r.set_image_panels(&[PanelFrames::view(Side::Left, cam, rect, [1.0, 1.0, 0.0, 1.0])]).unwrap();
    frames.show(&mut r, Side::Left, rect, [0, 0, 0, 255]).unwrap();
    r.render(&cam, [0.0, 0.0, 1.0, 1.0]).unwrap();
    let px = r.backend().read_pixels();
    let at = |x: f32, y: f32| px[((y as usize) * w as usize + x as usize) * 4..][..3].to_vec();
    // Inside, near the straight side: the yellow picture under a band (black or grey at 48/128).
    let p = at(rect[0] + 10.0, rect[1] + rect[3] * 0.5);
    assert!(p[0] > 150 && p[1] > 150 && p[2] < 60, "picture: {p:?}");
    // The top right corner of the rectangle is outside the round end: the frame's blue.
    assert_eq!(at(rect[0] + rect[2] - 2.0, rect[1] + 2.0), vec![0, 0, 255]);
    // Away from the panel: blue.
    assert_eq!(at(300.0, 20.0), vec![0, 0, 255]);
    drop(r);
    drop(ctx);
}
