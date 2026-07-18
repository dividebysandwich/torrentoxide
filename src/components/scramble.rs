//! Text that "decrypts" from random glyphs (Decode) or rolls its digits (Roll)
//! whenever its value changes — a signature cyberpunk reveal.

use std::time::Duration;

use leptos::prelude::*;

use crate::components::fx::rand_char;

const GLYPHS: &str = "ABCDEF0123456789<>[]{}#%$&*+=/\\|!?~^";
const DIGITS: &str = "0123456789";

#[derive(Clone, Copy, PartialEq)]
pub enum ScrambleMode {
    /// Reveal left-to-right from random glyphs (for text such as torrent names).
    Decode,
    /// Scramble only the digit characters briefly (for numeric counters).
    Roll,
}

#[component]
pub fn Scramble(
    #[prop(into)] text: Signal<String>,
    #[prop(default = ScrambleMode::Decode)] mode: ScrambleMode,
) -> impl IntoView {
    // Shown text — starts resolved so SSR/first paint is correct.
    let display = RwSignal::new(text.get_untracked());
    let handle = StoredValue::new(None::<IntervalHandle>);
    let revealed = StoredValue::new(0usize);
    let target = StoredValue::new(Vec::<char>::new());

    Effect::new(move |prev: Option<String>| {
        let want = text.get();
        if prev.as_ref() == Some(&want) {
            return want;
        }
        // cancel any in-flight animation
        handle.update_value(|h| {
            if let Some(hh) = h.take() {
                hh.clear();
            }
        });

        let chars: Vec<char> = want.chars().collect();
        let len = chars.len();
        target.set_value(chars);
        revealed.set_value(0);

        let charset = match mode {
            ScrambleMode::Decode => GLYPHS,
            ScrambleMode::Roll => DIGITS,
        };
        let ticks = match mode {
            ScrambleMode::Decode => 22usize,
            // short enough to settle between 4 Hz (250 ms) updates
            ScrambleMode::Roll => 4usize,
        };
        let step = (len.max(1) as f32 / ticks as f32).ceil().max(1.0) as usize;
        let final_text = want.clone();

        let h = set_interval_with_handle(
            move || {
                let done = revealed.get_value();
                let out: String = target.with_value(|t| {
                    t.iter()
                        .enumerate()
                        .map(|(i, &c)| {
                            let scrambleable = match mode {
                                ScrambleMode::Decode => c != ' ',
                                ScrambleMode::Roll => c.is_ascii_digit(),
                            };
                            if i < done || !scrambleable {
                                c
                            } else {
                                rand_char(charset)
                            }
                        })
                        .collect()
                });
                display.set(out);
                revealed.update_value(|r| *r = (*r + step).min(len));
                if done >= len {
                    display.set(final_text.clone());
                    handle.update_value(|h| {
                        if let Some(hh) = h.take() {
                            hh.clear();
                        }
                    });
                }
            },
            Duration::from_millis(28),
        )
        .ok();
        handle.set_value(h);
        want
    });

    view! { <span class="scramble">{move || display.get()}</span> }
}
