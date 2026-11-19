//! Sly 3 movie tests against the player's own disc image (specs/sly3/movies.md). They skip, and pass, when
//! `SLYNX_DISC_SLY3` is not set.

mod common;

use common::{Vector, assert_vector, check};
use formats_sly3::Disc;
use harness::golden::{Game, require_disc};
use video::movies::sly3_movies;

const M50: u32 = 12_500;

/// specs/sly3/movies.md 2, in movie-table order (19/21 and 20/22 are the same movies). `pictures` is the count of
/// pictures in the video stream, which the decoder confirms; the spec's "Pictures" column counts the 00 00 01 00
/// patterns anywhere in the file (audio data included) and is 2 to 27 % higher (a spec finding).
#[rustfmt::skip]
const VECTORS: [Vector; 21] = [
    Vector { identity: 0x14F, sector: 444753, bytes: 77496324, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 5434, audio_rate: 44100, audio_bytes: 9230784, sha256: "a0227479034c46fec18504778b5301600519a8c65a7eabe05c40cefbce8d423d" },
    Vector { identity: 0x150, sector: 482594, bytes: 44302340, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 3129, audio_rate: 44100, audio_bytes: 5325824, sha256: "aef8dbb2667c9a8a7e1b2679634de2c6d8c7190002cbb205e51f2a64c93ca701" },
    Vector { identity: 0x151, sector: 504227, bytes: 510345220, width: 640, bit_rate: M50, frame_rate_code: 4, pictures: 22372, audio_rate: 48000, audio_bytes: 40949824, sha256: "053e8ebe827f62d0adeb2b92df0620af14e2a4c6045ee1035e89912700830291" },
    Vector { identity: 0x130, sector: 1526531, bytes: 61390852, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 4300, audio_rate: 44100, audio_bytes: 7233920, sha256: "92b934ace2fa76f6c6795622ead3c846858f4cb5656d56b1c0b06f47a6993564" },
    Vector { identity: 0x12F, sector: 71778, bytes: 54394884, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 3829, audio_rate: 44100, audio_bytes: 6451392, sha256: "021bd56c048b518e3ff7764380321e3c75f01334268b95cb2785b65c1d5b8131" },
    Vector { identity: 0x14E, sector: 3232, bytes: 70549508, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 4970, audio_rate: 44100, audio_bytes: 8485632, sha256: "6fbfcd80ba6d2af63e66dc0fa08188604eca1cd8ceb6bbaf704792282a61fb0f" },
    Vector { identity: 0x134, sector: 1556508, bytes: 55754756, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 3913, audio_rate: 44100, audio_bytes: 6598720, sha256: "f95e969ca0f7de9f0bd14cc911c1541ce734f2f2160a8d203c6ead1ac5332132" },
    Vector { identity: 0x135, sector: 387661, bytes: 27705348, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 1942, audio_rate: 44100, audio_bytes: 3309248, sha256: "00404d7093f60bbedfd15f40eec23f2d76fa544bd7c1fb5347f9248b3c506238" },
    Vector { identity: 0x139, sector: 1583733, bytes: 49987588, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 3511, audio_rate: 44100, audio_bytes: 5907392, sha256: "551c285fe9b0ad7b65730bb2951c56fa480ed3bb0d2b1fb180e6a71768cc4fb4" },
    Vector { identity: 0x13A, sector: 332545, bytes: 30507012, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 2146, audio_rate: 44100, audio_bytes: 3695744, sha256: "efbad8962dc87cac01c12a5ce47ea0dd4a987634f5f45ede094d91edba4e59b7" },
    Vector { identity: 0x13E, sector: 1608142, bytes: 63995908, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 4494, audio_rate: 44100, audio_bytes: 7601152, sha256: "583f77294963b3cecc67e9adc3ffc16127ac70df319e8c2c21a45842f4de69da" },
    Vector { identity: 0x13F, sector: 253157, bytes: 35815428, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 2518, audio_rate: 44100, audio_bytes: 4267520, sha256: "4099c318365c29bc27fbcc4b476339b33d7ef73715321a34d8dc5a55a06711a2" },
    Vector { identity: 0x144, sector: 1639391, bytes: 39616516, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 2778, audio_rate: 44100, audio_bytes: 4746624, sha256: "f830698db1a0ba5db419061bdea9a285d57dd0e651ffecefa7f57b284fb603cc" },
    Vector { identity: 0x145, sector: 177776, bytes: 34652164, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 2428, audio_rate: 44100, audio_bytes: 4101888, sha256: "fdc27b0c6cc98c9c95e5793e04ec1476fac5ad7ba84c8b020880afec6424cdf7" },
    Vector { identity: 0x146, sector: 161495, bytes: 33341444, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 2338, audio_rate: 44100, audio_bytes: 3951552, sha256: "e23d02ad9c7f3070e35d7906194d674f61ab0f132b7b29e605c95f9cad58959a" },
    Vector { identity: 0x14A, sector: 1658736, bytes: 45088772, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 3172, audio_rate: 44100, audio_bytes: 5266560, sha256: "12949bce32b1e31b6d4ace78d9695e7c752dbd38d1f58c760602f1cffa291511" },
    Vector { identity: 0x14B, sector: 98339, bytes: 28377092, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 1991, audio_rate: 44100, audio_bytes: 3448512, sha256: "57efbebc2ba866cc713e1fcb7e087b9439096bf7bb2146a2c87041e3b15fd62b" },
    Vector { identity: 0x14C, sector: 1680753, bytes: 49741828, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 3491, audio_rate: 44100, audio_bytes: 5850368, sha256: "5452146bbc61025a681e01c754d40b6955fcd3a33dfe4c34e5fb28cf58d64ad3" },
    Vector { identity: 0x14D, sector: 37681, bytes: 69828612, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 4894, audio_rate: 44100, audio_bytes: 8382848, sha256: "01a0a85f24c8094fd6c1d9c672ad002af86c2d443b6a099565b4af9e338b986a" },
    Vector { identity: 0x152, sector: 1705042, bytes: 1376260, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 132, audio_rate: 44100, audio_bytes: 222144, sha256: "e8ca0371c28b4c08dd8d168eb85dc1efcbba82705bdcb7745286e4a615413ae3" },
    Vector { identity: 0x153, sector: 1705715, bytes: 5832708, width: 640, bit_rate: M50, frame_rate_code: 5, pictures: 438, audio_rate: 44100, audio_bytes: 736960, sha256: "7b2bb9a67517b7d62408e47d2239554718ca52b072abbc50dc94ea2c797b8cc9" },
];

