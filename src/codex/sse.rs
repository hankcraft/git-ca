/// Minimal Server-Sent Events parser.
///
/// Buffer bytes from the network with [`SseParser::push`] and drain whole
/// events with [`SseParser::next_event`]. Only the `event:` and `data:`
/// fields are surfaced — `id:` and `retry:` are accepted-but-ignored, which
/// is enough for the Responses API stream.
#[derive(Default)]
pub struct SseParser {
    buf: Vec<u8>,
    event_type: Option<String>,
    data: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, chunk: &[u8]) {
        self.buf.extend_from_slice(chunk);
    }

    /// Drain one complete event from the buffer if available. Returns `None`
    /// when more bytes are needed.
    pub fn next_event(&mut self) -> Option<SseEvent> {
        loop {
            let nl = self.buf.iter().position(|b| *b == b'\n')?;
            let mut line: Vec<u8> = self.buf.drain(..=nl).collect();
            line.pop(); // drop \n
            if line.last() == Some(&b'\r') {
                line.pop();
            }

            if line.is_empty() {
                if !self.data.is_empty() || self.event_type.is_some() {
                    let event = self.event_type.take().unwrap_or_else(|| "message".into());
                    let data = std::mem::take(&mut self.data);
                    let trimmed_data = data.strip_suffix('\n').unwrap_or(&data).to_string();
                    return Some(SseEvent {
                        event,
                        data: trimmed_data,
                    });
                }
                continue;
            }

            if line.starts_with(b":") {
                continue;
            }

            let (field, value) = split_field(&line);
            match field {
                "event" => self.event_type = Some(value.to_string()),
                "data" => {
                    if !self.data.is_empty() {
                        self.data.push('\n');
                    }
                    self.data.push_str(value);
                }
                _ => {} // id / retry / unknown — ignore
            }
        }
    }
}

fn split_field(line: &[u8]) -> (&str, &str) {
    let line = std::str::from_utf8(line).unwrap_or("");
    match line.split_once(':') {
        Some((field, value)) => (field.trim(), value.strip_prefix(' ').unwrap_or(value)),
        None => (line, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_event_is_parsed() {
        let mut p = SseParser::new();
        p.push(b"event: response.completed\ndata: {\"x\":1}\n\n");
        let ev = p.next_event().unwrap();
        assert_eq!(ev.event, "response.completed");
        assert_eq!(ev.data, "{\"x\":1}");
        assert!(p.next_event().is_none());
    }

    #[test]
    fn multiple_events_drain_in_order() {
        let mut p = SseParser::new();
        p.push(b"event: a\ndata: 1\n\nevent: b\ndata: 2\n\n");
        assert_eq!(
            p.next_event().unwrap(),
            SseEvent {
                event: "a".into(),
                data: "1".into(),
            }
        );
        assert_eq!(
            p.next_event().unwrap(),
            SseEvent {
                event: "b".into(),
                data: "2".into(),
            }
        );
        assert!(p.next_event().is_none());
    }

    #[test]
    fn data_without_event_uses_default_message_type() {
        let mut p = SseParser::new();
        p.push(b"data: hello\n\n");
        let ev = p.next_event().unwrap();
        assert_eq!(ev.event, "message");
        assert_eq!(ev.data, "hello");
    }

    #[test]
    fn multi_line_data_concatenates_with_newline() {
        let mut p = SseParser::new();
        p.push(b"data: line1\ndata: line2\n\n");
        let ev = p.next_event().unwrap();
        assert_eq!(ev.data, "line1\nline2");
    }

    #[test]
    fn event_split_across_chunks_assembles_correctly() {
        let mut p = SseParser::new();
        p.push(b"event: response.outpu");
        assert!(p.next_event().is_none());
        p.push(b"t_text.delta\ndata: {\"delta\":\"hi\"}\n\n");
        let ev = p.next_event().unwrap();
        assert_eq!(ev.event, "response.output_text.delta");
        assert_eq!(ev.data, "{\"delta\":\"hi\"}");
    }

    #[test]
    fn comment_lines_are_skipped() {
        let mut p = SseParser::new();
        p.push(b": keep-alive\nevent: a\ndata: 1\n\n");
        let ev = p.next_event().unwrap();
        assert_eq!(ev.event, "a");
        assert_eq!(ev.data, "1");
    }

    #[test]
    fn handles_crlf_line_endings() {
        let mut p = SseParser::new();
        p.push(b"event: a\r\ndata: 1\r\n\r\n");
        let ev = p.next_event().unwrap();
        assert_eq!(ev.event, "a");
        assert_eq!(ev.data, "1");
    }
}
