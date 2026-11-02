//! L1 tests: decode every original sound effect and render every song (data-gated).

use std::fs;
use std::sync::Arc;

use td_audio::{CmfPlayer, decode_voc};
use td_formats::cmf::Cmf;
use td_test_support::require_data;

#[test]
fn every_sound_effect_decodes_to_the_expected_length() {
    let dir = require_data!();
    // (file, rate, samples): samples = 1 + codes_per_byte * (data bytes - 1) for ADPCM.
    for (name, rate) in [
        ("afterb.voc", 6024),
        ("boomboom.voc", 6250),
        ("home.voc", 8000),
        ("hornet.voc", 4000),
        ("income.voc", 8000),
        ("missile.voc", 6410),
        ("poink.voc", 4000),
        ("pulse.voc", 4000),
        ("punyexp.voc", 6993),
        ("ric2.voc", 8000),
        ("ric3.voc", 12048),
        ("scratch.voc", 8000),
        ("stiletto.voc", 8000),
        ("vii.voc", 8000),
        ("vulturei.voc", 6024),
        ("whoosh.voc", 4000),
        ("zeus.voc", 8000),
    ] {
        let pcm = decode_voc(&fs::read(dir.join(name)).unwrap())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(pcm.rate, rate, "{name}");
        assert!(
            pcm.seconds() > 0.05 && pcm.seconds() < 10.0,
            "{name}: {}",
            pcm.seconds()
        );
        // Decoded audio must not be pinned at the rails (a sign of a broken decoder).
        let clipped = pcm.samples.iter().filter(|&&s| s == 0 || s == 255).count();
        assert!(
            clipped * 20 < pcm.samples.len(),
            "{name}: {clipped} clipped samples"
        );
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
