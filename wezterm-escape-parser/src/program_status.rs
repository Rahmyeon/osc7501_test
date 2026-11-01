//! Validated, owned reports for the OSC 7501 Program Status Protocol.
//!
//! This module parses one body at a time. Record storage and lifetime rules
//! belong to the terminal model, independently of escape-sequence framing.

use crate::allocate::*;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use core::fmt::{Display, Formatter, Result as FmtResult};

pub const MAX_SEQUENCE_BYTES: usize = 4096;
// Reserve the longer terminator, including for BEL-terminated input.
pub const MAX_BODY_BYTES: usize = MAX_SEQUENCE_BYTES - b"\x1b]7501;".len() - b"\x1b\\".len();
pub const MAX_MESSAGE_BYTES: usize = 2048;
pub const MAX_TITLE_BYTES: usize = 192;
pub const MAX_ID_BYTES: usize = 128;
pub const SUPPORT_REPLY: &[u8] =
    b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Permission,
    Question,
    Auth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub state: State,
    /// Empty means the root record. An explicitly empty wire id is invalid.
    pub id: String,
    pub kind: Option<Kind>,
    pub progress: Option<u8>,
    pub app: Option<String>,
    pub title: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Query,
    Report(Report),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    TooLong,
    InvalidState,
    InvalidId,
    InvalidText,
}

impl Command {
    /// Parse the bytes following `7501;`, excluding the terminator.
    /// No partial report escapes this function: validate before changing state.
    pub fn parse_body(body: &[u8]) -> core::result::Result<Self, ParseError> {
        if body.len() > MAX_BODY_BYTES {
            return Err(ParseError::TooLong);
        }
        if body == b"?" {
            return Ok(Self::Query);
        }

        let mut state = None;
        let mut id = None;
        let mut kind = None;
        let mut progress = None;
        let mut app = None;
        let mut title = None;
        let mut message = None;

        for pair in body.split(|b| *b == b':') {
            let Some(eq) = pair.iter().position(|b| *b == b'=') else {
                continue;
            };
            let key = pair[..eq].trim_ascii();
            let value = pair[eq + 1..].trim_ascii();
            if key.len() > 16 {
                return Err(ParseError::TooLong);
            }
            if key.is_empty() || !key.iter().all(u8::is_ascii_lowercase) {
                continue;
            }

            // Limits apply even to an overwritten or otherwise malformed pair.
            // An invalid explicit id must never turn into an absent/root id.
            match key {
                b"id" => validate_id(value)?,
                b"app" => check_len(value.len(), 32)?,
                b"title" => check_len(value.len(), 256)?,
                b"msg" => check_len(value.len(), 2732)?,
                _ => {}
            }
            if !value.iter().all(|b| is_name_byte(*b) || b",/=".contains(b)) {
                continue;
            }

            // Check every occurrence's limits, including overwritten values.
            match key {
                b"state" => state = Some(value),
                b"id" => id = Some(value),
                b"kind" => kind = Some(value),
                b"progress" => progress = Some(value),
                b"app" => app = Some(value),
                b"title" => {
                    title = Some(decode_text(value, MAX_TITLE_BYTES)?);
                }
                b"msg" => {
                    message = Some(decode_text(value, MAX_MESSAGE_BYTES)?);
                }
                _ => {}
            }
        }

        let state = match state {
            Some(b"idle") => State::Idle,
            Some(b"working") => State::Working,
            Some(b"done") => State::Done,
            Some(b"blocked") => State::Blocked,
            Some(b"error") => State::Error,
            Some(b"clear") => State::Clear,
            _ => return Err(ParseError::InvalidState),
        };
        let kind = if state == State::Blocked {
            match kind {
                Some(b"permission") => Some(Kind::Permission),
                Some(b"question") => Some(Kind::Question),
                Some(b"auth") => Some(Kind::Auth),
                _ => None,
            }
        } else {
            None
        };
        let progress = if matches!(state, State::Working | State::Blocked) {
            progress.and_then(parse_progress)
        } else {
            None
        };

        Ok(Self::Report(Report {
            state,
            id: ascii_string(id.unwrap_or(b"")),
            kind,
            progress,
            app: app
                .filter(|v| !v.is_empty() && v.iter().all(|b| is_name_byte(*b)))
                .map(ascii_string),
            title,
            message,
        }))
    }
}

