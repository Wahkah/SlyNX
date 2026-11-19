//! Times collision queries on a world of the player's own disc.
//!
//! `cargo run --release -p engine --example collision_bench -- <disc.iso> [world]` (default: Paris, France). The
//! disc path may also come from `SLYNX_DISC_SLY1`.

use std::time::Instant;

use engine::collision::world::{CollisionWorld, QueryFilter};
use engine::math::{madd, normalize};
use formats::disc::Disc;
use formats::options::OptionTable;
use formats::world::World;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
    fn dir(&mut self) -> [f32; 3] {
        normalize([self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)])
    }
}

fn time<T>(label: &str, n: usize, mut f: impl FnMut(usize) -> T) {
    let mut sink = 0usize;
    let start = Instant::now();
    for i in 0..n {
        let r = f(i);
        sink = sink.wrapping_add(std::hint::black_box(&r) as *const T as usize & 1);
    }
    let el = start.elapsed().as_secs_f64();
    println!(
        "{label:<44} {:>10.0} per ms  ({:.3} us each){}",
        n as f64 / el / 1000.0,
        el * 1e6 / n as f64,
        if sink == usize::MAX { "!" } else { "" }
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().or_else(|| std::env::var("SLYNX_DISC_SLY1").ok()).expect("disc image path");
    let name = args.next().unwrap_or_else(|| "Paris, France".into());
    let mut disc = Disc::open_path(&path).expect("open disc");
    let index = disc.find_world(&name).expect("world");
    let bytes = disc.read_world(index).expect("read world");
    let world = World::load(&bytes).expect("parse world");
    let t = Instant::now();
    let cw = CollisionWorld::from_world(&world, &OptionTable::sly1()).expect("collision world");
    let tris: usize = cw.bodies().iter().map(|b| cw.shapes[b.shape.0 as usize].main.triangles.len()).sum();
    println!(
        "{name}: {} shapes, {} bodies, {tris} triangles placed, built in {:.1} ms",
        cw.shapes.len(),
        cw.bodies().len(),
        t.elapsed().as_secs_f64() * 1e3
    );
    let f = QueryFilter::default();
    let bounds = cw.bodies().iter().find(|b| b.fixed).map(|b| b.bounds()).expect("world mesh");

    // Points on walkable surfaces: drop rays from above the level.
    let mut rng = Lcg(1);
    let mut ground = Vec::new();
    while ground.len() < 2000 {
        let o =
            [rng.range(bounds.min[0], bounds.max[0]), rng.range(bounds.min[1], bounds.max[1]), bounds.max[2] + 10.0];
        if let Some(g) = cw.ground_probe(o, bounds.max[2] - bounds.min[2] + 20.0, &f)
            && g.is_ground
        {
            ground.push(madd(g.point, [0.0, 0.0, 1.0], 60.0));
        }
    }
    let n = 200_000;
    let dirs: Vec<[f32; 3]> = (0..4096).map(|_| rng.dir()).collect();
    let tops: Vec<[f32; 3]> = (0..4096)
        .map(|_| {
            [rng.range(bounds.min[0], bounds.max[0]), rng.range(bounds.min[1], bounds.max[1]), bounds.max[2] + 10.0]
        })
        .collect();
    let height = bounds.max[2] - bounds.min[2] + 20.0;

    let rate = |len: f32| {
        let hits = (0..10_000)
            .filter(|&i| cw.ray_cast(ground[i % ground.len()], dirs[i % dirs.len()], len, &f).is_some())
            .count();
        hits as f64 / 100.0
    };
    println!(
        "rays from near the ground that hit: {:.0} % within 1,000 units, {:.0} % within 10,000",
        rate(1000.0),
        rate(10_000.0)
    );
    time("ground probe, 60 units above ground, 200 down", n, |i| cw.ground_probe(ground[i % ground.len()], 200.0, &f));
    time("ray, from near the ground, 1,000 units", n, |i| {
        cw.ray_cast(ground[i % ground.len()], dirs[i % dirs.len()], 1000.0, &f)
    });
    time("ray, from near the ground, 10,000 units", n, |i| {
        cw.ray_cast(ground[i % ground.len()], dirs[i % dirs.len()], 10_000.0, &f)
    });
    time("ray, top of the level to the bottom", n, |i| cw.ground_probe(tops[i % tops.len()], height, &f));
    time("line of sight, 1,000 units", n, |i| {
        let o = ground[i % ground.len()];
        cw.segment_blocked(o, madd(o, dirs[i % dirs.len()], 1000.0), &f)
    });
    time("sphere sweep r 30, 100 units", n, |i| {
        cw.sphere_sweep(ground[i % ground.len()], 30.0, engine::math::scale(dirs[i % dirs.len()], 100.0), &f)
    });
    time("capsule sweep r 30 h 120, 100 units", n / 4, |i| {
        let a = ground[i % ground.len()];
        cw.capsule_sweep(a, madd(a, [0.0, 0.0, 1.0], 120.0), 30.0, engine::math::scale(dirs[i % dirs.len()], 100.0), &f)
    });

    // The world mesh alone: the collision tree against the triangle hierarchy.
    let wm = cw.bodies().iter().find(|b| b.fixed).unwrap();
    let mesh = &cw.shapes[wm.shape.0 as usize].main;
    let m = 20_000;
    time("world mesh only: hierarchy ray, 1,000 units", m, |i| {
        mesh.ray_cast(ground[i % ground.len()], dirs[i % dirs.len()], 1000.0, false)
    });
    time("world mesh only: collision-tree ray, 1,000 units", m, |i| {
        mesh.tree_ray_cast(ground[i % ground.len()], dirs[i % dirs.len()], 1000.0, false)
    });
}
