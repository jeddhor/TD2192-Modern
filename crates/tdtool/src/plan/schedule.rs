//! Plans as replays and as tick-keyed oracle scenario lines.
//!
//! **Tick rule** (ORACLE.md §5a, `docs/parity/M9-playthrough.md`): the oracle polls the game's
//! tick counter `g_MissionTicks` (DS:1718) every emulated millisecond and injects the key
//! transitions of a `tick:T` line within 1 ms after the counter becomes T, that is during
//! steps 1–4 of tick T, long before the input steps 23–33 that read the keys. So the input of
//! port tick T (`TickControls` of replay word T − 1) is: keys down = the keys down after the
//! transitions of `tick:T`; keys made = the keys that go down at `tick:T`.
//!
//! **Comm viewer.** While the transmission screen is open the counter stays at the tick T that
//! opened it. The schedule then presses Space and F6 alternately every 15 frames with
//! `intick:T+N` lines, which the oracle skips once the counter has moved on: Space answers the
//! prompts, the first F6 after the last prompt closes the screen, and no key reaches a flight
//! tick (replay semantics: [`td_sim::replay::step`] resumes with no input).

use std::path::Path;

use anyhow::{Context, Result, bail};
use td_sim::flight::TickResult;
use td_sim::replay::Replay;
use td_sim::{FlightKey, TickControls, state_hash};

use crate::flight_setup::{FlightSetup, episode_number};

/// A plan: one input word per tick.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// `TickControls` words, tick 1 first.
    pub inputs: Vec<u64>,
    /// Ticks in which the comm viewer opened (1-based), found by running the plan.
    pub comm_ticks: Vec<u32>,
}

/// The oracle key name of a flight key (ORACLE.md §5 key names).
pub fn key_name(k: FlightKey) -> &'static str {
    match k {
        FlightKey::Kp8 => "kp8",
        FlightKey::Up => "up",
        FlightKey::Kp2 => "kp2",
        FlightKey::Down => "down",
        FlightKey::Kp4 => "kp4",
        FlightKey::Left => "left",
        FlightKey::Kp6 => "kp6",
        FlightKey::Right => "right",
        FlightKey::KpPlus => "kpplus",
        FlightKey::Kp0 => "kp0",
        FlightKey::Alt => "lalt",
        FlightKey::RightAlt => "ralt",
        FlightKey::Space => "space",
        FlightKey::Esc => "esc",
        FlightKey::P => "p",
        FlightKey::F5 => "f5",
        FlightKey::F6 => "f6",
        FlightKey::S => "s",
        FlightKey::Tab => "tab",
        FlightKey::M => "m",
    }
}

/// Keys the game clears by itself (afterburner, missile Space, Tab, Alt, F6 in the comm viewer).
/// A held key repeats (typematic, 500 ms, then every 33 ms) while it is the last one pressed,
/// and such a repeat sets a cleared byte again, which the replay cannot express: these keys
/// may be held for at most [`MAX_CLEARABLE_HOLD`] ticks.
pub const CLEARABLE: [FlightKey; 6] = [
    FlightKey::KpPlus,
    FlightKey::Kp0,
    FlightKey::Tab,
    FlightKey::Alt,
    FlightKey::RightAlt,
    FlightKey::F6,
];
/// Longest hold of a [`CLEARABLE`] key, in ticks (about 260 ms, below the 500 ms delay).
pub const MAX_CLEARABLE_HOLD: u32 = 5;

/// Checks that a plan can be keyed to the oracle: a key is made exactly in the tick it goes
/// down, no joystick, clearable keys held briefly. Returns the first problem.
pub fn check(inputs: &[u64]) -> Result<()> {
    let mut prev = TickControls::default();
    let mut held = [0u32; FlightKey::ALL.len()];
    for (i, &w) in inputs.iter().enumerate() {
        let c = TickControls::from_word(w);
        if c.joystick() != td_sim::Joystick::default() {
            bail!("tick {}: joystick input", i + 1);
        }
        for (k, &key) in FlightKey::ALL.iter().enumerate() {
            let pressed = c.is_down(key) && !prev.is_down(key);
            let made = TickControls::from_word(w).to_word() >> 32 & (1 << k) != 0;
            if made != pressed {
                bail!("tick {}: {key:?} made {made} but pressed {pressed}", i + 1);
            }
            held[k] = if c.is_down(key) { held[k] + 1 } else { 0 };
            if CLEARABLE.contains(&key) && held[k] > MAX_CLEARABLE_HOLD {
                bail!(
                    "tick {}: {key:?} held longer than {MAX_CLEARABLE_HOLD} ticks",
                    i + 1
                );
            }
        }
        prev = c;
    }
    Ok(())
}

