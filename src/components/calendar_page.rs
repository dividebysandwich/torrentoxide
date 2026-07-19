//! Release calendar: a month grid of upcoming/recent episode air dates for the
//! monitored series on the Wanted list (via TMDb).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::get_calendar;
use crate::components::fx::today_ymd;
use crate::types::CalendarEntry;

/// Day of week, 0 = Sunday (Sakamoto's algorithm).
fn weekday(y: i32, m: u32, d: u32) -> u32 {
    let t: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    let w = y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d as i32;
    (((w % 7) + 7) % 7) as u32
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 30,
    }
}

fn month_name(m: u32) -> &'static str {
    [
        "", "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ]
    .get(m as usize)
    .copied()
    .unwrap_or("")
}

#[component]
pub fn CalendarPage() -> impl IntoView {
    let entries = RwSignal::new(Vec::<CalendarEntry>::new());
    let status = RwSignal::new(String::new());
    let (ty, tm, td) = today_ymd();
    let year = RwSignal::new(ty);
    let month = RwSignal::new(tm);

    Effect::new(move |_| {
        status.set("Loading…".into());
        spawn_local(async move {
            match get_calendar().await {
                Ok(e) => {
                    status.set(if e.is_empty() {
                        "No dates — add monitored series to the Wanted list.".into()
                    } else {
                        String::new()
                    });
                    entries.set(e);
                }
                Err(e) => status.set(e.to_string()),
            }
        });
    });

    let prev = move |_| {
        month.update(|m| {
            if *m == 1 {
                *m = 12;
                year.update(|y| *y -= 1);
            } else {
                *m -= 1;
            }
        })
    };
    let next = move |_| {
        month.update(|m| {
            if *m == 12 {
                *m = 1;
                year.update(|y| *y += 1);
            } else {
                *m += 1;
            }
        })
    };

    let weekdays = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

    view! {
        <div class="settings-page">
            <section class="panel settings-section">
                <div class="files-head">
                    <h2 class="page-title">"CALENDAR"</h2>
                    <div class="cal-nav">
                        <button class="btn btn-ghost btn-sm" on:click=prev>"◀"</button>
                        <span class="cal-month">
                            {move || format!("{} {}", month_name(month.get()), year.get())}
                        </span>
                        <button class="btn btn-ghost btn-sm" on:click=next>"▶"</button>
                    </div>
                </div>
                <p class="add-status">{move || status.get()}</p>
                <div class="cal-grid">
                    {weekdays.iter().map(|w| view! { <div class="cal-wd">{*w}</div> }).collect_view()}
                    {move || {
                        let (y, m) = (year.get(), month.get());
                        let es = entries.get();
                        let first = weekday(y, m, 1);
                        let dim = days_in_month(y, m);
                        let mut out: Vec<AnyView> = Vec::new();
                        for _ in 0..first {
                            out.push(view! { <div class="cal-cell empty"></div> }.into_any());
                        }
                        for d in 1..=dim {
                            let key = format!("{y:04}-{m:02}-{d:02}");
                            let is_today = y == ty && m == tm && d == td;
                            let chips = es
                                .iter()
                                .filter(|e| e.air_date == key)
                                .cloned()
                                .map(|e| {
                                    view! {
                                        <div
                                            class="cal-ep"
                                            title=format!("{} S{:02}E{:02} — {}", e.title, e.season, e.episode, e.name)
                                        >
                                            <span class="cal-ep-se">{format!("S{:02}E{:02}", e.season, e.episode)}</span>
                                            <span class="cal-ep-title">{e.title.clone()}</span>
                                        </div>
                                    }
                                })
                                .collect_view();
                            out.push(
                                view! {
                                    <div class="cal-cell" class:today=is_today>
                                        <span class="cal-day">{d}</span>
                                        {chips}
                                    </div>
                                }
                                .into_any(),
                            );
                        }
                        while out.len() % 7 != 0 {
                            out.push(view! { <div class="cal-cell empty"></div> }.into_any());
                        }
                        out
                    }}
                </div>
            </section>
        </div>
    }
}
