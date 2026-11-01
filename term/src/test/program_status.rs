//! Exercise OSC 7501 through the real terminal input and lifecycle entry points.

use super::*;
use crate::program_status::ProgramStatusRecord;
use k9::assert_equal as assert_eq;
use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

fn report(term: &mut TestTerm, body: &str) {
    term.print(format!("\x1b]7501;{}\x1b\\", body));
}

fn record(term: &TestTerm, id: &str) -> ProgramStatusRecord {
    term.get_program_status_records()
        .into_iter()
        .find(|record| record.id == id)
        .unwrap_or_else(|| panic!("missing program status record {:?}", id))
}

fn ids(term: &TestTerm) -> Vec<String> {
    term.get_program_status_records()
        .into_iter()
        .map(|record| record.id)
        .collect()
}

#[test]
fn program_status_fragmented_reports_accept_bel_and_st() {
    for terminator in ["\x07", "\x1b\\"] {
        let mut term = TestTerm::new(3, 80, 0);
        let sequence = format!(
            "\x1b]7501;state=blocked:id=build/test:kind=question:progress=42:\
             app=cargo:title=VGVzdHM=:msg=Q29uZmlybT8={}",
            terminator
        );
        // Split every byte, including ESC and the final ST backslash. An
        // otherwise valid incomplete OSC must not expose a partial record.
        let bytes = sequence.as_bytes();
        for byte in &bytes[..bytes.len() - 1] {
            term.print([*byte]);
            assert!(term.get_program_status_records().is_empty());
        }
        term.print([bytes[bytes.len() - 1]]);
        let actual = record(&term, "build/test");
        assert_eq!(actual.state, "blocked");
        assert_eq!(actual.kind.as_deref(), Some("question"));
        assert_eq!(actual.progress, Some(42));
        assert_eq!(actual.app.as_deref(), Some("cargo"));
        assert_eq!(actual.effective_app.as_deref(), Some("cargo"));
        assert_eq!(actual.title.as_deref(), Some("Tests"));
        assert_eq!(actual.msg.as_deref(), Some("Confirm?"));
    }
}

#[test]
fn program_status_invalid_reports_do_not_partially_replace_records() {
    let mut term = TestTerm::new(3, 80, 0);
    report(&mut term, "state=done:app=trusted:msg=UmVhZHk=");
    let original = term.get_program_status_records();
    let invalid = [
        "app=other".to_owned(),
        "state=future:app=other".to_owned(),
        "state=working:id=:app=other".to_owned(),
        "state=working:id=build//test:app=other".to_owned(),
        "state=working:app=other:msg=YQ=".to_owned(),
        "state=working:app=other:msg=Cg==".to_owned(),
        "state=working:app=other:msg=/w==".to_owned(),
        format!("state=working:app={}", "a".repeat(33)),
        format!("state=working:{}=ignored", "a".repeat(17)),
        format!("state=working:id={}", "a".repeat(33)),
        format!("state=working:title={}", "a".repeat(257)),
    ];
    // Each rejected sequence contains enough valid data to corrupt the root
    // if dispatch mutates stored fields before validating the whole report.
    for body in invalid {
        report(&mut term, &body);
        assert_eq!(term.get_program_status_records(), original);
    }
    // A refused OSC must not poison parsing of the next valid report.
    report(&mut term, "state=idle:app=recovered");
    assert_eq!(record(&term, "").app.as_deref(), Some("recovered"));
}

#[test]
fn program_status_malformed_wire_ids_cannot_clear_valid_records() {
    for byte in [0x9d, 0xa0, 0xff] {
        for terminator in [b"\x07".as_slice(), b"\x1b\\"] {
            let mut term = TestTerm::new(3, 80, 0);
            report(&mut term, "state=done:id=badid:app=cargo");
            let original = term.get_program_status_records();
            let mut wire = b"\x1b]7501;state=clear:id=bad".to_vec();
            wire.push(byte);
            wire.extend_from_slice(b"id");
            wire.extend_from_slice(terminator);
            term.print(wire);
            assert_eq!(term.get_program_status_records(), original);
            report(&mut term, "state=idle:id=recovered");
            assert_eq!(record(&term, "recovered").state, "idle");
        }
    }
}

#[test]
fn program_status_incomplete_utf8_cannot_swallow_id_separator() {
    for prefix in [b"\xc2".as_slice(), b"\xe0", b"\xf0\xa0"] {
        for terminator in [b"\x07".as_slice(), b"\x1b\\"] {
            let mut term = TestTerm::new(3, 80, 0);
            report(&mut term, "state=done:app=keep-root");
            report(&mut term, "state=done:id=target");
            let mut wire = b"\x1b]7501;state=clear:future=x".to_vec();
            wire.extend_from_slice(prefix);
            wire.extend_from_slice(b":id=target");
            wire.extend_from_slice(terminator);
            term.print(wire);
            assert_eq!(ids(&term), vec![""]);
            assert_eq!(record(&term, "").app.as_deref(), Some("keep-root"));
        }
    }
}

