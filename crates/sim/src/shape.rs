//! TWR-SIM · SHAPE
//!   · Base: the plan-view outlines every type is drawn from
//!   · DIMS: each type's length and span (`research/out/aircraft/types.csv`)
//!   · outline: a type's full silhouette in metres, ready to turn and draw
//!
//! Four outlines are traced from manufacturers' airport-planning sheets
//! (`research/out/aircraft/geometry/`, each file names its sheet). Every
//! other type is one of them stretched to its own length and span, so a
//! 737 is an A320 shape at 737 size: its nose and tail are approximate.
//! The turboprop outline is `authored`, drawn by hand to ATR 72
//! proportions, not traced. Stretching sets the drawn span to `types.csv`
//! exactly, whatever the trace measured.

/// A base outline: the right half, nose tip at (0, 0), x aft, y right, in
/// metres; the first and last points sit on the centreline. `nose_gear` is
/// the nose gear's x, the point the aircraft's position marks.
pub struct Base {
    pub name: &'static str,
    pub nose_gear: f64,
    pub half: &'static [(f64, f64)],
}
/// Airbus A320neo, sharklets: traced, span 0.84% short of the sheet.
// (15.05, 6.28) is a traced point, not an approximation of TAU.
#[allow(clippy::approx_constant)]
pub const A20N: Base = Base {
    name: "A20N",
    nose_gear: 5.09,
    half: &[
        (0.0, 0.0), (0.02, 0.2), (0.32, 0.53), (0.63, 0.78), (1.25, 1.1), (2.6, 1.57),
        (4.03, 1.88), (5.55, 1.98), (11.72, 1.95), (12.95, 2.23), (14.05, 4.3), (12.18, 4.2),
        (11.3, 4.33), (11.18, 5.58), (11.22, 6.83), (12.5, 7.03), (13.85, 6.97), (14.75, 6.8),
        (14.8, 6.38), (15.05, 6.28), (20.18, 16.25), (20.52, 16.8), (20.95, 17.27), (21.35, 17.55),
        (22.8, 17.75), (23.35, 17.7), (22.4, 17.5), (22.2, 17.23), (20.73, 12.3), (21.27, 12.05),
        (20.6, 11.88), (19.57, 8.6), (19.6, 8.53), (19.98, 8.45), (20.02, 8.38), (19.45, 8.18),
        (18.93, 6.42), (18.93, 5.08), (19.4, 4.92), (19.32, 4.83), (18.93, 4.72), (18.93, 2.0),
        (28.02, 1.98), (30.23, 1.7), (32.08, 1.35), (32.38, 1.5), (32.63, 1.75), (35.33, 5.97),
        (35.75, 6.22), (36.75, 6.25), (35.58, 0.82), (37.4, 0.3), (37.6, 0.13), (37.6, 0.0),
    ],
};

/// Airbus A321neo, sharklets: traced, span 1.96% off the A20N trace of the same wing.
pub const A21N: Base = Base {
    name: "A21N",
    nose_gear: 5.12,
    half: &[
        (0.0, 0.0), (0.02, 0.25), (0.25, 0.55), (1.48, 1.23), (3.03, 1.73), (4.28, 1.95),
        (16.1, 2.0), (17.1, 2.2), (17.23, 2.33), (18.27, 4.38), (16.4, 4.28), (15.48, 4.42),
        (15.4, 5.05), (15.45, 6.95), (16.7, 7.13), (17.6, 7.13), (18.4, 7.03), (19.0, 6.9),
        (19.02, 6.45), (19.3, 6.35), (24.95, 17.25), (25.57, 17.85), (27.32, 18.1), (27.73, 18.05),
        (26.77, 17.85), (26.55, 17.55), (24.77, 11.47), (24.8, 11.38), (24.98, 11.38), (25.25, 11.22),
        (24.65, 11.05), (24.48, 10.32), (24.68, 10.2), (24.38, 10.1), (24.07, 9.0), (24.27, 8.82),
        (23.98, 8.72), (23.82, 8.13), (23.85, 8.05), (24.2, 7.97), (24.3, 7.88), (23.7, 7.7),
        (23.43, 6.78), (23.65, 6.6), (23.35, 6.53), (23.3, 5.45), (23.7, 5.4), (23.9, 5.25),
        (23.3, 5.1), (23.3, 4.55), (23.55, 4.47), (23.52, 4.4), (23.3, 4.35), (23.3, 2.58),
        (23.55, 2.58), (23.52, 2.5), (23.3, 2.45), (23.3, 2.02), (34.55, 1.98), (36.67, 1.77),
        (39.13, 1.27), (39.48, 1.55), (42.3, 6.05), (42.55, 6.25), (43.73, 6.25), (42.5, 0.8),
        (44.25, 0.38), (44.52, 0.23), (44.52, 0.0),
    ],
};