/// The input word of tick `t` for keys `down`, given the previous tick's keys.
pub fn word(prev: &[FlightKey], down: &[FlightKey]) -> u64 {
    let mut c = TickControls::default();
    for &k in down {
        c.set_down(k);
        if !prev.contains(&k) {
            c.set_made(k);
        }
    }
    c.to_word()
}

/// The replay of a plan: its episode and flight, the session seed at program start,
/// checkpoints every [`super::CHECKPOINT_INTERVAL`] ticks.
pub fn replay(setup: &FlightSetup, plan: &Plan) -> Replay {
    let mut f = setup.start();
    let mut checkpoints = vec![];
    for (i, &w) in plan.inputs.iter().enumerate() {
        td_sim::replay::step(&mut f, &setup.world, w);
        if (i as u32 + 1).is_multiple_of(super::CHECKPOINT_INTERVAL) {
            checkpoints.push(state_hash(&f));
        }
    }
    Replay {
        episode: episode_number(setup.episode),
        mission: setup.number,
        seed: setup.session().rng.seed(),
        inputs: plan.inputs.clone(),
        interval: super::CHECKPOINT_INTERVAL,
        checkpoints,
    }
}

/// Ticks whose comm viewer opened, found by running the inputs.
pub fn comm_ticks(setup: &FlightSetup, inputs: &[u64]) -> Vec<u32> {
    let mut f = setup.start();
    let mut out = vec![];
    for (i, &w) in inputs.iter().enumerate() {
        let c = TickControls::from_word(w);
        let mut r = f.tick(&setup.world, &c, &mut vec![]);
        while let TickResult::Modal(m) = r {
            if m == td_sim::Modal::CommViewer {
                out.push(i as u32 + 1);
            }
            r = f.resume(&setup.world, &TickControls::default(), &mut vec![]);
        }
    }
    out
}

/// Frames between the alternating Space and F6 presses while a comm viewer is open.
pub const COMM_STEP: u64 = 15;
/// Presses scheduled per comm viewer (Space and F6 alternate): about 57 s of screen time.
pub const COMM_PRESSES: u64 = 260;

/// The oracle scenario lines of a plan (key transitions per tick, comm viewer presses).
pub fn oracle_lines(inputs: &[u64], comm: &[u32]) -> Vec<String> {
    let mut out = vec![];
    let mut prev = TickControls::default();
    for (i, &w) in inputs.iter().enumerate() {
        let t = i + 1;
        let c = TickControls::from_word(w);
        for &k in &FlightKey::ALL {
            if prev.is_down(k) && !c.is_down(k) {
                out.push(format!("tick:{t} up {}", key_name(k)));
            }
        }
        for &k in &FlightKey::ALL {
            if c.is_down(k) && !prev.is_down(k) {
                out.push(format!("tick:{t} down {}", key_name(k)));
            }
        }
        if comm.contains(&(t as u32)) {
            for n in 0..COMM_PRESSES {
                let key = if n % 2 == 0 { "space" } else { "f6" };
                out.push(format!("intick:{t}+{} press {key} 3", 20 + n * COMM_STEP));
            }
        }
        prev = c;
    }
    // Release whatever is still down after the last tick.
    for &k in &FlightKey::ALL {
        if prev.is_down(k) {
            out.push(format!("tick:{} up {}", inputs.len() + 1, key_name(k)));
        }
    }
    out
}

/// Writes `<out>/e<E>f<NN><suffix>.tdr`.
pub fn write_outputs(setup: &FlightSetup, plan: &Plan, out: &Path, suffix: &str) -> Result<()> {
    check(&plan.inputs)?;
    std::fs::create_dir_all(out).with_context(|| format!("create {}", out.display()))?;
    let path = out.join(format!("{}{suffix}.tdr", setup.stem()));
    std::fs::write(&path, replay(setup, plan).to_bytes())
        .with_context(|| format!("write {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}