fn check_len(len: usize, max: usize) -> core::result::Result<(), ParseError> {
    if len > max {
        Err(ParseError::TooLong)
    } else {
        Ok(())
    }
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"_.+-".contains(&b)
}

fn ascii_string(value: &[u8]) -> String {
    // Callers only pass bytes already checked against the ASCII alphabet.
    core::str::from_utf8(value)
        .expect("validated ASCII")
        .to_owned()
}

fn validate_id(value: &[u8]) -> core::result::Result<(), ParseError> {
    check_len(value.len(), MAX_ID_BYTES)?;
    let mut depth = 0;
    for segment in value.split(|b| *b == b'/') {
        depth += 1;
        check_len(depth, 8)?;
        check_len(segment.len(), 32)?;
        if segment.is_empty() || !segment.iter().all(|b| is_name_byte(*b)) {
            return Err(ParseError::InvalidId);
        }
    }
    Ok(())
}

fn parse_progress(value: &[u8]) -> Option<u8> {
    if value.is_empty() || !value.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let n = core::str::from_utf8(value).ok()?.parse::<u8>().ok()?;
    (n <= 100).then_some(n)
}

fn decode_text(value: &[u8], max: usize) -> core::result::Result<String, ParseError> {
    let engine = if value.ends_with(b"=") {
        &STANDARD
    } else {
        &STANDARD_NO_PAD
    };
    let decoded = engine.decode(value).map_err(|_| ParseError::InvalidText)?;
    check_len(decoded.len(), max)?;
    let text = String::from_utf8(decoded).map_err(|_| ParseError::InvalidText)?;
    if text.chars().any(char::is_control) {
        return Err(ParseError::InvalidText);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;
    use crate::{Action, OperatingSystemCommand};
    #[cfg(not(feature = "std"))]
    use alloc::{format, vec};

    fn report(body: &[u8]) -> Report {
        match Command::parse_body(body).unwrap() {
            Command::Report(report) => report,
            Command::Query => panic!("expected report"),
        }
    }

    #[test]
    fn program_status_query_and_states() {
        assert_eq!(Command::parse_body(b"?"), Ok(Command::Query));
        for (name, state) in [
            ("idle", State::Idle),
            ("working", State::Working),
            ("done", State::Done),
            ("blocked", State::Blocked),
            ("error", State::Error),
            ("clear", State::Clear),
        ] {
            assert_eq!(report(format!("state={name}").as_bytes()).state, state);
        }
        for body in [b"".as_slice(), b"app=cargo", b"state=unknown", b"state="] {
            assert_eq!(Command::parse_body(body), Err(ParseError::InvalidState));
        }
    }

    #[test]
    fn program_status_all_fields_and_owned_text() {
        let mut input = b"state=blocked:kind=permission:id=build/test:progress=42:app=cargo:title=UGxhbg:msg=QXBwcm92ZT8=".to_vec();
        let record = report(&input);
        input.fill(b'x');
        assert_eq!(record.state, State::Blocked);
        assert_eq!(record.kind, Some(Kind::Permission));
        assert_eq!(record.progress, Some(42));
        assert_eq!(record.id, "build/test");
        assert_eq!(record.app.as_deref(), Some("cargo"));
        assert_eq!(record.title.as_deref(), Some("Plan"));
        assert_eq!(record.message.as_deref(), Some("Approve?"));
    }

    #[test]
    fn program_status_pairs_are_forgiving_and_last_value_wins() {
        let record = report(b" state = working :oops:future=yes:progress=1:progress=80:app=old:app=new:bad=\xff:kind=question");
        assert_eq!(record.progress, Some(80));
        assert_eq!(record.app.as_deref(), Some("new"));
        assert_eq!(record.kind, None);
        assert_eq!(report(b"state=done:progress=40:kind=auth").progress, None);
        assert_eq!(report(b"state=blocked:kind=future").kind, None);
        assert_eq!(report(b"state=idle:app=bad/name").app, None);
    }

    #[test]
    fn program_status_progress_validation() {
        for value in ["", "101", "300", "-1", "1.0", "1_0", "unknown"] {
            assert_eq!(
                report(format!("state=working:progress={value}").as_bytes()).progress,
                None
            );
        }
        assert_eq!(report(b"state=blocked:progress=0").progress, Some(0));
        assert_eq!(report(b"state=working:progress=100").progress, Some(100));
    }

    #[test]
    fn program_status_ids_never_fall_back_to_root() {
        for id in [
            "", "/a", "a/", "a//b", "a,b", "a=b", "bad@id", "bad id", "é", "bad\0id",
        ] {
            assert_eq!(
                Command::parse_body(format!("state=clear:id={id}").as_bytes()),
                Err(ParseError::InvalidId)
            );
        }
        assert_eq!(report(b"state=clear").id, "");
        assert_eq!(report(b"state=working:id=A_b.+-1/test").id, "A_b.+-1/test");
        assert_eq!(
            Command::parse_body(b"state=idle:id=bad//id:id=good"),
            Err(ParseError::InvalidId)
        );
        let too_deep = "a/".repeat(8) + "a";
        assert_eq!(
            Command::parse_body(format!("state=idle:id={too_deep}").as_bytes()),
            Err(ParseError::TooLong)
        );
        assert_eq!(
            Command::parse_body(format!("state=idle:id={}", "a".repeat(33)).as_bytes()),
            Err(ParseError::TooLong)
        );
    }

    #[test]
    fn program_status_invalid_text_discards_report() {
        for bytes in [
            b"line\n".as_slice(),
            b"tab\t",
            b"\x1b[31m",
            b"\xff",
            "\u{85}".as_bytes(),
        ] {
            let encoded = STANDARD.encode(bytes);
            assert_eq!(
                Command::parse_body(format!("state=working:msg={encoded}").as_bytes()),
                Err(ParseError::InvalidText)
            );
        }
        assert_eq!(
            Command::parse_body(b"state=working:msg=abcde"),
            Err(ParseError::InvalidText)
        );
        let encoded = STANDARD_NO_PAD.encode("部署中");
        assert_eq!(
            report(format!("state=working:msg={encoded}").as_bytes())
                .message
                .as_deref(),
            Some("部署中")
        );
        assert_eq!(report(b"state=idle:msg=").message.as_deref(), Some(""));
    }

    #[test]
    fn program_status_limits_apply_before_any_report_is_returned() {
        assert!(
            Command::parse_body(
                format!("state=idle:future={}", "x".repeat(MAX_BODY_BYTES - 18)).as_bytes()
            )
            .is_ok()
        );
        assert_eq!(
            Command::parse_body(&vec![b'x'; MAX_BODY_BYTES + 1]),
            Err(ParseError::TooLong)
        );
        assert_eq!(
            Command::parse_body(format!("state=idle:{}=x", "x".repeat(17)).as_bytes()),
            Err(ParseError::TooLong)
        );
        assert_eq!(
            Command::parse_body(format!("state=idle:app={}:app=ok", "x".repeat(33)).as_bytes()),
            Err(ParseError::TooLong)
        );
        for (key, max) in [("msg", MAX_MESSAGE_BYTES), ("title", MAX_TITLE_BYTES)] {
            let text = "x".repeat(max);
            assert!(
                Command::parse_body(
                    format!("state=idle:{key}={}", STANDARD.encode(&text)).as_bytes()
                )
                .is_ok()
            );
            assert_eq!(
                Command::parse_body(
                    format!("state=idle:{key}={}", STANDARD.encode(text + "x")).as_bytes()
                ),
                Err(ParseError::TooLong)
            );
        }
    }

    #[test]
    fn program_status_overwritten_fields_still_require_validation() {
        for body in [
            b"state=done:msg=abcde:msg=b2s=".as_slice(),
            b"state=done:title=Cg==:title=b2s=",
        ] {
            assert_eq!(Command::parse_body(body), Err(ParseError::InvalidText));
        }
        assert_eq!(
            Command::parse_body(b"state=clear:id=bad%id"),
            Err(ParseError::InvalidId)
        );
        assert_eq!(
            Command::parse_body(format!("state=done:app={}", "%".repeat(33)).as_bytes()),
            Err(ParseError::TooLong)
        );
    }

    #[test]
    fn program_status_osc_framing_and_encoding() {
        for terminator in ["\x07", "\x1b\\"] {
            let mut parser = Parser::new();
            let query = parser.parse_as_vec(format!("\x1b]7501;?{terminator}").as_bytes());
            assert_eq!(
                query,
                vec![Action::OperatingSystemCommand(Box::new(
                    OperatingSystemCommand::ProgramStatus(Command::Query)
                ))]
            );
            assert_eq!(format!("{}", query[0]), "\x1b]7501;?\x1b\\");

            let command = Command::parse_body(b"state=blocked:id=build/test:kind=question:progress=42:app=cargo:title=UGxhbg:msg=QXBwcm92ZT8=").unwrap();
            let osc = OperatingSystemCommand::ProgramStatus(command.clone());
            let encoded = format!("{osc}");
            let actions = parser.parse_as_vec(encoded.as_bytes());
            assert_eq!(actions, vec![Action::OperatingSystemCommand(Box::new(osc))]);
            assert_eq!(
                encoded,
                "\x1b]7501;state=blocked:id=build/test:kind=question:progress=42:app=cargo:title=UGxhbg==:msg=QXBwcm92ZT8=\x1b\\"
            );
        }
    }

    #[test]
    fn program_status_fragmented_reports_require_complete_terminator() {
        for wire in [
            b"\x1b]7501;state=working:id=build:progress=5\x1b\\".as_slice(),
            b"\x1b]7501;state=working:id=build:progress=5\x07",
        ] {
            let expected = Parser::new().parse_as_vec(wire);
            assert_eq!(expected.len(), 1);
            for split in 0..wire.len() {
                let mut parser = Parser::new();
                assert!(parser.parse_as_vec(&wire[..split]).is_empty());
                assert_eq!(parser.parse_as_vec(&wire[split..]), expected);
            }
            let mut parser = Parser::new();
            for byte in &wire[..wire.len() - 1] {
                assert!(parser.parse_as_vec(&[*byte]).is_empty());
            }
            assert_eq!(parser.parse_as_vec(&wire[wire.len() - 1..]), expected);
        }
    }

    #[test]
    fn program_status_bad_reports_cannot_produce_valid_commands() {
        for body in [
            "state=working:msg=abcde",
            "state=working:id=a//b",
            "state=future",
            "app=cargo",
        ] {
            let actions = Parser::new().parse_as_vec(format!("\x1b]7501;{body}\x07").as_bytes());
            assert!(matches!(
                &actions[..],
                [Action::OperatingSystemCommand(osc)]
                    if matches!(&**osc, OperatingSystemCommand::Unspecified(_))
            ));
        }
    }

    #[test]
    fn program_status_malformed_semicolon_pair_is_skipped() {
        let body = b"state=working:future=bad;value:progress=50";
        let expected = Command::parse_body(body).unwrap();
        let actions =
            Parser::new().parse_as_vec(b"\x1b]7501;state=working:future=bad;value:progress=50\x07");
        assert_eq!(
            actions,
            vec![Action::OperatingSystemCommand(Box::new(
                OperatingSystemCommand::ProgramStatus(expected.clone())
            ))]
        );
        assert_eq!(
            OperatingSystemCommand::parse(&[
                b"7501",
                b"state=working:future=bad",
                b"value:progress=50",
            ]),
            OperatingSystemCommand::ProgramStatus(expected)
        );
    }

    #[test]
    fn program_status_oversized_stream_is_discarded_and_parser_recovers() {
        let mut parser = Parser::new();
        let mut wire = b"\x1b]7501;state=working:future=".to_vec();
        wire.extend_from_slice(&vec![b'x'; MAX_SEQUENCE_BYTES * 2]);
        assert!(parser.parse_as_vec(&wire).is_empty());
        assert!(parser.parse_as_vec(b"\x1b\\").is_empty());
        assert_eq!(
            parser.parse_as_vec(b"\x1b]7501;?\x07"),
            vec![Action::OperatingSystemCommand(Box::new(
                OperatingSystemCommand::ProgramStatus(Command::Query)
            ))]
        );
    }

    #[test]
    fn program_status_raw_controls_do_not_manufacture_valid_values() {
        for wire in [
            b"\x1b]7501;state=wo\0rking\x07".as_slice(),
            b"\x1b]7501;state=clear:id=bad\0id\x07",
            b"\x1b]7501;state=clear:id=bad\x7fid\x07",
        ] {
            let actions = Parser::new().parse_as_vec(wire);
            assert!(matches!(
                &actions[..],
                [Action::OperatingSystemCommand(osc)]
                    if matches!(&**osc, OperatingSystemCommand::Unspecified(_))
            ));
        }
        let actions =
            Parser::new().parse_as_vec(b"\x1b]7501;state=working:future=x\0y:progress=5\x07");
        assert!(matches!(
            &actions[..],
            [Action::OperatingSystemCommand(osc)]
                if matches!(&**osc, OperatingSystemCommand::ProgramStatus(Command::Report(report))
                    if report.progress == Some(5))
        ));
    }

    #[test]
    fn program_status_invalid_utf8_bytes_cannot_repair_an_id() {
        for byte in (0xa0..=0xc1).chain(0xf5..=0xff) {
            for terminator in [b"\x07".as_slice(), b"\x1b\\"] {
                let mut wire = b"\x1b]7501;state=clear:id=bad".to_vec();
                wire.push(byte);
                wire.extend_from_slice(b"id");
                wire.extend_from_slice(terminator);
                let actions = Parser::new().parse_as_vec(&wire);
                assert!(matches!(
                    &actions[..],
                    [Action::OperatingSystemCommand(osc)]
                        if matches!(&**osc, OperatingSystemCommand::Unspecified(_))
                ));

                // The same malformed byte in an unknown pair skips that pair,
                // rather than discarding otherwise valid report fields.
                let mut wire = b"\x1b]7501;state=working:future=bad".to_vec();
                wire.push(byte);
                wire.extend_from_slice(b"value:progress=5");
                wire.extend_from_slice(terminator);
                let actions = Parser::new().parse_as_vec(&wire);
                assert!(matches!(
                    &actions[..],
                    [Action::OperatingSystemCommand(osc)]
                        if matches!(&**osc, OperatingSystemCommand::ProgramStatus(Command::Report(report))
                            if report.state == State::Working && report.progress == Some(5))
                ));
            }
        }
    }

    #[test]
    fn program_status_incomplete_utf8_cannot_swallow_an_id_pair() {
        for lead in [b"\xc2".as_slice(), b"\xe0", b"\xf0\xa0"] {
            for terminator in [b"\x07".as_slice(), b"\x1b\\"] {
                let mut wire = b"\x1b]7501;state=clear:future=x".to_vec();
                wire.extend_from_slice(lead);
                wire.extend_from_slice(b":id=target");
                wire.extend_from_slice(terminator);
                let actions = Parser::new().parse_as_vec(&wire);
                assert!(matches!(
                    &actions[..],
                    [Action::OperatingSystemCommand(osc)]
                        if matches!(&**osc, OperatingSystemCommand::ProgramStatus(Command::Report(report))
                            if report.state == State::Clear && report.id == "target")
                ));
            }
        }
    }
}

/// Formats only the body; OperatingSystemCommand supplies OSC framing.
impl Display for Command {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        let Self::Report(report) = self else {
            return f.write_str("?");
        };
        let state = match report.state {
            State::Idle => "idle",
            State::Working => "working",
            State::Done => "done",
            State::Blocked => "blocked",
            State::Error => "error",
            State::Clear => "clear",
        };
        write!(f, "state={state}")?;
        if !report.id.is_empty() {
            write!(f, ":id={}", report.id)?;
        }
        if let Some(kind) = report.kind {
            let kind = match kind {
                Kind::Permission => "permission",
                Kind::Question => "question",
                Kind::Auth => "auth",
            };
            write!(f, ":kind={kind}")?;
        }
        if let Some(progress) = report.progress {
            write!(f, ":progress={progress}")?;
        }
        if let Some(app) = &report.app {
            write!(f, ":app={app}")?;
        }
        if let Some(title) = &report.title {
            write!(f, ":title={}", STANDARD.encode(title))?;
        }
        if let Some(message) = &report.message {
            write!(f, ":msg={}", STANDARD.encode(message))?;
        }
        Ok(())
    }
}
