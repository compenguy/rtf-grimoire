// RTF document format tokenizer
//
// Written according to the RTF Format Specification 1.9.1, which carries
// the following copyright notice:
//
//     Copyright (c) 2008 Microsoft Corporation.  All Rights reserved.
//

use crate::raw::{control_bin_raw, control_symbol_raw, control_word_hexbyte_raw, control_word_raw};
use crate::raw::{end_group_raw, newline_raw, rtf_text_raw, start_group_raw};

use nom::types::CompleteByteSlice as Input;

#[derive(Debug)]
pub struct ParseError {
    inner: nom::ErrorKind<u32>,
}

impl<I> std::convert::From<nom::Err<I, u32>> for ParseError {
    fn from(error: nom::Err<I, u32>) -> Self {
        Self {
            inner: error.into_error_kind(),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Parser Error: {}", self.inner.description())
    }
}

type Result<T> = std::result::Result<T, ParseError>;

#[derive(PartialEq)]
pub enum Token {
    ControlSymbol(char),
    ControlWord {
        name: String,
        arg: Option<i32>,
    },
    ControlBin(Vec<u8>),
    /// Text is not str because it can be in any of various encodings -
    /// it's up to the processor to identify any encoding information in
    /// the stream, and do any encoding conversion desired
    Text(Vec<u8>),
    StartGroup,
    EndGroup,
    Newline,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Token::ControlSymbol(c) => write!(f, "Token::ControlSymbol({})", c),
            Token::ControlWord { name, arg } => write!(
                f,
                "Token::ControlWord({}{})",
                name,
                arg.map(|i| format!(":{}", i)).unwrap_or_default()
            ),
            Token::ControlBin(data) => {
                write!(f, "Token::ControlBin(")?;
                for byte in data {
                    write!(f, " {:02x?}", byte)?;
                }
                write!(f, ")")
            }
            Token::Text(data) => {
                write!(f, "Token::Text(")?;
                for byte in data {
                    write!(f, " {:02x?}", byte)?;
                }
                write!(f, ")")
            }
            Token::StartGroup => write!(f, "Token::StartGroup"),
            Token::EndGroup => write!(f, "Token::EndGroup"),
            Token::Newline => write!(f, "Token::Newline"),
        }
    }
}

impl Token {
    pub fn to_rtf(&self) -> Vec<u8> {
        match self {
            Token::ControlSymbol(c) => format!("\\{}", c).as_bytes().to_vec(),
            Token::ControlWord { name, arg } => match arg {
                Some(num) => format!("\\{}{}", name, num).as_bytes().to_vec(),
                None => format!("\\{}", name).as_bytes().to_vec(),
            },
            Token::ControlBin(data) => {
                let mut rtf: Vec<u8> = Vec::with_capacity(12 + data.len());
                rtf.extend_from_slice(format!("\\bin{} ", data.len()).as_bytes());
                rtf.extend_from_slice(data);
                rtf
            }
            Token::Text(data) => data.to_vec(),
            Token::StartGroup => b"{".to_vec(),
            Token::EndGroup => b"}".to_vec(),
            Token::Newline => b"\\r\\n".to_vec(),
        }
    }

    /// This function returns a control word delimiter if one is required, or an
    /// empty string if none is required
    ///
    /// Control Word tokens must be delimited by a non-alphanumeric value, so
    /// if the subsequent content could be alphanumeric, a space (' ') delimiter
    /// must be inserted
    pub fn token_delimiter_after(&self, next_token: &Token) -> &'static str {
        if let Token::ControlWord { .. } = self {
            // TODO: actually check the content of Text to see if a space is needed
            // it's safe to be lazy here, but less efficient
            if let Token::Text(_) = next_token {
                return " ";
            }
        }
        ""
    }

    /// This function returns a control word delimiter if one is required, or an
    /// empty string if none is required
    ///
    /// Control Word tokens must be delimited by a non-alphanumeric value, so
    /// if the subsequent content could be alphanumeric, a space (' ') delimiter
    /// must be inserted
    pub fn token_delimiter_before(&self, prev_token: &Token) -> &'static str {
        prev_token.token_delimiter_after(self)
    }

    pub fn get_name(&self) -> Option<String> {
        if let Token::ControlWord { ref name, .. } = self {
            Some(name.clone())
        } else {
            None
        }
    }

    pub fn get_arg(&self) -> Option<i32> {
        if let Token::ControlWord { ref arg, .. } = self {
            *arg
        } else {
            None
        }
    }

    pub fn get_symbol(&self) -> Option<char> {
        if let Token::ControlSymbol(c) = self {
            Some(*c)
        } else {
            None
        }
    }

    pub fn get_bin(&self) -> Option<&[u8]> {
        if let Token::ControlBin(data) = self {
            Some(data.as_slice())
        } else {
            None
        }
    }

    pub fn get_text(&self) -> Option<&[u8]> {
        if let Token::Text(data) = self {
            Some(data.as_slice())
        } else {
            None
        }
    }
}