/// Airbus A321ceo, wing-tip fences (34.10 m): traced.
pub const A321: Base = Base {
    name: "A321",
    nose_gear: 5.12,
    half: &[
        (0.0, 0.0), (0.02, 0.25), (0.25, 0.55), (1.0, 1.0), (2.63, 1.6), (4.47, 1.98),
        (16.1, 2.0), (17.1, 2.2), (17.23, 2.33), (18.45, 4.72), (17.0, 4.63), (15.55, 4.72),
        (15.43, 4.88), (15.4, 5.08), (15.45, 6.63), (15.6, 6.72), (16.38, 6.8), (18.05, 6.8),
        (18.88, 6.67), (18.98, 6.4), (19.32, 6.38), (24.4, 16.2), (24.82, 16.68), (25.73, 17.02),
        (26.75, 16.98), (26.35, 16.88), (24.77, 11.47), (24.8, 11.38), (24.98, 11.38), (25.25, 11.22),
        (24.65, 11.05), (24.48, 10.32), (24.68, 10.2), (24.38, 10.1), (24.07, 9.0), (24.27, 8.82),
        (23.98, 8.72), (23.82, 8.07), (24.2, 7.97), (24.3, 7.88), (23.7, 7.7), (23.43, 6.78),
        (23.65, 6.6), (23.38, 6.55), (23.3, 6.33), (23.3, 5.45), (23.9, 5.3), (23.82, 5.2),
        (23.3, 5.1), (23.3, 4.55), (23.55, 4.47), (23.52, 4.4), (23.3, 4.35), (23.3, 2.58),
        (23.55, 2.58), (23.52, 2.5), (23.3, 2.45), (23.32, 2.0), (34.75, 1.98), (36.67, 1.77),
        (39.13, 1.27), (39.48, 1.55), (42.3, 6.05), (42.55, 6.25), (43.73, 6.25), (42.5, 0.8),
        (44.23, 0.38), (44.52, 0.23), (44.52, 0.0),
    ],
};

/// Boeing 777-300ER: traced, span +1.08%. Nose gear from the side view
/// on the same sheet (5.89 m). The base for every code E and F type.
pub const B77W: Base = Base {
    name: "B77W",
    nose_gear: 5.89,
    half: &[
        (0.0, 0.0), (0.0, 0.33), (0.23, 0.7), (1.02, 1.27), (1.75, 1.63), (4.15, 2.5),
        (7.22, 3.2), (8.95, 3.38), (26.85, 3.38), (30.0, 7.95), (28.9, 7.83), (27.52, 7.8),
        (25.6, 8.07), (25.52, 8.18), (25.57, 11.72), (25.7, 11.5), (27.45, 11.8), (29.0, 11.8),
        (29.85, 11.7), (30.8, 11.47), (30.93, 11.13), (31.98, 10.85), (41.75, 25.07), (45.9, 31.32),
        (46.15, 31.63), (48.4, 32.75), (49.3, 32.7), (47.45, 29.93), (44.63, 22.9), (40.58, 10.9),
        (40.23, 8.82), (40.23, 3.4), (55.95, 3.38), (60.3, 3.0), (63.22, 2.52), (64.05, 2.33),
        (64.47, 2.5), (64.85, 2.88), (71.2, 10.7), (71.6, 10.97), (72.33, 11.1), (73.85, 10.9),
        (73.97, 10.88), (73.97, 10.8), (70.28, 1.02), (73.08, 0.23), (73.15, 0.0),
    ],
};

/// `authored`: a high straight wing, nacelles ahead of it, a T-tail, at
/// ATR 72 length (27.16 m). Not traced; the nose gear x is a guess.
pub const TURBOPROP: Base = Base {
    name: "TURBOPROP",
    nose_gear: 3.0,
    half: &[
        (0.0, 0.0), (0.4, 0.7), (1.2, 1.1), (2.5, 1.35), (4.0, 1.42), (10.9, 1.42),
        (10.9, 3.5), (8.8, 3.55), (8.5, 4.1), (8.8, 4.65), (11.0, 4.7), (11.2, 13.52),
        (12.6, 13.52), (13.4, 4.7), (14.2, 4.6), (14.4, 4.1), (14.2, 3.6), (13.4, 3.5),
        (13.3, 1.42), (20.0, 1.42), (23.5, 1.0), (24.6, 0.8), (25.6, 3.65), (26.5, 3.65),
        (26.4, 0.6), (27.16, 0.0),
    ],
};

