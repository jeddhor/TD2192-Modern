//! Spec test vectors that no other test asserted (VE-9, M12 checklist §0): each test names its
//! spec, revision and vector. Static vectors only; the L5 comparisons are in the `oracle_*`
//! tests.

use td_sim::{BpRandom, Real48};

/// core/bp7-runtime-semantics rev 2 §6 vector 8: three Real `Random` calls from seed 0 give
/// these Real48 values (exponent byte first): 2⁻³², 0.0313799395…, 0.8610484672….
#[test]
fn bp7_vector_8_real_random_from_seed_0() {
    let mut r = BpRandom::new();
    let got: Vec<[u8; 6]> = (0..3).map(|_| r.random_real().to_bytes()).collect();
    assert_eq!(
        got,
        [
            [0x61, 0x00, 0x00, 0x00, 0x00, 0x00],
            [0x7C, 0x00, 0x60, 0x40, 0x88, 0x00],
            [0x80, 0x00, 0x1F, 0xAC, 0x6D, 0x5C],
        ]
    );
    let v: Vec<f64> = got
        .iter()
        .map(|b| Real48::from_bytes(*b).to_f64())
        .collect();
    assert_eq!(v[0], 2f64.powi(-32));
    assert!((v[1] - 0.031_379_939_5).abs() < 1e-10, "{}", v[1]);
    assert!((v[2] - 0.861_048_467_2).abs() < 1e-10, "{}", v[2]);
}
