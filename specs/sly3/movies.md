---
id: sly3-formats-movies
title: Sly 3 movies - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97464 image; executable SCUS_974.64 SHA-256 ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: the 23 movie records read from the executable and every distinct movie scanned (sequence header, audio header, picture count, hash)
  - kind: disc
    what: same image, re-scan 2026-11-17 with picture start codes counted in the demultiplexed video stream only
    used_for: corrected Pictures and Seconds columns and the audio/picture comparison
reviewed_by: pending
date: 2026-11-14
---

# Sly 3 movies: differences from Sly 2

Read [../sly2/movies.md](../sly2/movies.md) first. Same container and audio encoding as Sly 2: MPEG-2 program
stream, video stream `0xE0`, audio in private stream 1 behind the sub-header `FF A1 00 00`, an `SShd` header with
format 16 (SPU ADPCM), 2 channels, **32-byte interleave**, then `SSbd` and the data; each file ends with an end code
(`00 00 01 B9`). [D] Tags as in [disc.md](disc.md).

## 1. Differences

| | Sly 2 | Sly 3 |
|---|---|---|
| List | file table, 24 records | executable movie table ([disc.md](disc.md) 5.2): 23 records, 21 distinct [D] |
| Video size | 640 × 480 (one 720 × 480) | 640 × 480 in all [D] |
| Frame rate | code 4 (29.97/s) | **code 5 (30/s)** in 20 movies; code 4 in one (identity `0x151`) [D] |
| Bit-rate field | 4.5 to 5.5 Mbit/s | 5.0 Mbit/s in all [D] |
| Audio rate | 44,100 (21), 48,000 (2) | 44,100 (20), 48,000 (1, identity `0x151`) [D] |

Aspect code 2 (4:3) in all. [D] Which worlds reference which movies is not yet surveyed.

## 2. Test vectors

"Pictures" counts picture start codes in the demultiplexed video elementary stream (stream `0xE0`) only; an earlier
version counted the start-code pattern anywhere in the file, including inside audio packets, and was 2 to 27 % too
high. "Seconds" is pictures / frame rate; "audio s" is the `SSbd` size / 2 / 16 × 28 / rate. [D]

