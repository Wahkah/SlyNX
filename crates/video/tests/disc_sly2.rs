//! Sly 2 movie tests against the player's own disc image (specs/sly2/movies.md). They skip, and pass, when
//! `SLYNX_DISC_SLY2` is not set.

mod common;

use common::{Vector, assert_vector, check};
use formats::hash::{Sha256, hex};
use formats_sly2::Disc;
use harness::golden::{Game, require_disc};
use video::movies::{Region, sly2_movies};
use video::{MoviePlayer, PlayerStatus};

const M45: u32 = 11_250;
const M50: u32 = 12_500;
const M55: u32 = 13_750;

/// specs/sly2/movies.md test vectors, in file-table order (records 23 and 27 are the same movie).
#[rustfmt::skip]
const VECTORS: [Vector; 23] = [
    Vector { identity: 0x7D9, sector: 1298436, bytes: 40452100, width: 720, bit_rate: M45, frame_rate_code: 4, pictures: 2584, audio_rate: 44100, audio_bytes: 4347136, sha256: "83c36d4fbc8f550f428bf5f4e6687e03ddadf663e63ba7e09b7f48618d9eb75e" },
    Vector { identity: 0x7DA, sector: 58380, bytes: 21381124, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 1310, audio_rate: 44100, audio_bytes: 2203904, sha256: "6da0bbff55e685ceb6ef58ebf4e81fb5d479edbe795b9c196ffaf2fce5db742e" },
    Vector { identity: 0x7DE, sector: 1326206, bytes: 42369028, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 2598, audio_rate: 44100, audio_bytes: 4370688, sha256: "bb475cf6b993cd004e6c878a1f83ced8bcaf13829e57bd1ca2cb28582c31c476" },
    Vector { identity: 0x7DF, sector: 92343, bytes: 20103172, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 1232, audio_rate: 44100, audio_bytes: 2072640, sha256: "ca59234ac8e5ba55a35cf22e1004b83680c36b8e80cbc6f0f77a577dc311fb41" },
    Vector { identity: 0x7E3, sector: 124137, bytes: 38912004, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 2383, audio_rate: 44100, audio_bytes: 4008960, sha256: "153955fa809cf6be3d528ef756ec3602b51cb24a1331fe583f9b698fb1470217" },
    Vector { identity: 0x7E4, sector: 220097, bytes: 33865732, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 2095, audio_rate: 44100, audio_bytes: 3524480, sha256: "5137f4aacb653254ac4878f7719547389829f8208a8ed49371c391b98e26325f" },
    Vector { identity: 0x7E7, sector: 260612, bytes: 46399492, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 2878, audio_rate: 44100, audio_bytes: 4841728, sha256: "024fbf9d421ca3c6886059a42672fac4c69d92f95d4a69fc1f606810ca8051c4" },
    Vector { identity: 0x7E8, sector: 36306, bytes: 25542660, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 1566, audio_rate: 44100, audio_bytes: 2634560, sha256: "94042801e89a7c9ac416684f94080e650a1eb9809f56e336194cd74586d97f54" },
    Vector { identity: 0x7EC, sector: 1268530, bytes: 35274756, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 2171, audio_rate: 44100, audio_bytes: 3652352, sha256: "81a6aa8efe1499ee7b1946a6ac1fddc5c4fad3c5728a69d8fc8cdaa601c9a154" },
    Vector { identity: 0x7ED, sector: 1285755, bytes: 25968644, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 1609, audio_rate: 44100, audio_bytes: 2706880, sha256: "aa588930150820e2ede6ea78f24205d9539579e69a781356f0c8ccb6b8ac0372" },
    Vector { identity: 0x7F1, sector: 102160, bytes: 45006852, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 2784, audio_rate: 44100, audio_bytes: 4683584, sha256: "eca6a167b45e1ae9f8d0c91fd0b59c96c72989bd7d7a468d6ad90564f5304e4b" },
    Vector { identity: 0x7F2, sector: 961247, bytes: 24248324, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 1484, audio_rate: 44100, audio_bytes: 2496576, sha256: "d898351ea1a23174cf95fd6465bc4bbb5d15fa13ace39085bd926be47afc951f" },
    Vector { identity: 0x7F6, sector: 878801, bytes: 37683204, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 2306, audio_rate: 44100, audio_bytes: 3879424, sha256: "facc3d665af5256163f6721db637cd9282e33720935bb56b3553447c7a645cf7" },
    Vector { identity: 0x7F7, sector: 236634, bytes: 28672004, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 1888, audio_rate: 44100, audio_bytes: 3176256, sha256: "289d83130d96cdf02c674b7d500316b7fd381f0b32b700bda6491abd49f9f8e3" },
    Vector { identity: 0x7FA, sector: 159131, bytes: 36126724, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 2275, audio_rate: 44100, audio_bytes: 3827264, sha256: "cd017f138b7c312172dbd7d91b4fcbe06f4bd363802c614d999874a1aba5521a" },
    Vector { identity: 0x7FB, sector: 1346895, bytes: 85835780, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 5261, audio_rate: 44100, audio_bytes: 8850624, sha256: "c3637c1c50990cabd9d3ad10f4b4d558fd0231aab9bba221297be55e06a1c21b" },
    Vector { identity: 0x80B, sector: 897202, bytes: 61325316, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 4034, audio_rate: 44100, audio_bytes: 6853312, sha256: "dade0dc290f36619bc89d1954428050f1635c2475fd36b866d5e2b4b6b2cec27" },
    Vector { identity: 0x802, sector: 204695, bytes: 9666564, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 600, audio_rate: 44100, audio_bytes: 1009088, sha256: "48821aaaf6654f67a679f31e2c2864c64d565c914864df715b73c75670be5cfb" },
    Vector { identity: 0x7D4, sector: 29785, bytes: 13352964, width: 640, bit_rate: M50, frame_rate_code: 4, pictures: 927, audio_rate: 48000, audio_bytes: 1696896, sha256: "cad16e34205bb8610c1b9f3132121b7aeac8352920ec7b50d24076efde0894d7" },
    Vector { identity: 0x801, sector: 973088, bytes: 345489412, width: 640, bit_rate: M55, frame_rate_code: 4, pictures: 17345, audio_rate: 48000, audio_bytes: 31748288, sha256: "7e9942caed0d028bc9ceb7a93ffdfead44a99d0e37c5d980e6bba9f41f664866" },
    Vector { identity: 0x7D5, sector: 1141785, bytes: 56918020, width: 640, bit_rate: M45, frame_rate_code: 4, pictures: 4016, audio_rate: 44100, audio_bytes: 6756160, sha256: "c92cdd47ada24a9fab38b342e817c0755444a6f1c1dccb0c1db938588c708e01" },
    Vector { identity: 0x808, sector: 959782, bytes: 2998276, width: 640, bit_rate: M50, frame_rate_code: 4, pictures: 132, audio_rate: 44100, audio_bytes: 222144, sha256: "c1b6125e5d24a49ddf517cb1165099110914377a3b43bd6c661dcd68ef6d2963" },
    Vector { identity: 0x80A, sector: 1388808, bytes: 6275076, width: 640, bit_rate: M50, frame_rate_code: 4, pictures: 277, audio_rate: 44100, audio_bytes: 466112, sha256: "9352a412169cba61ceab8df4b882fabfb6738f987c6f063b41ec1aa5fcea4a7e" },
];

