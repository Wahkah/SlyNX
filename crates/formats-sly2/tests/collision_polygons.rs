//! Collision surfaces that are polygons (classes/solid-object.md 2.1) on the player's own Sly 2 (USA) disc
//! (`SLYNX_DISC_SLY2`); skipped when it is not set. The edges naming a stored surface (stored under it, or under a neighbour with it as the other side) can name sides whose ends lie
//! beyond its three corners: the surface is the convex hull, in its plane, of its corners and edge ends, and the
//! corner triangle alone leaves a gap a character falls through. `collision::with_polygons` covers the polygons; this test finds the gaps in
//! every world and checks that the stored surfaces keep their indices.

use formats_sly2::collision::{surface_polygon, with_polygons};
use formats_sly2::stream::{Body, Record, visit};
use formats_sly2::{Disc, World};

fn area(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
}

#[test]
fn polygon_surfaces_are_covered() {
    let Some(path) = std::env::var_os("SLYNX_DISC_SLY2").filter(|p| !p.is_empty()) else { return };
    let mut d = Disc::open_path(path).expect("disc");
    let rows: Vec<usize> = d.world_rows().collect();
    let (mut parts, mut polygons, mut added, mut gap_area) = (0usize, 0usize, 0usize, 0.0f64);
    let mut from_neighbours = 0usize;
    let mut largest = (0.0f32, 0usize, 0i32);
    let mut vector = false;
    for row in rows {
        let bytes = d.read_world(row).expect("world");
        let w = World::parse(&bytes, &d.options).expect("parse");
        for b in &w.bodies {
            visit(&b.objects, &mut |r: &Record| {
                let Body::Solid(s) = &r.body else { return };
                for p in &s.collision {
                    parts += 1;
                    let mesh = &p.mesh;
                    let out = with_polygons(mesh);
                    assert_eq!(
                        &out.triangles[..mesh.triangles.len()],
                        &mesh.triangles[..],
                        "stored surfaces keep their indices"
                    );
                    assert_eq!(out.positions, mesh.positions);
                    let extra = out.triangles.len() - mesh.triangles.len();
                    if extra == 0 {
                        continue;
                    }
                    added += extra;
                    let a: f32 = (mesh.triangles.len()..out.triangles.len())
                        .map(|t| {
                            let [x, y, z] = out.corners(t);
                            area(x, y, z)
                        })
                        .sum();
                    gap_area += a as f64;
                    // A surface's edges: stored under it and stored under a neighbour that names it.
                    let ends = mesh.edge_ends();
                    for (t, tri) in mesh.triangles.iter().enumerate() {
                        let own =
                            &mesh.edges[tri.first_edge as usize..(tri.first_edge + tri.edge_count as u32) as usize];
                        let by_own = own.iter().flat_map(|e| e.ends).any(|v| !tri.corners.contains(&v));
                        if ends[t].iter().any(|v| !tri.corners.contains(v)) {
                            polygons += 1;
                            if !by_own {
                                from_neighbours += 1;
                            }
                        }
                    }
                    if a > largest.0 {
                        largest = (a, row, r.id);
                    }
                    // Test vector (classes/solid-object.md 2.1): the cellar's solid 136751, surface 0 (corners 13, 9,
                    // 7), is a seven-sided polygon.
                    if row == 3 && r.id == 136751 && mesh.triangles.first().is_some_and(|t| t.corners == [13, 9, 7]) {
                        assert_eq!(surface_polygon(mesh, 0).len(), 7, "the test vector's polygon");
                        vector = true;
                    }
                }
            });
        }
    }
    eprintln!(
        "{parts} collision parts; {polygons} surfaces are polygons ({from_neighbours} only through a neighbour's edges); {added} fan triangles cover {gap_area:.0} square units; \
         largest gap {:.0} in row {} object {}",
        largest.0, largest.1, largest.2
    );
    assert!(polygons > 0, "Sly 2 stores polygon surfaces too");
    assert!(vector, "the cellar's solid 136751 was checked");
}
