//! Renders the Paris music and a few effects from the player's own disc to WAV files.
//!
//! ```sh
//! cargo run --release -p audio --example render_paris -- <disc.iso> <output folder outside the repository>
//! ```
//!
//! Writes `paris_music_calm.wav` (60 s at the default intensity), `paris_music_alarm.wav` (60 s with an alarm
//! requested after 10 s), `paris_hiding.wav` (30 s hiding), and `effect_<id>.wav` for a few sound IDs. The files
//! are game-derived: keep them out of the repository.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use audio::disc::{bank_sound_counts, read_bank, read_music, world_audio};
use audio::{AudioRenderer, Config, Emitter, INTENSITY_ALARM, INTENSITY_HIDING, SoundEngine, create};
use formats::disc::Disc;

const RATE: u32 = 48_000;
/// Frames per render call, like the platform's audio thread (8 ms).
const BLOCK: usize = 384;
/// One game frame at 60 Hz, in output frames.
const FRAME: usize = 800;

fn write_wav(path: &Path, samples: &[f32]) -> std::io::Result<()> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, b)
}

/// Runs the game side once per 60 Hz frame and the renderer in 8 ms blocks for `seconds`; `each_frame` gets the
/// frame number.
fn run(
    engine: &mut SoundEngine,
    renderer: &mut AudioRenderer,
    seconds: f32,
    mut each_frame: impl FnMut(&mut SoundEngine, usize),
) -> (Vec<f32>, f64) {
    let total = (seconds * RATE as f32) as usize;
    let mut out = vec![0.0f32; total * 2];
    let mut done = 0;
    let mut next_frame = 0;
    let mut frame_no = 0;
    let mut render_time = 0.0;
    while done < total {
        if done >= next_frame {
            each_frame(engine, frame_no);
            engine.update(1.0 / 60.0);
            frame_no += 1;
            next_frame += FRAME;
        }
        let n = BLOCK.min(total - done).min(next_frame - done);
        let t = Instant::now();
        renderer.render(&mut out[done * 2..(done + n) * 2]);
        render_time += t.elapsed().as_secs_f64();
        done += n;
    }
    (out, render_time)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: render_paris <disc.iso> <output folder>");
        std::process::exit(2);
    }
    let out_dir = PathBuf::from(&args[2]);
    std::fs::create_dir_all(&out_dir).expect("create output folder");
    if harness::golden::is_inside_repository(&out_dir) {
        eprintln!("refusing to write game-derived audio inside the repository: {}", out_dir.display());
        std::process::exit(2);
    }
    let mut disc = Disc::open_path(&args[1]).expect("open disc");
    let paris = disc.find_world("Paris, France").expect("Paris world");
    let world = disc.read_world(paris).expect("read Paris");
    let (music_desc, bank_index) = world_audio(&world).expect("world layout");
    let music = Arc::new(read_music(&mut disc, &music_desc.expect("Paris has music")).expect("music"));
    println!(
        "Paris: music {} ({} instruments, {} segments), sound bank {bank_index}",
        String::from_utf8_lossy(&music.name),
        music.instruments.len(),
        music.segments.len()
    );
    let counts = bank_sound_counts(&mut disc).expect("bank counts");
    let bank0 = Arc::new(read_bank(&mut disc, 0).expect("bank 0"));
    let world_bank = (bank_index > 0).then(|| Arc::new(read_bank(&mut disc, bank_index as usize).expect("bank")));

    let setup = || {
        let (mut e, r) = create(Config::default());
        e.set_bank_counts(counts);
        e.load_bank(0, Some(bank0.clone()));
        if let Some(b) = &world_bank {
            e.load_bank(bank_index as usize, Some(b.clone()));
        }
        (e, r)
    };

    // Music at the default intensity.
    let (mut e, mut r) = setup();
    e.play_music(music.clone());
    let (pcm, t) = run(&mut e, &mut r, 60.0, |_, _| {});
    write_wav(&out_dir.join("paris_music_calm.wav"), &pcm).unwrap();
    println!(
        "paris_music_calm.wav: 60 s rendered in {:.1} ms; segments now {:?}",
        t * 1e3,
        r.sequencer().active_segments().collect::<Vec<_>>()
    );

    // Music with an alarm after 10 s, removed at 40 s.
    let (mut e, mut r) = setup();
    e.play_music(music.clone());
    let mut req = None;
    let (pcm, _) = run(&mut e, &mut r, 60.0, |e, f| {
        if f == 600 {
            req = Some(e.request_intensity(INTENSITY_ALARM));
        }
        if f == 2400
            && let Some(q) = req.take()
        {
            e.remove_intensity(q);
        }
    });
    write_wav(&out_dir.join("paris_music_alarm.wav"), &pcm).unwrap();
    println!("paris_music_alarm.wav");

    let (mut e, mut r) = setup();
    e.play_music(music.clone());
    e.request_intensity(INTENSITY_HIDING);
    let (pcm, _) = run(&mut e, &mut r, 30.0, |_, _| {});
    write_wav(&out_dir.join("paris_hiding.wav"), &pcm).unwrap();
    println!("paris_hiding.wav");

    // A few effects: bank 0's first sounds and the world bank's first sounds, 3 s each (continuous ones stopped
    // after 2 s).
    let base = counts.iter().take(bank_index.max(0) as usize).sum::<u16>();
    for id in [0u16, 1, 2, 5, 10, base, base + 1, base + 2] {
        let (mut e, mut r) = setup();
        let mut h = audio::SoundHandle::NONE;
        let (pcm, _) = run(&mut e, &mut r, 3.0, |e, f| {
            if f == 0 {
                h = e.play(id, Emitter::global());
            }
            if f == 120 {
                e.stop(h);
            }
        });
        let peak = pcm.iter().fold(0.0f32, |a, &x| a.max(x.abs()));
        write_wav(&out_dir.join(format!("effect_{id}.wav")), &pcm).unwrap();
        println!("effect_{id}.wav: peak {peak:.3}");
    }
    println!("written to {}", out_dir.display());
}