#[test]
fn every_movie_with_its_first_and_last_gop() {
    let Some(path) = require_disc(Game::Sly3, "every_movie_with_its_first_and_last_gop") else { return };
    let disc = Disc::open_path(&path).expect("open disc");
    assert_eq!(disc.movies.len(), 23);
    let movies = sly3_movies(&disc.movies);
    assert_eq!(movies.len(), 21);
    let results = std::thread::scope(|s| {
        let hs: Vec<_> = movies
            .iter()
            .map(|(_, f)| {
                let path = &path;
                s.spawn(move || check(path, f, false))
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("movie thread")).collect::<Vec<_>>()
    });
    for (((i, f), v), c) in movies.iter().zip(&VECTORS).zip(&results) {
        let id = format!("movie {i} ({:#x})", f.identity);
        assert_vector(&id, f, v, c);
        // "Audio s" of the spec: SSbd size / 2 / 16 x 28 / rate.
        let audio_s = c.audio_frames as f64 / f64::from(v.audio_rate);
        let spec_s = f64::from(v.audio_bytes) / 2.0 / 16.0 * 28.0 / f64::from(v.audio_rate);
        assert!((audio_s - spec_s).abs() < 0.001, "{id}");
    }
    // Movie 5 (0x14E): with its real picture count (4,970, 165.7 s at 30/s) the audio (168.4 s) is not 42 s short,
    // as specs/sly3/movies.md says from the raw count, but 2.7 s longer.
    let five = &results[5];
    let longer = five.audio_frames as f64 / 44_100.0 - 4_970.0 / 30.0;
    assert!((longer - 2.7).abs() < 0.1, "{longer}");
}
