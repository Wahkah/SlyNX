//! The benchmark on a PC, through `platform-pc`'s file system, with the player's own disc images linked into a
//! temporary SD folder. Skips each game whose `SLYNX_DISC_SLYn` is not set; only the first GOP of each movie is
//! decoded here.

use std::path::PathBuf;
use std::time::Instant;

use harness::golden::{Game, require_disc};
use platform_pc::PcFs;
use video_bench::{LOG_FILE, Settings, run};

#[cfg(unix)]
#[test]
fn bench_runs_through_the_platform_file_system() {
    let games =
        [Game::Sly1, Game::Sly2, Game::Sly3].map(|g| require_disc(g, "bench_runs_through_the_platform_file_system"));
    if games.iter().all(Option::is_none) {
        return;
    }
    let dir = std::env::temp_dir().join(format!("slynx-video-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let sd = dir.join("sd");
    std::fs::create_dir_all(sd.join("switch/SlyNX")).unwrap();
    for (i, iso) in games.iter().enumerate() {
        if let Some(iso) = iso {
            std::os::unix::fs::symlink(iso, sd.join(format!("switch/SlyNX/sly{}.iso", i + 1))).unwrap();
        }
    }
    let user: PathBuf = dir.join("user");
    std::fs::create_dir_all(&user).unwrap();
    std::fs::write(user.join("video-bench.cfg"), "frames = 15\n").unwrap();
    let fs = PcFs::new(dir.join("romfs"), sd, user.clone());
    let settings = Settings::load(&fs);
    assert_eq!(settings.frames, 15);
    let start = Instant::now();
    let now = || start.elapsed().as_nanos() as u64;
    let mut text = String::new();
    let mut log = |line: &str| {
        text.push_str(line);
        text.push('\n');
        platform_api::FileSystem::write_user(&fs, LOG_FILE, text.as_bytes()).unwrap();
    };
    let results = run(&fs, &settings, false, &now, &mut log, &mut || true);
    let expected: usize = [17, 23, 21].iter().zip(&games).filter(|(_, g)| g.is_some()).map(|(n, _)| n).sum();
    assert_eq!(results.len(), expected);
    for r in &results {
        assert_eq!(r.frame_ns.len(), 15, "sly{} movie {}", r.game, r.index);
        assert_eq!(r.slice_errors, 0, "sly{} movie {}", r.game, r.index);
        assert_eq!(r.height, 480);
    }
    let log = std::fs::read_to_string(user.join(LOG_FILE)).unwrap();
    assert!(log.contains("summary boost off"), "{log}");
    eprintln!("{log}");
    let _ = std::fs::remove_dir_all(&dir);
}