#[test]
fn program_status_late_replacement_notifies_without_clearing_finished_children() {
    let mut term = TestTerm::new(3, 80, 0);
    report(&mut term, "state=done:id=build:app=cargo");
    report(&mut term, "state=done:id=build/tests");
    let changes = Arc::new(AtomicUsize::new(0));
    term.set_notification_handler(Box::new(CountChanges(Arc::clone(&changes))));
    term.program_status_process_exited();
    assert_eq!(changes.load(Ordering::SeqCst), 0);
    report(&mut term, "state=working:id=build");
    assert_eq!(ids(&term), vec!["build/tests"]);
    assert_eq!(record(&term, "build/tests").effective_app, None);
    assert_eq!(changes.load(Ordering::SeqCst), 1);
    report(&mut term, "state=working:id=build");
    assert_eq!(changes.load(Ordering::SeqCst), 1);
}

#[test]
fn program_status_replacement_removes_omitted_fields_and_updates_inheritance() {
    let mut term = TestTerm::new(3, 80, 0);
    report(
        &mut term,
        "state=working:app=cargo:progress=12:title=QnVpbGQ=:msg=UnVubmluZw==",
    );
    report(&mut term, "state=blocked:id=build/test:kind=permission");
    assert_eq!(record(&term, "build/test").app, None);
    assert_eq!(
        record(&term, "build/test").effective_app.as_deref(),
        Some("cargo")
    );
    // An absent intermediate parent does not break inheritance. A later parent
    // changes the effective app without rewriting its children's reports.
    report(&mut term, "state=idle:id=build:app=ninja");
    assert_eq!(
        record(&term, "build/test").effective_app.as_deref(),
        Some("ninja")
    );
    report(&mut term, "state=idle:id=build");
    assert_eq!(
        record(&term, "build/test").effective_app.as_deref(),
        Some("cargo")
    );
    report(&mut term, "state=idle");
    let root = record(&term, "");
    assert_eq!(root.app, None);
    assert_eq!(root.progress, None);
    assert_eq!(root.title, None);
    assert_eq!(root.msg, None);
    assert_eq!(record(&term, "build/test").effective_app, None);
}

#[test]
fn program_status_clear_uses_path_boundaries_and_missing_parents() {
    let mut term = TestTerm::new(3, 80, 0);
    for id in ["build/test", "build/test/unit", "builder", "build-two/test"] {
        report(&mut term, &format!("state=done:id={}", id));
    }
    // `build` itself is absent. Clearing it still removes descendants, but not
    // IDs that merely share the same byte prefix.
    report(&mut term, "state=clear:id=build");
    assert_eq!(ids(&term), vec!["build-two/test", "builder"]);
    report(&mut term, "state=clear:id=absent");
    assert_eq!(ids(&term), vec!["build-two/test", "builder"]);
    report(&mut term, "state=clear");
    assert!(term.get_program_status_records().is_empty());
}

#[test]
fn program_status_osc_133_a_cleans_transients_but_p_does_not() {
    let mut term = TestTerm::new(3, 80, 0);
    for state in ["working", "blocked", "done", "error", "idle"] {
        report(&mut term, &format!("state={0}:id={0}", state));
    }
    let original = term.get_program_status_records();
    // OSC 133 P is named StartPrompt in the parser. The required lifetime event
    // is OSC 133 A, named FreshLineAndStartPrompt instead.
    term.print(b"\x1b]133;P\x1b\\");
    assert_eq!(term.get_program_status_records(), original);
    term.print(b"\x1b]133;A\x07");
    assert_eq!(ids(&term), vec!["done", "error", "idle"]);
    term.print(b"\x1b]133;A\x1b\\");
    assert_eq!(ids(&term), vec!["done", "error", "idle"]);
    // A prompt ends one command, not the attached shell.
    report(&mut term, "state=working:id=next-command");
    assert_eq!(record(&term, "next-command").state, "working");
}

#[test]
fn program_status_full_reset_clears_but_soft_reset_and_screen_changes_preserve() {
    let mut term = TestTerm::new(3, 80, 0);
    report(&mut term, "state=blocked:app=cargo:kind=auth");
    report(&mut term, "state=done:id=tests");
    let original = term.get_program_status_records();
    term.print(b"\x1b[?1049h");
    assert_eq!(term.get_program_status_records(), original);
    term.print(b"\x1b[!p");
    assert_eq!(term.get_program_status_records(), original);
    term.print(b"\x1b[?1049l");
    assert_eq!(term.get_program_status_records(), original);
    term.print(b"\x1bc");
    assert!(term.get_program_status_records().is_empty());
    report(&mut term, "state=working:app=after-reset");
    assert_eq!(record(&term, "").state, "working");
}

struct CountChanges(Arc<AtomicUsize>);

impl AlertHandler for CountChanges {
    fn alert(&mut self, _alert: Alert) {}

