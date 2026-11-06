//! The two-digit muzzle bytes of a direct start (weapons rev 5 §3.3, §3.3.2, §3.3.3; OB-017
//! preserved), built by `td_sim::flight::front_end` from the installation:
//!
//! - against the SHA-256 test vectors of weapons §3.3.2 and §3.3.3 (a) (needs TD2192_DATA);
//! - against the DGROUP of every whole-flight capture at tick 1, and constant in every record
//!   of the flight: VE-2's 20 E1 flights, VE-4's 20 E2 and 18 E3 flights (needs TD2192_DATA and
//!   TD2192_ORACLE). The harness is [`full_flight::check_world`].
//!
//! No captured bytes are stored.

mod full_flight;

use td_assets::{DataDir, ExeResources, ResourceValue};
use td_sim::flight::Episode;
use td_sim::flight::front_end::{TWO_DIGIT_HEADINGS, direct_start_world};
use td_test_support::{require_data, require_oracle, sha256_hex};

/// SHA-256 of the 272 bytes in heading order (0Ch first) of a direct start of `flight`.
fn hash_272(dir: &DataDir, ep: Episode, flight: u8) -> String {
    let res = ExeResources::load(dir).unwrap();
    let Some(ResourceValue::Bytes(table)) = res.get("flight.muzzle_offsets").cloned() else {
        panic!("muzzle table");
    };
    let world = direct_start_world(ep, flight, &table, |f| dir.read(f)).unwrap();
    let bytes: Vec<u8> = TWO_DIGIT_HEADINGS
        .iter()
        .flat_map(|&h| {
            let at = 34 * usize::from(h) + 2;
            world[at..at + 34].to_vec()
        })
        .collect();
    assert_eq!(bytes.len(), 272);
    sha256_hex(&bytes)
}

#[test]
fn e2_direct_starts_match_the_spec_vectors() {
    // weapons §3.3.2 test vector (a): the first scene card of each block.
    let data = require_data!();
    let dir = DataDir::open(&data).unwrap();
    let vectors: [(&[u8], &str); 9] = [
        (
            &[1, 16, 19],
            "5259d241f1500f5893d8a27d68467d367a376e9d37c03bbab9d9b2fc09f9e284",
        ),
        (
            &[2],
            "334b4d5faddac79f239dac13fa32a0f181452244b7120d81da56c995789b7d1c",
        ),
        (
            &[3, 10],
            "9f0d4186eca6774c7da2189921091b1f88ec6df2c2433ed6e0d6c817e4fefb52",
        ),
        (
            &[4, 8, 15],
            "261851e13643f97e4892a78efda65b3f73bea021d52fa4faf8f6fa2bb051327f",
        ),
        (
            &[5, 7],
            "175b62509de2e320f359ee469de56d9b4232f1e6e797edcd22692d9e5cd74017",
        ),
        (
            &[6],
            "016fa3f54284fef7760487fb5bcceebebcb2096db689eac4a0dee90fde5be04a",
        ),
        (
            &[9, 13, 18, 20],
            "789835d75f154603f44546b022c9a6ddebcc7aa9067413db052d262ae917643c",
        ),
        (
            &[11, 12, 14],
            "71cb7f8ebcb756f1088d7bf7b6bb6f01188e5ecb355456e8b096f41bc210deec",
        ),
        (
            &[17],
            "6399e5d55e7d771a738d5abc2218f8e01ec671cd152b92dc755abc15a94beb1b",
        ),
    ];
    let mut n = 0;
    for (flights, want) in vectors {
        for &f in flights {
            assert_eq!(hash_272(&dir, Episode::E2, f), want, "E2 flight {f}");
            n += 1;
        }
    }
    assert_eq!(n, 20);
}

#[test]
fn e3_direct_starts_match_the_spec_vectors() {
    // weapons §3.3.3 test vector (a): a scene card, or the restore screen and the title card.
    let data = require_data!();
    let dir = DataDir::open(&data).unwrap();
    let vectors: [(&[u8], &str); 6] = [
        (
            &[1],
            "5dc16d6e4ae602b24c59ce5ec1e5f6c95eb7b0c9503ac8298ec0f69803528fe3",
        ),
        (
            &[2],
            "9bb3a05836408cf061e0bc065274141e6f8adb06f0ca9ea6334b5bce53402ff3",
        ),
        (
            &[3, 5],
            "5a544596022aa9edc4db5e411c9c82e935da051c519ef0481cce99bca2aec9d3",
        ),
        (
            &[6, 8],
            "6483a00c2b7b4d0d4ed492b54a7ca13455911303bd41a365c1b828d8b853e43a",
        ),
        (
            &[12, 15, 19],
            "d1ec822edc15d2ed5a0b9dd20e37bf591c8466c0c3e561ab233ca480c3969d82",
        ),
        (
            &[4, 9, 10, 11, 13, 14, 16, 17, 18],
            "41a4abd362b580d5fe2a0d9116d87cb5866e47b28cc82f7ec28c76cbf4a41181",
        ),
    ];
    let mut n = 0;
    for (flights, want) in vectors {
        for &f in flights {
            assert_eq!(hash_272(&dir, Episode::E3, f), want, "E3 flight {f}");
            n += 1;
        }
    }
    assert_eq!(n, 18);
}

fn check_all(l: &full_flight::Layout, flights: impl IntoIterator<Item = u8>) {
    let (data, oracle) = (require_data!(), require_oracle!());
    let compared = flights
        .into_iter()
        .filter(|&f| full_flight::check_world(l, &data, &oracle, f))
        .count();
    eprintln!("{:?}: {compared} captures compared", l.episode);
}

#[test]
fn e1_direct_start_worlds_match_every_capture() {
    check_all(&full_flight::E1, 1..=20);
}

#[test]
fn e2_direct_start_worlds_match_every_capture() {
    check_all(&full_flight::E2, 1..=20);
}

#[test]
fn e3_direct_start_worlds_match_every_capture() {
    check_all(&full_flight::E3, (1..=19).filter(|&f| f != 7));
}
