//! golem: paste detection for Windows console terminals.
//!
//! Crossterm reads Windows console input records, which carry no bracketed
//! paste markers: ConPTY turns a paste into ordinary key presses, `\r\n`
//! into Enter + ctrl+Enter. Each Enter would then send the half-pasted
//! prompt. The input thread reads key presses that arrive without a pause
//! as one burst, and [`coalesce`] turns a burst of plain text keys into a
//! single [`Event::Paste`], like a bracketed paste elsewhere.

#![cfg_attr(not(windows), allow(dead_code))]

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Duration;

/// Longest pause between two key events of one paste. ConPTY delivers a
/// paste in chunks; pauses up to ~8 ms were seen between them. Typed keys
/// are much further apart.
pub const GAP: Duration = Duration::from_millis(15);

/// Upper bound of events read into one burst.
pub const MAX_EVENTS: usize = 1 << 20;

/// The text a key press stands for inside a paste; `None` when the key is
/// not plain text.
pub fn paste_char(key: &KeyEvent) -> Option<char> {
    let mods = key.modifiers;
    match key.code {
        // AltGr arrives as ctrl+alt on Windows.
        KeyCode::Char(c)
            if mods.difference(KeyModifiers::SHIFT).is_empty()
                || mods.contains(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            Some(c)
        }
        KeyCode::Enter if mods.is_empty() => Some('\r'),
        // The `\n` of `\r\n` (byte 0x0a) arrives as ctrl+Enter / ctrl+j.
        KeyCode::Enter if mods == KeyModifiers::CONTROL => Some('\n'),
        KeyCode::Char('j') if mods == KeyModifiers::CONTROL => Some('\n'),
        KeyCode::Tab if mods.is_empty() => Some('\t'),
        _ => None,
    }
}

fn is_press(key: &KeyEvent) -> bool {
    key.kind != KeyEventKind::Release
}

/// Whether `event` may start a paste burst (a plain text key press).
pub fn starts_burst(event: &Event) -> bool {
    matches!(event, Event::Key(key) if is_press(key) && paste_char(key).is_some())
}

/// Turn a burst of events read without pauses into what the app should
/// see: one `Paste` with the text keys (other events such as mouse moves
/// follow it), or the events unchanged when the burst is not a paste.
///
/// A burst is a paste when it holds only text key presses, at least three,
/// or two that do not end in a plain Enter. Two keys typed in a quick
/// roll stay keys, so a fast "x⏎" still sends.
pub fn coalesce(events: Vec<Event>) -> Vec<Event> {
    let mut text = String::new();
    let mut presses = 0usize;
    let mut last_enter = false;
    let mut others = Vec::new();
    for event in &events {
        match event {
            Event::Key(key) if !is_press(key) => {}
            Event::Key(key) => match paste_char(key) {
                Some(ch) => {
                    text.push(ch);
                    presses += 1;
                    last_enter = ch == '\r';
                }
                None => return events,
            },
            other => others.push(other.clone()),
        }
    }
    if presses >= 3 || (presses == 2 && !last_enter) {
        let mut out = Vec::with_capacity(others.len() + 1);
        out.push(Event::Paste(text));
        out.extend(others);
        out
    } else {
        events
    }
}

#[cfg(test)]
#[path = "../../tests/unit/input__paste_burst__tests.rs"]
mod tests;