/// Length and span in metres, and the base drawn for each type in `TYPES`.
pub const DIMS: [(&str, f64, f64, &Base); 24] = [
    ("A20N", 37.57, 35.8, &A20N),
    ("A21N", 44.51, 35.8, &A21N),
    ("B38M", 39.53, 35.91, &A20N),
    ("AT76", 27.16, 27.04, &TURBOPROP),
    ("A320", 37.57, 35.8, &A20N),
    ("B738", 39.47, 35.78, &A20N),
    ("B788", 56.72, 60.14, &B77W),
    ("B77W", 73.86, 64.8, &B77W),
    ("B789", 62.82, 60.14, &B77W),
    ("A321", 44.51, 35.8, &A321),
    ("A333", 63.67, 60.29, &B77W),
    ("A359", 66.61, 64.75, &B77W),
    ("A319", 33.83, 35.81, &A20N),
    ("A343", 63.7, 60.29, &B77W),
    ("DH8D", 32.86, 28.44, &TURBOPROP),
    ("B737", 33.62, 35.78, &A20N),
    ("A388", 72.73, 79.75, &B77W),
    ("B78X", 68.31, 60.14, &B77W),
    ("A339", 63.66, 64.0, &B77W),
    ("E190", 36.24, 28.71, &A20N),
    ("B748", 76.26, 68.4, &B77W),
    ("A332", 58.82, 60.29, &B77W),
    ("AT46", 22.68, 24.6, &TURBOPROP),
    ("B739", 42.12, 35.78, &A20N),
];

/// The base's own span: twice its widest half.
fn base_span(b: &Base) -> f64 {
    2.0 * b.half.iter().fold(0.0_f64, |m, p| m.max(p.1))
}

/// Length, span and base for `icao`. A type not in `DIMS` draws as an A20N,
/// the commonest type at IGI, rather than as nothing.
pub fn dims(icao: &str) -> (f64, f64, &'static Base) {
    DIMS.iter().find(|d| d.0 == icao).map_or((37.57, 35.8, &A20N), |d| (d.1, d.2, d.3))
}

/// The full silhouette of `icao` as a closed ring of (forward, right)
/// points in metres, with the nose gear at (0, 0): forward is toward the
/// nose. Right half nose to tail, then the left half back to the nose.
pub fn outline(icao: &str) -> Vec<(f64, f64)> {
    let (len, span, b) = dims(icao);
    let blen = b.half.iter().fold(0.0_f64, |m, p| m.max(p.0));
    let (kx, ky) = (len / blen, span / base_span(b));
    let p = |&(x, y): &(f64, f64)| ((b.nose_gear - x) * kx, y * ky);
    let right: Vec<_> = b.half.iter().map(p).collect();
    let left = right.iter().rev().skip(1).take(right.len().saturating_sub(2)).map(|&(f, r)| (f, -r));
    right.iter().copied().chain(left).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traffic::TYPES;

    #[test]
    fn every_type_has_a_shape_at_its_published_span_and_length() {
        for t in &TYPES {
            let d = DIMS.iter().find(|d| d.0 == t.icao).unwrap_or_else(|| panic!("{} has no shape", t.icao));
            let o = outline(t.icao);
            let span = o.iter().fold(0.0_f64, |m, p| m.max(p.1)) - o.iter().fold(0.0_f64, |m, p| m.min(p.1));
            let len = o.iter().fold(f64::MIN, |m, p| m.max(p.0)) - o.iter().fold(f64::MAX, |m, p| m.min(p.0));
            assert!((span / d.2 - 1.0).abs() < 0.02, "{}: span {span:.2} vs {}", t.icao, d.2);
            assert!((len / d.1 - 1.0).abs() < 0.02, "{}: length {len:.2} vs {}", t.icao, d.1);
        }
    }

    #[test]
    fn nose_gear_sits_behind_the_nose_and_the_ring_is_symmetric() {
        let o = outline("A20N");
        let nose = o.iter().fold(f64::MIN, |m, p| m.max(p.0));
        assert!((nose - 5.09).abs() < 0.01, "nose {nose}");
        let sum: f64 = o.iter().map(|p| p.1).sum();
        assert!(sum.abs() < 1e-9, "left and right halves differ: {sum}");
    }
}