// Ordering here is important. Plain text is all content that isn't something else:
// If the next unparsed character is anything other than an opening brace ({), closing brace (}),
// backslash (\), or a CRLF (carriage return/line feed), the reader assumes that the character is
// plain text and writes the character to the current destination using the current formatting
// properties.  Finally, a control hexbyte is a special case of a control symbol, but needs to be
// handled specially, so hexbyte should be tested for before control symbols.
//
// See section "Conventions of an RTF Reader" in the RTF specification.
named!(pub read_token<Input, Token>,
    alt!(read_control_hexbyte | read_control_symbol | read_control_bin | read_control_word | read_start_group | read_end_group | read_newline | read_rtf_text)
);

named!(pub read_control_hexbyte<Input, Token>,
    map!(
        control_word_hexbyte_raw,
        |(name, arg)| Token::ControlWord { name: String::from(name), arg }
    )
);

named!(pub read_control_symbol<Input, Token>,
    map!(
        control_symbol_raw,
        Token::ControlSymbol
    )
);

named!(pub read_control_word<Input, Token>,
    map!(
        control_word_raw,
        |(name, arg)| Token::ControlWord { name: String::from(name), arg }
    )
);

named!(pub read_control_bin<Input, Token>,
    map!(
        control_bin_raw,
        |bytes| Token::ControlBin(bytes.to_vec())
    )
);

named!(pub read_newline<Input, Token>,
    map!(
        newline_raw,
        |_| Token::Newline
    )
);

named!(pub read_start_group<Input, Token>,
    map!(
        start_group_raw,
        |_| Token::StartGroup
    )
);

named!(pub read_end_group<Input, Token>,
    map!(
        end_group_raw,
        |_| Token::EndGroup
    )
);

named!(pub read_rtf_text<Input, Token>,
    map!(
        rtf_text_raw,
        |text_bytes| Token::Text(text_bytes.to_vec())
    )
);

named!(pub read_token_stream<Input, Vec<Token> >, many0!(read_token));

