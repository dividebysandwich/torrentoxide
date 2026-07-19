//! Minimal RSS 2.0 / Torznab item parser (via quick-xml). Extracts each item's
//! title, download URL (magnet preferred, else enclosure, else link), size and
//! seeders — enough for both Jackett/Torznab results and plain torrent RSS feeds.
//! (Atom feeds are not handled.)

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use crate::types::Release;

#[derive(Default)]
struct Item {
    title: Option<String>,
    link: Option<String>,
    enclosure_url: Option<String>,
    magnet: Option<String>,
    size: Option<u64>,
    seeders: Option<u32>,
}

impl Item {
    fn into_release(self) -> Option<Release> {
        let title = self.title.filter(|t| !t.trim().is_empty())?;
        let url = self
            .magnet
            .or(self.enclosure_url)
            .or(self.link)
            .filter(|u| !u.trim().is_empty())?;
        Some(Release {
            title: title.trim().to_string(),
            url,
            size: self.size.unwrap_or(0),
            seeders: self.seeders,
            indexer: String::new(),
        })
    }
}

#[derive(Clone, Copy)]
enum Field {
    Title,
    Link,
    Size,
}

/// Strip any namespace prefix (`torznab:attr` → `attr`) and lower-case.
fn local_name(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    s.rsplit(':').next().unwrap_or(&s).to_ascii_lowercase()
}

pub fn parse_items(xml: &str) -> Vec<Release> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut cur: Option<Item> = None;
    let mut field: Option<Field> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref());
                if name == "item" {
                    cur = Some(Item::default());
                }
                field = match name.as_str() {
                    "title" => Some(Field::Title),
                    "link" => Some(Field::Link),
                    "size" => Some(Field::Size),
                    _ => None,
                };
            }
            Ok(Event::Empty(e)) => {
                if let Some(item) = cur.as_mut() {
                    match local_name(e.name().as_ref()).as_str() {
                        "enclosure" => read_enclosure(&e, item),
                        "attr" => read_torznab_attr(&e, item),
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if let (Some(item), Some(f)) = (cur.as_mut(), field) {
                    let txt = e
                        .xml_content(quick_xml::XmlVersion::Implicit1_0)
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    apply_text(item, f, txt);
                }
                field = None;
            }
            Ok(Event::CData(e)) => {
                if let (Some(item), Some(f)) = (cur.as_mut(), field) {
                    let bytes = e.into_inner();
                    let txt = String::from_utf8_lossy(&bytes).trim().to_string();
                    apply_text(item, f, txt);
                }
                field = None;
            }
            Ok(Event::End(e)) => {
                if local_name(e.name().as_ref()) == "item" {
                    if let Some(r) = cur.take().and_then(Item::into_release) {
                        out.push(r);
                    }
                }
                field = None;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

fn apply_text(item: &mut Item, f: Field, txt: String) {
    if txt.is_empty() {
        return;
    }
    match f {
        Field::Title => item.title = Some(txt),
        Field::Link => {
            if txt.starts_with("magnet:") {
                item.magnet.get_or_insert(txt);
            } else {
                item.link.get_or_insert(txt);
            }
        }
        Field::Size => item.size = item.size.or_else(|| txt.parse().ok()),
    }
}

fn read_enclosure(e: &BytesStart, item: &mut Item) {
    for a in e.attributes().flatten() {
        let key = local_name(a.key.as_ref());
        let val = a
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .unwrap_or_default()
            .to_string();
        match key.as_str() {
            "url" => {
                if val.starts_with("magnet:") {
                    item.magnet.get_or_insert(val);
                } else {
                    item.enclosure_url.get_or_insert(val);
                }
            }
            "length" => item.size = item.size.or_else(|| val.parse().ok()),
            _ => {}
        }
    }
}

fn read_torznab_attr(e: &BytesStart, item: &mut Item) {
    let mut name = String::new();
    let mut value = String::new();
    for a in e.attributes().flatten() {
        let key = local_name(a.key.as_ref());
        let val = a
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .unwrap_or_default()
            .to_string();
        match key.as_str() {
            "name" => name = val,
            "value" => value = val,
            _ => {}
        }
    }
    match name.as_str() {
        "seeders" => item.seeders = value.parse().ok(),
        "size" => item.size = item.size.or_else(|| value.parse().ok()),
        "magneturl" => {
            item.magnet.get_or_insert(value);
        }
        _ => {}
    }
}
