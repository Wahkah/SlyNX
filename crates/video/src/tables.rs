//! Fixed tables of ISO/IEC 13818-2: scan orders (7.3), the default quantiser matrices (6.3.11) and the quantiser
//! scale table (7.4.2.2).

/// Zigzag scan (alternate_scan = 0): the scan index of each coefficient, in raster order (Figure 7-2).
const ZIGZAG_INDEX: [u8; 64] = [
    0, 1, 5, 6, 14, 15, 27, 28, //
    2, 4, 7, 13, 16, 26, 29, 42, //
    3, 8, 12, 17, 25, 30, 41, 43, //
    9, 11, 18, 24, 31, 40, 44, 53, //
    10, 19, 23, 32, 39, 45, 52, 54, //
    20, 22, 33, 38, 46, 51, 55, 60, //
    21, 34, 37, 47, 50, 56, 59, 61, //
    35, 36, 48, 49, 57, 58, 62, 63,
];

/// Alternate scan (alternate_scan = 1): the scan index of each coefficient, in raster order (Figure 7-3).
const ALTERNATE_INDEX: [u8; 64] = [
    0, 4, 6, 20, 22, 36, 38, 52, //
    1, 5, 7, 21, 23, 37, 39, 53, //
    2, 8, 19, 24, 34, 40, 50, 54, //
    3, 9, 18, 25, 35, 41, 51, 55, //
    10, 17, 26, 30, 42, 46, 56, 60, //
    11, 16, 27, 31, 43, 47, 57, 61, //
    12, 15, 28, 32, 44, 48, 58, 62, //
    13, 14, 29, 33, 45, 49, 59, 63,
];

const fn invert(index: &[u8; 64]) -> [u8; 64] {
    let mut out = [0u8; 64];
    let mut i = 0;
    while i < 64 {
        out[index[i] as usize] = i as u8;
        i += 1;
    }
    out
}

/// Raster position of each scan index, for `alternate_scan` 0 (`SCAN[0]`) and 1 (`SCAN[1]`).
pub const SCAN: [[u8; 64]; 2] = [invert(&ZIGZAG_INDEX), invert(&ALTERNATE_INDEX)];

/// Default intra quantiser matrix, raster order.
pub const DEFAULT_INTRA_MATRIX: [u8; 64] = [
    8, 16, 19, 22, 26, 27, 29, 34, //
    16, 16, 22, 24, 27, 29, 34, 37, //
    19, 22, 26, 27, 29, 34, 34, 38, //
    22, 22, 26, 27, 29, 34, 37, 40, //
    22, 26, 27, 29, 32, 35, 40, 48, //
    26, 27, 29, 32, 35, 40, 48, 58, //
    26, 27, 29, 34, 38, 46, 56, 69, //
    27, 29, 35, 38, 46, 56, 69, 83,
];

/// Default non-intra quantiser matrix: 16 everywhere.
pub const DEFAULT_NON_INTRA_MATRIX: [u8; 64] = [16; 64];

/// quantiser_scale for each quantiser_scale_code, for q_scale_type 0 (linear) and 1 (non-linear). Code 0 is
/// forbidden.
pub const QUANTISER_SCALE: [[u8; 32]; 2] = [
    [
        0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 44, 46, 48, 50, 52, 54, 56,
        58, 60, 62,
    ],
    [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 18, 20, 22, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64, 72, 80, 88, 96,
        104, 112,
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_are_permutations() {
        for scan in &SCAN {
            let mut seen = [false; 64];
            for &p in scan {
                assert!(!seen[p as usize]);
                seen[p as usize] = true;
            }
        }
        // Zigzag starts across the top row, then down the first column.
        assert_eq!(&SCAN[0][..6], &[0, 1, 8, 16, 9, 2]);
        // Alternate scan starts down the first column.
        assert_eq!(&SCAN[1][..6], &[0, 8, 16, 24, 1, 9]);
        assert_eq!(SCAN[0][63], 63);
        assert_eq!(SCAN[1][63], 63);
    }

    #[test]
    fn quantiser_scales_increase() {
        for t in &QUANTISER_SCALE {
            assert!(t.windows(2).all(|w| w[0] < w[1]));
        }
        assert_eq!(QUANTISER_SCALE[1][31], 112);
        assert_eq!(QUANTISER_SCALE[0][31], 62);
    }
}
