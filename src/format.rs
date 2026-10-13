use std::ops::Range;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub color: Option<u8>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub obfuscated: bool,
}

#[derive(Clone, Debug)]
pub struct Span {
    pub visible: Range<usize>,
    pub raw: Range<usize>,
    pub style: Style,
}

#[derive(Clone, Debug, Default)]
pub struct StyledLine {
    pub text: String,
    pub spans: Vec<Span>,
}

impl StyledLine {
    pub fn parse(raw: &str) -> Self {
        let mut output = Self::default();
        let mut style = Style::default();
        let mut chars = raw.char_indices().peekable();
        while let Some((start, ch)) = chars.next() {
            if ch == '§'
                && let Some(&(_, code)) = chars.peek()
            {
                let code = code.to_ascii_lowercase();
                let recognized = match code {
                    '0'..='9' | 'a'..='f' => {
                        style = Style {
                            color: Some(code.to_digit(16).unwrap() as u8),
                            ..Style::default()
                        };
                        true
                    }
                    'k' => {
                        style.obfuscated = true;
                        true
                    }
                    'l' => {
                        style.bold = true;
                        true
                    }
                    'm' => {
                        style.strike = true;
                        true
                    }
                    'n' => {
                        style.underline = true;
                        true
                    }
                    'o' => {
                        style.italic = true;
                        true
                    }
                    'r' => {
                        style = Style::default();
                        true
                    }
                    _ => false,
                };
                if recognized {
                    chars.next();
                    continue;
                }
            }
            let position = output.text.len();
            output.text.push(ch);
            let end = start + ch.len_utf8();
            if let Some(span) = output.spans.last_mut()
                && span.style == style
                && span.raw.end == start
            {
                span.visible.end = output.text.len();
                span.raw.end = end;
            } else {
                output.spans.push(Span {
                    visible: position..output.text.len(),
                    raw: start..end,
                    style,
                });
            }
        }
        output
    }

    pub fn raw_range(&self, visible: Range<usize>) -> Option<Range<usize>> {
        let first = self
            .spans
            .iter()
            .find(|span| span.visible.end > visible.start)?;
        let last = self
            .spans
            .iter()
            .rev()
            .find(|span| span.visible.start < visible.end)?;
        Some(
            (first.raw.start + visible.start.saturating_sub(first.visible.start))
                ..(last.raw.start + visible.end.saturating_sub(last.visible.start)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_match_crosses_codes_and_preserves_utf8_offsets() {
        let raw = "§a你好§lABC§bDEF";
        let styled = StyledLine::parse(raw);
        assert_eq!(styled.text, "你好ABCDEF");
        let range = styled.raw_range(6..12).unwrap();
        assert_eq!(&raw[range], "ABC§bDEF");
        assert!(styled.spans[1].style.bold);
        assert!(!styled.spans[2].style.bold);
    }
    #[test]
    fn reset_and_unknown_codes() {
        let line = StyledLine::parse("§Lx§nY§rZ§q?§");
        assert_eq!(line.text, "xYZ§q?§");
        assert!(line.spans[0].style.bold);
        assert!(line.spans[1].style.underline);
        assert_eq!(line.spans[2].style, Style::default());
    }
}
