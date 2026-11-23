use super::*;

fn key(code: KeyCode, mods: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(code, mods))
}

fn release(code: KeyCode) -> Event {
    let mut k = KeyEvent::new(code, KeyModifiers::NONE);
    k.kind = KeyEventKind::Release;
    Event::Key(k)
}

/// Key events as ConPTY sends a paste: a press and a release per key,
/// `\r\n` as Enter + ctrl+Enter.
fn conpty(text: &str) -> Vec<Event> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let (code, mods) = match c {
            '\r' => (KeyCode::Enter, KeyModifiers::NONE),
            '\n' => (KeyCode::Enter, KeyModifiers::CONTROL),
            '\t' => (KeyCode::Tab, KeyModifiers::NONE),
            c if c.is_ascii_uppercase() => (KeyCode::Char(c), KeyModifiers::SHIFT),
            c => (KeyCode::Char(c), KeyModifiers::NONE),
        };
        out.push(key(code, mods));
        out.push(release(code));
    }
    out
}

#[test]
fn multiline_paste_becomes_one_paste_event() {
    let events = coalesce(conpty("line One\r\n\tline two\r\nend"));
    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Paste(text) => assert_eq!(text, "line One\r\n\tline two\r\nend"),
        other => panic!("expected a paste, got {other:?}"),
    }
}

#[test]
fn trailing_newline_stays_in_the_paste() {
    match &coalesce(conpty("ls\r\n"))[0] {
        Event::Paste(text) => assert_eq!(text, "ls\r\n"),
        other => panic!("expected a paste, got {other:?}"),
    }
}

#[test]
fn a_single_key_and_its_release_stay_keys() {
    let events = conpty("a");
    assert_eq!(coalesce(events.clone()), events);
    let enter = conpty("\r");
    assert_eq!(coalesce(enter.clone()), enter);
}

#[test]
fn a_quick_char_then_enter_still_sends() {
    let events = conpty("x\r");
    assert_eq!(coalesce(events.clone()), events);
}

#[test]
fn two_quick_chars_paste_the_same_text() {
    match &coalesce(conpty("ab"))[0] {
        Event::Paste(text) => assert_eq!(text, "ab"),
        other => panic!("expected a paste, got {other:?}"),
    }
}

#[test]
fn a_non_text_key_keeps_the_burst_as_keys() {
    let mut events = conpty("abc");
    events.push(key(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(coalesce(events.clone()), events);
    let mut ctrl = conpty("abc");
    ctrl.push(key(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert_eq!(coalesce(ctrl.clone()), ctrl);
}

#[test]
fn other_events_follow_the_paste() {
    let mut events = conpty("ab");
    events.insert(1, Event::Resize(80, 24));
    events.extend(conpty("c"));
    assert_eq!(
        coalesce(events),
        vec![Event::Paste("abc".into()), Event::Resize(80, 24)]
    );
}

#[test]
fn altgr_characters_count_as_text() {
    let events = vec![
        key(KeyCode::Char('ą'), KeyModifiers::CONTROL | KeyModifiers::ALT),
        key(KeyCode::Char('ę'), KeyModifiers::CONTROL | KeyModifiers::ALT),
        key(KeyCode::Char('x'), KeyModifiers::NONE),
    ];
    assert_eq!(coalesce(events), vec![Event::Paste("ąęx".into())]);
}

#[test]
fn only_text_key_presses_start_a_burst() {
    assert!(starts_burst(&key(KeyCode::Char('a'), KeyModifiers::NONE)));
    assert!(starts_burst(&key(KeyCode::Enter, KeyModifiers::NONE)));
    assert!(!starts_burst(&key(KeyCode::Char('v'), KeyModifiers::CONTROL)));
    assert!(!starts_burst(&key(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(!starts_burst(&release(KeyCode::Char('a'))));
    assert!(!starts_burst(&Event::Resize(1, 1)));
}