pub fn parse(bytes: &[u8]) -> Result<Vec<Token>> {
    read_token_stream(Input(bytes))
        .map_err(ParseError::from)
        .map(|(_, tokens)| tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_symbol_tokens() {
        let syms_str = br#"\*\.\+\~"#;
        let valid_syms = vec![
            Token::ControlSymbol('*'),
            Token::ControlSymbol('.'),
            Token::ControlSymbol('+'),
            Token::ControlSymbol('~'),
        ];
        let syms_after_parse = Input(b"");
        let syms = read_token_stream(Input(syms_str));
        assert_eq!(syms, Ok((syms_after_parse, valid_syms)));
    }

    #[test]
    fn test_control_word_tokens() {
        let words_str = br#"\par\b0\b\uncle\foo-5\applepi314159"#;
        let valid_words = vec![
            Token::ControlWord {
                name: "par".to_string(),
                arg: None,
            },
            Token::ControlWord {
                name: "b".to_string(),
                arg: Some(0),
            },
            Token::ControlWord {
                name: "b".to_string(),
                arg: None,
            },
            Token::ControlWord {
                name: "uncle".to_string(),
                arg: None,
            },
            Token::ControlWord {
                name: "foo".to_string(),
                arg: Some(-5),
            },
            Token::ControlWord {
                name: "applepi".to_string(),
                arg: Some(314159),
            },
        ];
        let words_after_parse = Input(b"");
        let words = read_token_stream(Input(words_str));
        assert_eq!(words, Ok((words_after_parse, valid_words)));
    }

    #[test]
    fn test_control_bin_tokens() {
        let bins_str = b"\\bin5 ABC{}\\bin1 {\\bin0 \\bin0\\bin1  \\bin1\x01\\bin1 \x02";
        let valid_bins = vec![
            Token::ControlBin(b"ABC{}".to_vec()),
            Token::ControlBin(b"{".to_vec()),
            Token::ControlBin(b"".to_vec()),
            Token::ControlBin(b"".to_vec()),
            Token::ControlBin(b" ".to_vec()),
            Token::ControlBin(b"\x01".to_vec()),
            Token::ControlBin(b"\x02".to_vec()),
        ];
        let bins_after_parse = Input(b"");
        let bins = read_token_stream(Input(bins_str));
        assert_eq!(bins, Ok((bins_after_parse, valid_bins)));
    }

    #[test]
    fn test_control() {
        let controls_str = b"\\*\\bin5 ABC{}\\b\\bin1 {\\bin0 \\b0\\bin0\\bin1  \\supercalifragilistic31415\\bin1\x01\\bin1 \x02";
        let valid_controls = vec![
            Token::ControlSymbol('*'),
            Token::ControlBin(b"ABC{}".to_vec()),
            Token::ControlWord {
                name: "b".to_string(),
                arg: None,
            },
            Token::ControlBin(b"{".to_vec()),
            Token::ControlBin(b"".to_vec()),
            Token::ControlWord {
                name: "b".to_string(),
                arg: Some(0),
            },
            Token::ControlBin(b"".to_vec()),
            Token::ControlBin(b" ".to_vec()),
            Token::ControlWord {
                name: "supercalifragilistic".to_string(),
                arg: Some(31415),
            },
            Token::ControlBin(b"\x01".to_vec()),
            Token::ControlBin(b"\x02".to_vec()),
        ];
        let controls_after_parse = Input(b"");
        let controls = read_token_stream(Input(controls_str));
        assert_eq!(controls, Ok((controls_after_parse, valid_controls)));
    }

    #[test]
    fn test_group_tokens() {
        // Have to be very careful here to insert crlf, regardless of host platform
        let group_content_str = b"\\b Hello World \\b0 \\par\r\nThis is a test {\\*\\nothing}";
        let valid_group_content = vec![
            Token::ControlWord {
                name: "b".to_string(),
                arg: None,
            },
            Token::Text(b"Hello World ".to_vec()),
            Token::ControlWord {
                name: "b".to_string(),
                arg: Some(0),
            },
            Token::ControlWord {
                name: "par".to_string(),
                arg: None,
            },
            Token::Newline,
            Token::Text(b"This is a test ".to_vec()),
            Token::StartGroup,
            Token::ControlSymbol('*'),
            Token::ControlWord {
                name: "nothing".to_string(),
                arg: None,
            },
            Token::EndGroup,
        ];
        let group_content_after_parse = Input(b"");
        let group_content = read_token_stream(Input(group_content_str));
        assert_eq!(
            group_content,
            Ok((group_content_after_parse, valid_group_content))
        );
    }

    #[test]
    fn test_sample_doc() {
        let test_bytes = include_bytes!("../tests/sample.rtf");
        if let Err(e) = parse(test_bytes) {
            panic!("Parsing error: {:?}", e);
        }
        match read_token_stream(Input(test_bytes)) {
            Ok((unparsed, _)) => assert_eq!(
                unparsed.len(),
                0,
                "Unparsed data: {} bytes (first <=5 bytes: {:02X?})",
                unparsed.len(),
                &unparsed[0..std::cmp::min(5, unparsed.len())]
            ),
            Err(e) => panic!("Parsing error: {:?}", e),
        }
    }

    // The spec doc is interested because it has unmatched "{}" groups
    #[test]
    fn test_spec_doc() {
        let test_bytes = include_bytes!("../tests/RTF-Spec-1.7.rtf");
        if let Err(e) = parse(test_bytes) {
            panic!("Parsing error: {:?}", e);
        }
        match read_token_stream(Input(test_bytes)) {
            Ok((unparsed, _)) => assert_eq!(
                unparsed.len(),
                0,
                "Unparsed data: {} bytes (first <=5 bytes: {:02X?})",
                unparsed.len(),
                &unparsed[0..std::cmp::min(5, unparsed.len())]
            ),
            Err(e) => panic!("Parsing error: {:?}", e),
        }
    }
}
