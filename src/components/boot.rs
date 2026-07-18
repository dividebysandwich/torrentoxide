//! A brief "system boot" overlay shown on load before the UI takes over.

use std::time::Duration;

use leptos::prelude::*;

const LINES: &[&str] = &[
    "> INITIALIZING NETRUNNER INTERFACE",
    "> LOADING KERNEL MODULES ........... OK",
    "> MOUNTING TORRENT ENGINE .......... OK",
    "> NEGOTIATING DHT UPLINK ........... OK",
    "> DECRYPTING SESSION VAULT ......... OK",
    "> TORRENTOXIDE // ONLINE",
];

#[component]
pub fn BootSequence() -> impl IntoView {
    let visible = RwSignal::new(true);
    let shown = RwSignal::new(0usize);

    Effect::new(move |ran: Option<bool>| {
        if ran == Some(true) {
            return true;
        }
        let total = LINES.len();
        let handle = set_interval_with_handle(
            move || shown.update(|s| *s = (*s + 1).min(total)),
            Duration::from_millis(170),
        )
        .ok();
        let dur = total as u64 * 170;
        set_timeout(
            move || {
                if let Some(h) = &handle {
                    h.clear();
                }
            },
            Duration::from_millis(dur + 150),
        );
        set_timeout(move || visible.set(false), Duration::from_millis(dur + 800));
        true
    });

    let visible_lines = move || (0..shown.get()).collect::<Vec<usize>>();

    view! {
        {move || {
            visible.get().then(|| {
                view! {
                    <div class="boot-overlay">
                        <div class="boot-box">
                            <div class="boot-title">"TORRENTOXIDE"</div>
                            <For
                                each=visible_lines
                                key=|i| *i
                                children=move |i| view! { <div class="boot-line">{LINES[i]}</div> }
                            />
                            <div class="boot-cursor">"▮"</div>
                        </div>
                    </div>
                }
            })
        }}
    }
}