    fn program_status_changed(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn program_status_change_callback_tracks_visible_mutations() {
    let mut term = TestTerm::new(3, 80, 0);
    let changes = Arc::new(AtomicUsize::new(0));
    term.set_notification_handler(Box::new(CountChanges(Arc::clone(&changes))));
    report(&mut term, "state=working:app=cargo");
    assert_eq!(changes.load(Ordering::SeqCst), 1);
    report(&mut term, "state=working:app=cargo");
    report(&mut term, "state=clear:id=absent");
    assert_eq!(changes.load(Ordering::SeqCst), 1);
    report(&mut term, "state=done:id=tests");
    assert_eq!(changes.load(Ordering::SeqCst), 2);
    term.print(b"\x1b]133;A\x1b\\");
    assert_eq!(changes.load(Ordering::SeqCst), 3);
    term.print(b"\x1b]133;A\x1b\\");
    assert_eq!(changes.load(Ordering::SeqCst), 3);
    term.print(b"\x1bc");
    assert_eq!(changes.load(Ordering::SeqCst), 4);
    term.print(b"\x1bc");
    assert_eq!(changes.load(Ordering::SeqCst), 4);
}

#[test]
fn program_status_process_exit_cleans_transients_and_preserves_results() {
    let mut term = TestTerm::new(3, 80, 0);
    for state in ["working", "blocked", "done", "error", "idle"] {
        report(&mut term, &format!("state={0}:id={0}", state));
    }
    // The process owner calls this even if the pane remains visible. Screen
    // destruction must not be needed for correct record lifetime.
    term.program_status_process_exited();
    assert_eq!(ids(&term), vec!["done", "error", "idle"]);
    term.program_status_process_exited();
    assert_eq!(ids(&term), vec!["done", "error", "idle"]);
}

#[test]
fn program_status_exit_before_buffer_drain_does_not_revive_work() {
    let mut term = TestTerm::new(3, 80, 0);
    term.program_status_process_exited();
    // The waiter can finish before the parser applies the child's final bytes.
    // Finished results are useful; transient work must not be revived.
    report(&mut term, "state=working:id=late-work");
    report(&mut term, "state=blocked:id=late-question:kind=permission");
    report(&mut term, "state=done:id=late-result");
    report(&mut term, "state=error:id=late-failure");
    assert_eq!(ids(&term), vec!["late-failure", "late-result"]);
    // Resetting the display does not restart an already-exited process.
    term.print(b"\x1bc");
    report(&mut term, "state=working:id=still-exited");
    assert!(term.get_program_status_records().is_empty());
}

#[test]
fn program_status_snapshots_are_owned_copies() {
    let mut term = TestTerm::new(3, 80, 0);
    report(&mut term, "state=working:app=cargo:msg=UnVubmluZw==");
    let mut copy = term.get_program_status_records();
    copy[0].app = Some("changed-by-caller".to_owned());
    copy[0].id = "changed-id".to_owned();
    assert_eq!(record(&term, "").app.as_deref(), Some("cargo"));
    assert_eq!(record(&term, "").msg.as_deref(), Some("Running"));
    report(&mut term, "state=clear");
    assert!(term.get_program_status_records().is_empty());
    assert_eq!(copy[0].state, "working");
    assert_eq!(copy[0].msg.as_deref(), Some("Running"));
}

struct QueryWriter(Sender<Vec<u8>>);

impl Write for QueryWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .send(bytes.to_vec())
            .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn receive_bytes(receiver: &Receiver<Vec<u8>>, expected_length: usize) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut result = Vec::new();
    while result.len() < expected_length {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .expect("terminal writer did not produce the capability response in time");
        result.extend(
            receiver
                .recv_timeout(remaining)
                .expect("terminal writer did not produce the capability response"),
        );
    }
    result
}

#[test]
fn program_status_only_queries_write_the_fixed_capability_reply() {
    let (sender, receiver) = mpsc::channel();
    let mut term = Terminal::new(
        TerminalSize {
            rows: 3,
            cols: 80,
            pixel_width: 640,
            pixel_height: 48,
            dpi: 0,
        },
        Arc::new(TestTermConfig { scrollback: 0 }),
        "WezTerm",
        "test",
        Box::new(QueryWriter(sender)),
    );
    let reply =
        b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";
    term.advance_bytes(b"\x1b]7501;?\x07");
    assert_eq!(
        receive_bytes(&receiver, reply.len()).as_slice(),
        reply.as_slice()
    );
    assert!(term.get_program_status_records().is_empty());
    // The second query is an ordering barrier for the asynchronous writer.
    // Any response to the report would precede this fixed response.
    term.advance_bytes(b"\x1b]7501;state=blocked:app=private:msg=U2VjcmV0\x1b\\");
    term.advance_bytes(b"\x1b]7501;?\x1b\\");
    assert_eq!(
        receive_bytes(&receiver, reply.len()).as_slice(),
        reply.as_slice()
    );
    assert_eq!(
        term.get_program_status_records()[0].msg.as_deref(),
        Some("Secret")
    );
}
