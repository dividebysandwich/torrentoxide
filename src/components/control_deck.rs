//! A slim HUD "control deck" for live, persisted global settings.
//! Currently: global download/upload rate limits (0 = unlimited).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{get_settings, set_settings};
use crate::components::dashboard_state;
use crate::types::{fmt_bytes, Settings};

#[component]
pub fn ControlDeck() -> impl IntoView {
    let state = dashboard_state();
    let settings = RwSignal::new(Settings::default());
    let loaded = RwSignal::new(false);

    // Free-disk gauge (from the live snapshot).
    let disk_free = move || state.snapshot.get().disk_free;
    let disk_total = move || state.snapshot.get().disk_total;
    let have_disk = move || disk_total() > 0;
    let used_pct = move || {
        let t = disk_total();
        if t > 0 {
            ((t - disk_free().min(t)) as f64 / t as f64 * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        }
    };
    let disk_low = move || {
        let t = disk_total();
        t > 0 && (disk_free() as f64 / t as f64) < 0.05
    };
    let disk_text = move || {
        if have_disk() {
            format!("{} FREE", fmt_bytes(disk_free() as f64))
        } else {
            "— ".to_string()
        }
    };

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
    let on_ratio_toggle = move |e| {
        let on = event_target_checked(&e);
        settings.update(|s| s.ratio_enabled = on);
        save();
    };
    let on_ratio = move |e| {
        let v = event_target_value(&e).trim().parse::<f32>().unwrap_or(0.0).max(0.0);
        settings.update(|s| s.ratio_limit = v);
        save();
    };
    let ratio_on = move || settings.get().ratio_enabled;
    let ratio_val = move || format!("{:.2}", settings.get().ratio_limit);

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
            <span class="deck-sep"></span>
            <label class="deck-field deck-seed" class:on=ratio_on>
                <input
                    r#type="checkbox"
                    class="deck-toggle"
                    prop:checked=ratio_on
                    on:change=on_ratio_toggle
                />
                <span class="deck-label seed">"♻ SEED ≤"</span>
                <input
                    class="deck-input narrow"
                    r#type="number"
                    min="0"
                    step="0.1"
                    prop:value=ratio_val
                    prop:disabled=move || !ratio_on()
                    on:change=on_ratio
                />
                <span class="deck-unit">"RATIO"</span>
            </label>
            <span class="deck-hint">
                {move || if loaded.get() { "0 = UNLIMITED" } else { "SYNC…" }}
            </span>
            <div class="deck-disk" class:low=disk_low>
                <span class="deck-label">"◆ DISK"</span>
                <div class="disk-bar">
                    <span
                        class="disk-fill"
                        class:low=disk_low
                        style=move || format!("width:{:.1}%", used_pct())
                    ></span>
                </div>
                <span class="disk-text">{disk_text}</span>
            </div>
        </div>
    }
}