| # | Identity | Sector | Bytes | Pictures | Seconds | Audio rate | Audio bytes | Audio s | SHA-256 |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---|
| 0 | `0x14F` | 444753 | 77496324 | 5434 | 181.13 | 44100 | 9230784 | 183.15 | `a0227479034c46fec18504778b5301600519a8c65a7eabe05c40cefbce8d423d` |
| 1 | `0x150` | 482594 | 44302340 | 3129 | 104.30 | 44100 | 5325824 | 105.67 | `aef8dbb2667c9a8a7e1b2679634de2c6d8c7190002cbb205e51f2a64c93ca701` |
| 2 | `0x151` | 504227 | 510345220 | 22372 | 746.48 (29.97/s) | 48000 | 40949824 | 746.48 | `053e8ebe827f62d0adeb2b92df0620af14e2a4c6045ee1035e89912700830291` |
| 3 | `0x130` | 1526531 | 61390852 | 4300 | 143.33 | 44100 | 7233920 | 143.53 | `92b934ace2fa76f6c6795622ead3c846858f4cb5656d56b1c0b06f47a6993564` |
| 4 | `0x12F` | 71778 | 54394884 | 3829 | 127.63 | 44100 | 6451392 | 128.00 | `021bd56c048b518e3ff7764380321e3c75f01334268b95cb2785b65c1d5b8131` |
| 5 | `0x14E` | 3232 | 70549508 | 4970 | 165.67 | 44100 | 8485632 | 168.37 | `6fbfcd80ba6d2af63e66dc0fa08188604eca1cd8ceb6bbaf704792282a61fb0f` |
| 6 | `0x134` | 1556508 | 55754756 | 3913 | 130.43 | 44100 | 6598720 | 130.93 | `f95e969ca0f7de9f0bd14cc911c1541ce734f2f2160a8d203c6ead1ac5332132` |
| 7 | `0x135` | 387661 | 27705348 | 1942 | 64.73 | 44100 | 3309248 | 65.66 | `00404d7093f60bbedfd15f40eec23f2d76fa544bd7c1fb5347f9248b3c506238` |
| 8 | `0x139` | 1583733 | 49987588 | 3511 | 117.03 | 44100 | 5907392 | 117.21 | `551c285fe9b0ad7b65730bb2951c56fa480ed3bb0d2b1fb180e6a71768cc4fb4` |
| 9 | `0x13A` | 332545 | 30507012 | 2146 | 71.53 | 44100 | 3695744 | 73.33 | `efbad8962dc87cac01c12a5ce47ea0dd4a987634f5f45ede094d91edba4e59b7` |
| 10 | `0x13E` | 1608142 | 63995908 | 4494 | 149.80 | 44100 | 7601152 | 150.82 | `583f77294963b3cecc67e9adc3ffc16127ac70df319e8c2c21a45842f4de69da` |
| 11 | `0x13F` | 253157 | 35815428 | 2518 | 83.93 | 44100 | 4267520 | 84.67 | `4099c318365c29bc27fbcc4b476339b33d7ef73715321a34d8dc5a55a06711a2` |
| 12 | `0x144` | 1639391 | 39616516 | 2778 | 92.60 | 44100 | 4746624 | 94.18 | `f830698db1a0ba5db419061bdea9a285d57dd0e651ffecefa7f57b284fb603cc` |
| 13 | `0x145` | 177776 | 34652164 | 2428 | 80.93 | 44100 | 4101888 | 81.39 | `fdc27b0c6cc98c9c95e5793e04ec1476fac5ad7ba84c8b020880afec6424cdf7` |
| 14 | `0x146` | 161495 | 33341444 | 2338 | 77.93 | 44100 | 3951552 | 78.40 | `e23d02ad9c7f3070e35d7906194d674f61ab0f132b7b29e605c95f9cad58959a` |
| 15 | `0x14A` | 1658736 | 45088772 | 3172 | 105.73 | 44100 | 5266560 | 104.50 | `12949bce32b1e31b6d4ace78d9695e7c752dbd38d1f58c760602f1cffa291511` |
| 16 | `0x14B` | 98339 | 28377092 | 1991 | 66.37 | 44100 | 3448512 | 68.42 | `57efbebc2ba866cc713e1fcb7e087b9439096bf7bb2146a2c87041e3b15fd62b` |
| 17 | `0x14C` | 1680753 | 49741828 | 3491 | 116.37 | 44100 | 5850368 | 116.08 | `5452146bbc61025a681e01c754d40b6955fcd3a33dfe4c34e5fb28cf58d64ad3` |
| 18 | `0x14D` | 37681 | 69828612 | 4894 | 163.13 | 44100 | 8382848 | 166.33 | `01a0a85f24c8094fd6c1d9c672ad002af86c2d443b6a099565b4af9e338b986a` |
| 19, 21 | `0x152` | 1705042 | 1376260 | 132 | 4.40 | 44100 | 222144 | 4.41 | `e8ca0371c28b4c08dd8d168eb85dc1efcbba82705bdcb7745286e4a615413ae3` |
| 20, 22 | `0x153` | 1705715 | 5832708 | 438 | 14.60 | 44100 | 736960 | 14.62 | `7b2bb9a67517b7d62408e47d2239554718ca52b072abbc50dc94ea2c797b8cc9` |

Movie 2 (`0x151`, 746 s, the only 29.97/s and 48 kHz one) is presumably the end credits [I]. Audio minus pictures, in seconds, per movie: 0x14F +2.02, 0x150 +1.37, 0x151 +0.00, 0x130 +0.20, 0x12F +0.37, 0x14E +2.70, 0x134 +0.50, 0x135 +0.93, 0x139 +0.18, 0x13A +1.80, 0x13E +1.02, 0x13F +0.74, 0x144 +1.58, 0x145 +0.46, 0x146 +0.47, 0x14A -1.23, 0x14B +2.05, 0x14C -0.29, 0x14D +3.20, 0x152 +0.01, 0x153 +0.02. The audio runs from 1.23 s shorter (`0x14A`) to 3.20 s longer (`0x14D`) than the pictures; for example `0x14E`'s audio is 2.70 s longer. [D]

## 3. Open questions

- Which worlds and screens start each movie.
- Whether the trailing audio after the last picture (up to 3.2 s, `0x14D`) plays over a held last frame or a black
  screen.