#[test]
fn every_movie_with_its_first_and_last_gop() {
    let Some(path) = require_disc(Game::Sly2, "every_movie_with_its_first_and_last_gop") else { return };
    let disc = Disc::open_path(&path).expect("open disc");
    let movies = sly2_movies(&disc.files);
    assert_eq!(movies.len(), 23);
    assert_eq!(
        movies.iter().map(|m| m.0).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 23, 28]
    );
    let results = std::thread::scope(|s| {
        let hs: Vec<_> = movies
            .iter()
            .map(|(_, f)| {
                let path = &path;
                s.spawn(move || check(path, f, f.identity == 0x808))
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("movie thread")).collect::<Vec<_>>()
    });
    for (((i, f), v), c) in movies.iter().zip(&VECTORS).zip(&results) {
        let id = format!("record {i} ({:#x})", f.identity);
        assert_vector(&id, f, v, c);
        // The audio lasts as long as the pictures, to within 0.1 s (movies.md), except 0x80B, whose audio is 1.38 s
        // longer by the spec's own sizes (a spec finding).
        let video_s = v.pictures as f64 * 1001.0 / 30000.0;
        let audio_s = c.audio_frames as f64 / f64::from(v.audio_rate);
        let tolerance = if f.identity == 0x80B { 1.4 } else { 0.1 };
        assert!((audio_s - video_s).abs() < tolerance, "{id}: audio {audio_s} s, video {video_s} s");
        assert_eq!(&c.first_audio_payload[4..8], &[0x18, 0, 0, 0], "{id}");
    }
    // 0x808: 194,376 samples per channel, left and right identical, the left channel's SHA-256.
    let (k, _) = movies.iter().enumerate().find(|(_, m)| m.1.identity == 0x808).expect("0x808");
    let c = &results[k];
    assert_eq!(c.left.len(), 194_376);
    assert!(c.left_equals_right);
    let mut h = Sha256::new();
    for s in &c.left {
        h.update(&s.to_le_bytes());
    }
    assert_eq!(hex(&h.finish()), "9b4584d0fc117ec50e1761adca5cfdbd7b29246648590fa7c1a7145a9434c908");
}

#[test]
fn player_resamples_to_48_khz() {
    let Some(path) = require_disc(Game::Sly2, "player_resamples_to_48_khz") else { return };
    let disc = Disc::open_path(&path).expect("open disc");
    let (_, f) = sly2_movies(&disc.files).into_iter().find(|m| m.1.identity == 0x808).expect("0x808");
    let file = std::fs::File::open(&path).expect("open image");
    let mut p = MoviePlayer::new(Region::of_file_record(file, &f));
    let h = p.audio_header().expect("read").expect("audio header");
    assert_eq!((h.sample_rate, h.format), (44_100, 0x10));
    assert_eq!(p.audio_rate().expect("read"), Some(48_000));
    // A 60 Hz game: 800 frames of 48 kHz audio per update, the clock from the audio played.
    let mut played = 0u64;
    let mut buf = vec![0i16; 1600];
    let mut shown = 0;
    let mut status = PlayerStatus::Playing;
    for _ in 0..60 * 10 {
        let n = p.read_audio(&mut buf).expect("audio");
        played += n as u64 / 2;
        status = p.update(played * 1_000_000_000 / 48_000).expect("update");
        shown = p.frame().map_or(0, |f| f.info.display_index + 1);
        if status == PlayerStatus::Ended {
            break;
        }
    }
    assert_eq!(status, PlayerStatus::Ended);
    assert_eq!(shown, 132);
    loop {
        let n = p.read_audio(&mut buf).expect("audio");
        if n == 0 {
            break;
        }
        played += n as u64 / 2;
    }
    // 194,376 frames at 44.1 kHz are 211,566 at 48 kHz.
    let want = 194_376u64 * 48_000 / 44_100;
    assert!(played.abs_diff(want) <= 2, "{played} vs {want}");
    assert_eq!(p.decoder_stats().slice_errors, 0);
}
