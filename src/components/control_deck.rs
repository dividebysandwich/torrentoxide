//! A slim HUD "control deck" for live, persisted global settings.
//! Currently: global download/upload rate limits (0 = unlimited).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{get_settings, set_settings};
use crate::types::Settings;

#[component]
pub fn ControlDeck() -> impl IntoView {
    let settings = RwSignal::new(Settings::default());
    let loaded = RwSignal::new(false);

    // Load persisted settings once, client-side.
    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(s) = get_settings().await {
                settings.set(s);
                loaded.set(true);
            }
        });
    });

    // Persist current settings to the server; rate limits apply immediately.
    let save = move || {
        let s = settings.get();
        spawn_local(async move {
            let _ = set_settings(s).await;
        });
    };

    // A KiB/s field: empty / invalid parses back to 0 (unlimited).
    let on_down = move |e| {
        let v = event_target_value(&e).trim().parse::<u32>().unwrap_or(0);
        settings.update(|s| s.down_limit_kbps = v);
        save();
    };
    let on_up = move |e| {
        let v = event_target_value(&e).trim().parse::<u32>().unwrap_or(0);
        settings.update(|s| s.up_limit_kbps = v);
        save();
    };

    // Show a blank field (with an ∞ placeholder) when the limit is 0.
    let down_val = move || match settings.get().down_limit_kbps {
        0 => String::new(),
        v => v.to_string(),
    };
    let up_val = move || match settings.get().up_limit_kbps {
        0 => String::new(),
        v => v.to_string(),
    };

    view! {
        <div class="control-deck panel">
            <span class="deck-title">"⌁ THROTTLE"</span>
            <label class="deck-field">
                <span class="deck-label down">"▼ DOWN"</span>
                <input
                    class="deck-input"
                    r#type="number"
                    min="0"
                    placeholder="∞"
                    prop:value=down_val
                    on:change=on_down
                />
                <span class="deck-unit">"KiB/s"</span>
            </label>
            <label class="deck-field">
                <span class="deck-label up">"▲ UP"</span>
                <input
                    class="deck-input"
                    r#type="number"
                    min="0"
                    placeholder="∞"
                    prop:value=up_val
                    on:change=on_up
                />
                <span class="deck-unit">"KiB/s"</span>
            </label>
            <span class="deck-hint">
                {move || if loaded.get() { "0 = UNLIMITED" } else { "SYNC…" }}
            </span>
        </div>
    }
}
