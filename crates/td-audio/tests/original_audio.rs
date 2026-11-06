//! L1 tests: decode every original sound effect and render every song (data-gated).

use std::fs;
use std::sync::Arc;

use td_audio::{CmfPlayer, decode_voc};
use td_formats::cmf::Cmf;
use td_test_support::require_data;

/// voc-and-adpcm rev 3 §2.2 (rates with the DSP clamp) and §4.2 (SHA-256 of the decoded 8-bit
/// output, first 16 hex digits; sample counts 2, 3 or 4 × (D − 1) for ADPCM, D for PCM).
#[test]
fn every_sound_effect_decodes_as_the_spec_says() {
    let dir = require_data!();
    for (name, rate, samples, sha) in [
        ("afterb.voc", 6024, 8_316, "0ea3db570d6efe55"),
        ("boomboom.voc", 6250, 10_992, "78a7a1289d7dfeca"),
        ("home.voc", 8000, 32_230, "7c39168b2243ac75"),
        ("hornet.voc", 4000, 4_409, "c1ef92309ab534fd"),
        ("income.voc", 8000, 11_144, "938defcb857d54ad"),
        ("missile.voc", 6410, 7_526, "b3e38d9dd542843a"),
        ("poink.voc", 4000, 4_409, "6b1eda6d402c8894"),
        ("pulse.voc", 4000, 897, "a1ded05990d84da2"),
        ("punyexp.voc", 6993, 3_423, "634b47fd5df7ce0e"),
        ("ric2.voc", 8000, 23_941, "8d63c7d6bad0d34b"),
        // tc 173 is above the 4-bit limit 172: 11,904 Hz, not the header's 12,048 (U-402).
        ("ric3.voc", 11904, 33_526, "2916617f4cccdceb"),
        ("scratch.voc", 8000, 1_188, "b9f30973a47c2f81"),
        ("stiletto.voc", 8000, 2_880, "7fe078fe09967221"),
        ("vii.voc", 8000, 2_918, "c13a7450190bbc9c"),
        ("vulturei.voc", 6024, 1_320, "f2ce2a7b7c6a7b6c"),
        ("whoosh.voc", 4000, 17_689, "dfe0401a58dd882f"),
        ("zeus.voc", 8000, 5_478, "2ff1ac4b975e2488"),
    ] {
        let pcm = decode_voc(&fs::read(dir.join(name)).unwrap())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(pcm.rate, rate, "{name}");
        assert_eq!(pcm.samples.len(), samples, "{name}");
        let per_byte = usize::from(pcm.dma.codes_per_byte);
        let refs = usize::from(per_byte > 1);
        assert_eq!(
            per_byte * (pcm.dma.bytes as usize - refs),
            samples,
            "{name}"
        );
        let hash = td_test_support::sha256_hex(&pcm.samples);
        assert_eq!(&hash[..16], sha, "{name}: {hash}");
    }
}

#[test]
fn every_song_renders_audible_output() {
    let dir = require_data!();
    for name in [
        "climax.cmf",
        "death.cmf",
        "hangar.cmf",
        "intro.cmf",
        "intro1.cmf",
        "intro2.cmf",
        "low.cmf",
        "menu.cmf",
        "mood.cmf",
        "mood2.cmf",
        "mood3.cmf",
        "moodmaj.cmf",
        "satair.cmf",
        "td01.cmf",
        "td02.cmf",
        "td03.cmf",
        "vulture.cmf",
    ] {
        let song = Arc::new(Cmf::parse(&fs::read(dir.join(name)).unwrap()).unwrap());
        let mut p = CmfPlayer::new(song, false);
        let mut buf = vec![0i32; p.sample_rate() as usize * 5]; // first 5 seconds
        p.render(&mut buf);
        let peak = buf.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!(peak > 500, "{name}: peak {peak}");
    }
}
