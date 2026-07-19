//! Small client-side effect helpers: a fast PRNG, a stable hash for seeded
//! shuffles, and a WebAudio "chirp" for button feedback.

use std::cell::Cell;

thread_local! {
    static RNG: Cell<u64> = const { Cell::new(0x2545_F491_4F6C_DD1D) };
}

/// Fast xorshift PRNG for visual effects (deterministic, not cryptographic).
pub fn rand_u64() -> u64 {
    RNG.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        x
    })
}

/// A pseudo-random element from a `&str` charset (by char).
pub fn rand_char(charset: &str) -> char {
    let chars: &[u8] = charset.as_bytes();
    let i = (rand_u64() as usize) % chars.len();
    chars[i] as char
}

pub fn rand_index(n: usize) -> usize {
    if n == 0 {
        0
    } else {
        (rand_u64() % n as u64) as usize
    }
}

/// Deterministic 64-bit mix — used to seed stable per-item shuffles.
pub fn hash2(a: u64, b: u64) -> u64 {
    let mut x = a
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ b.wrapping_add(0x632B_E59B_D9B4_E019);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    x
}

/// Current local wall-clock time as `HH:MM:SS`.
#[cfg(feature = "hydrate")]
pub fn now_hms() -> String {
    let d = js_sys::Date::new_0();
    format!(
        "{:02}:{:02}:{:02}",
        d.get_hours() as u32,
        d.get_minutes() as u32,
        d.get_seconds() as u32
    )
}

#[cfg(not(feature = "hydrate"))]
pub fn now_hms() -> String {
    "00:00:00".to_string()
}

/// Today's date as `(year, month 1-12, day)` — browser clock.
#[cfg(feature = "hydrate")]
pub fn today_ymd() -> (i32, u32, u32) {
    let d = js_sys::Date::new_0();
    (
        d.get_full_year() as i32,
        d.get_month() as u32 + 1,
        d.get_date() as u32,
    )
}

#[cfg(not(feature = "hydrate"))]
pub fn today_ymd() -> (i32, u32, u32) {
    (1970, 1, 1)
}

/// Play a short square-wave "chirp" for UI feedback (browser only).
#[cfg(feature = "hydrate")]
pub fn chirp(freq: f64) {
    use std::cell::RefCell;
    thread_local! {
        static CTX: RefCell<Option<web_sys::AudioContext>> = const { RefCell::new(None) };
    }
    CTX.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            *cell = web_sys::AudioContext::new().ok();
        }
        let Some(ctx) = cell.as_ref() else { return };
        let (Ok(osc), Ok(gain)) = (ctx.create_oscillator(), ctx.create_gain()) else {
            return;
        };
        osc.set_type(web_sys::OscillatorType::Square);
        osc.frequency().set_value(freq as f32);
        let t = ctx.current_time();
        gain.gain().set_value(0.0);
        let _ = gain.gain().set_value_at_time(0.05, t);
        let _ = gain.gain().exponential_ramp_to_value_at_time(0.0001, t + 0.09);
        let _ = osc.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&ctx.destination());
        let _ = osc.start();
        let _ = osc.stop_with_when(t + 0.1);
    });
}

#[cfg(not(feature = "hydrate"))]
pub fn chirp(_freq: f64) {}
