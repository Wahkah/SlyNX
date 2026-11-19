//! Times the mixer: 32 looping voices (synthetic ADPCM, no game data) rendered in 10 ms blocks.
//!
//! ```sh
//! cargo run --release -p audio --example mixer_bench
//! ```

use std::time::Instant;

use audio::adpcm::{FLAG_END, FLAG_LOOP_START, FLAG_REPEAT};
use audio::envelope::{AdsrWords, Envelope};
use audio::mixer::{Mixer, Owner, Pool, SampleData, Source, TICK_FRAMES, VoiceStart};

struct Data(Vec<u8>);

impl SampleData for Data {
    fn data(&self, _: Source) -> &[u8] {
        &self.0
    }
}

fn main() {
    // 64 blocks of pseudo-random codes, looping.
    let mut seed = 0x1234_5678u32;
    let mut data = Vec::new();
    for b in 0..64 {
        let flags = match b {
            0 => FLAG_LOOP_START,
            63 => FLAG_END | FLAG_REPEAT,
            _ => 0,
        };
        data.push(0x18); // filter 1, shift 8
        data.push(flags);
        for _ in 0..14 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            data.push((seed >> 24) as u8);
        }
    }
    let data = Data(data);
    for voices in [8usize, 32, 48] {
        let mut m = Mixer::new(voices, 0, 0);
        m.reverb.set(3, 10_000);
        for i in 0..voices {
            let v = m.alloc(Pool::Effects, 0).unwrap();
            m.start(
                v,
                VoiceStart {
                    source: Source::Bank(0),
                    offset: 0,
                    envelope: Envelope::new(AdsrWords { w1: 0x80FF, w2: 0x9FC0 }),
                    owner: Owner::Effect { instance: 0 },
                    priority: 0,
                    group: 0,
                    reverb: i % 2 == 0,
                    volume: 1.0,
                    pan: 0.0,
                    semitones: 0.0,
                    rate_scale: 1.0,
                    bend_down: 2.0,
                    bend_up: 2.0,
                    wrap: false,
                },
            );
        }
        let blocks = 2000; // 20 s of audio
        let mut out = vec![0.0f32; 480 * 2];
        let start = Instant::now();
        let mut ticks = 0usize;
        let mut to_tick = 0usize;
        for _ in 0..blocks {
            out.fill(0.0);
            let mut done = 0;
            while done < 480 {
                if to_tick == 0 {
                    // What the renderer does every tick: new targets for every voice.
                    for i in 0..voices {
                        let ratio = 0.5 + (i as f64) * 0.05;
                        m.set_target(i, 0.02, (i as f32) * 11.0, ratio, 0.02);
                    }
                    to_tick = TICK_FRAMES;
                    ticks += 1;
                }
                let n = (480 - done).min(to_tick);
                m.mix(&data, &mut out[done * 2..(done + n) * 2], n);
                done += n;
                to_tick -= n;
            }
            std::hint::black_box(&out);
        }
        let per_block = start.elapsed().as_secs_f64() / blocks as f64;
        let active = m.voices.iter().filter(|v| v.active).count();
        println!(
            "{voices} voices ({active} still active, {ticks} ticks): {:.1} us per 10 ms block ({:.2}% of real time)",
            per_block * 1e6,
            per_block / 0.010 * 100.0
        );
    }
}
