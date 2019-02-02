// RTF document format tokenizer
//
// Written according to the RTF Format Specification 1.9.1, which carries
// the following copyright notice:
//
//     Copyright (c) 2008 Microsoft Corporation.  All Rights reserved.
//

use nom::crlf;
use nom::types::CompleteByteSlice;

use crate::raw::{control_bin_raw, control_symbol_raw, control_word_raw};
use crate::raw::{document_raw, group_raw, rtf_text_raw};

#[derive(PartialEq)]
pub enum Control {
    Symbol(char),
    Word { name: String, arg: Option<i32> },
    Bin(Vec<u8>),
}

impl std::fmt::Debug for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Control::Symbol(c) => write!(f, "Control::Symbol({})", c),
            Control::Word { name, arg } => write!(
                f,
                "Control::Word({}{})",
                name,
                arg.map(|i| format!(":{}", i)).unwrap_or_default()
            ),
            Control::Bin(data) => {
                write!(f, "Control::Bin(")?;
                for byte in data {
                    write!(f, " {:02x?}", byte)?;
                }
                write!(f, ")")
            }
        }
    }
}

impl Control {
    pub fn get_name(&self) -> Option<String> {
        if let Control::Word { ref name, .. } = self {
            Some(name.clone())
        } else {
            None
        }
    }

    pub fn get_arg(&self) -> Option<i32> {
        if let Control::Word { ref arg, .. } = self {
            *arg
        } else {
            None
        }
    }
}

named!(pub control<CompleteByteSlice, Control>,
    alt!(control_symbol | control_bin | control_word)
);

named!(pub control_symbol<CompleteByteSlice, Control>,
    map!(
        control_symbol_raw,
        Control::Symbol
    )
);

named!(pub control_word<CompleteByteSlice, Control>,
    map!(
        control_word_raw,
        |(name, arg)| Control::Word { name: String::from(name), arg }
    )
);

named!(pub control_bin<CompleteByteSlice, Control>,
    map!(
        control_bin_raw,
        |bytes| Control::Bin(bytes.to_vec())
    )
);

// Text is not str because it can be in any of various encodings -
// it's up to the processor to identify any encoding information in
// the stream, and do any encoding conversion desired
#[derive(PartialEq)]
pub enum GroupContent {
    Control(Control),
    Group(Group),
    Text(Vec<u8>),
    Newline,
}

impl std::fmt::Debug for GroupContent {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            GroupContent::Control(c) => write!(f, "\nGroupContent::Control({:?})", c),
            GroupContent::Group(g) => write!(f, "\nGroupContent::Group({:?})", g),
            GroupContent::Newline => write!(f, "\nGroupContent::Newline"),
            GroupContent::Text(bytes) => {
                write!(f, "\nGroupContent::Text(\"")?;
                for byte in bytes {
                    write!(f, "{}", *byte as char)?;
                }
                write!(f, "\")")
            }
        }
    }
}

named!(pub group_content<CompleteByteSlice, GroupContent>,
    alt!(group_content_control | group_content_group | group_content_newline | group_content_rtf_text)
);

named!(pub group_content_control<CompleteByteSlice, GroupContent>,
    map!(
        control,
        GroupContent::Control
    )
);

named!(pub group_content_group<CompleteByteSlice, GroupContent>,
    map!(
        group,
        GroupContent::Group
    )
);

named!(pub group_content_newline<CompleteByteSlice, GroupContent>,
    map!(
        crlf,
        |_| GroupContent::Newline
    )
);

// If the character is anything other than an opening brace ({), closing brace (}), backslash (\),
// or a CRLF (carriage return/line feed), the reader assumes that the character is plain text and
// writes the character to the current destination using the current formatting properties.
// See section "Conventions of an RTF Reader"
named!(pub group_content_rtf_text<CompleteByteSlice, GroupContent>,
    map!(
        rtf_text_raw,
        |text_bytes| GroupContent::Text(text_bytes.to_vec())
    )
);

#[derive(Debug, PartialEq)]
pub struct Group(Vec<GroupContent>);

named!(pub group<CompleteByteSlice, Group>,
    map!(
        group_raw,
        Group
    )
);

#[derive(Debug, PartialEq)]
pub struct Document(Vec<GroupContent>);

named!(pub document<CompleteByteSlice, Document >,
    map!(
        document_raw,
        Document
    )
);

#[cfg(test)]
mod tests {
    use super::*;

    use nom::types::CompleteByteSlice;

    named!(controls<CompleteByteSlice, Vec<Control> >, many1!(control));

    #[test]
    fn test_control_symbol() {
        let syms_str = CompleteByteSlice(br#"\*\.\+\~"#);
        let valid_syms = vec![
            Control::Symbol('*'),
            Control::Symbol('.'),
            Control::Symbol('+'),
            Control::Symbol('~'),
        ];
        let syms_after_parse = CompleteByteSlice(b"");
        let syms = controls(syms_str);
        assert_eq!(syms, Ok((syms_after_parse, valid_syms)));
    }

    named!(signed_ints<CompleteByteSlice, Vec<i32> >, separated_list_complete!(tag!(","), signed_int));

    #[test]
    fn test_signed_int() {
        let ints_str = CompleteByteSlice(br#"1,0,10,-15,-32765,16328,-73,-0"#);
        let valid_ints = vec![1, 0, 10, -15, -32765, 16328, -73, 0];
        let ints_after_parse = CompleteByteSlice(b"");
        let ints = signed_ints(ints_str);
        assert_eq!(ints, Ok((ints_after_parse, valid_ints)));
    }

    #[test]
    fn test_control_word() {
        let words_str = CompleteByteSlice(br#"\par\b0\b\uncle\foo-5\applepi314159"#);
        let valid_words = vec![
            Control::Word {
                name: "par".to_string(),
                arg: None,
            },
            Control::Word {
                name: "b".to_string(),
                arg: Some(0),
            },
            Control::Word {
                name: "b".to_string(),
                arg: None,
            },
            Control::Word {
                name: "uncle".to_string(),
                arg: None,
            },
            Control::Word {
                name: "foo".to_string(),
                arg: Some(-5),
            },
            Control::Word {
                name: "applepi".to_string(),
                arg: Some(314159),
            },
        ];
        let words_after_parse = CompleteByteSlice(b"");
        let words = controls(words_str);
        assert_eq!(words, Ok((words_after_parse, valid_words)));
    }

    #[test]
    fn test_control_bin() {
        let bins_str =
            CompleteByteSlice(b"\\bin5 ABC{}\\bin1 {\\bin0 \\bin0\\bin1  \\bin1\x01\\bin1 \x02");
        let valid_bins = vec![
            Control::Bin(b"ABC{}".to_vec()),
            Control::Bin(b"{".to_vec()),
            Control::Bin(b"".to_vec()),
            Control::Bin(b"".to_vec()),
            Control::Bin(b" ".to_vec()),
            Control::Bin(b"\x01".to_vec()),
            Control::Bin(b"\x02".to_vec()),
        ];
        let bins_after_parse = CompleteByteSlice(b"");
        let bins = controls(bins_str);
        assert_eq!(bins, Ok((bins_after_parse, valid_bins)));
    }

    #[test]
    fn test_control() {
        let controls_str = CompleteByteSlice(b"\\*\\bin5 ABC{}\\b\\bin1 {\\bin0 \\b0\\bin0\\bin1  \\supercalifragilistic31415\\bin1\x01\\bin1 \x02");
        let valid_controls = vec![
            Control::Symbol('*'),
            Control::Bin(b"ABC{}".to_vec()),
            Control::Word {
                name: "b".to_string(),
                arg: None,
            },
            Control::Bin(b"{".to_vec()),
            Control::Bin(b"".to_vec()),
            Control::Word {
                name: "b".to_string(),
                arg: Some(0),
            },
            Control::Bin(b"".to_vec()),
            Control::Bin(b" ".to_vec()),
            Control::Word {
                name: "supercalifragilistic".to_string(),
                arg: Some(31415),
            },
            Control::Bin(b"\x01".to_vec()),
            Control::Bin(b"\x02".to_vec()),
        ];
        let controls_after_parse = CompleteByteSlice(b"");
        let controls = controls(controls_str);
        assert_eq!(controls, Ok((controls_after_parse, valid_controls)));
    }

    named!(group_content_list<CompleteByteSlice, Vec<GroupContent> >, many1!(group_content));

    #[test]
    fn test_group_content() {
        // Have to be very careful here to insert crlf, regardless of host platform
        let group_content_str =
            CompleteByteSlice(b"\\b Hello World \\b0 \\par\r\nThis is a test {\\*\\nothing}");
        let valid_group_content = vec![
            GroupContent::Control(Control::Word {
                name: "b".to_string(),
                arg: None,
            }),
            GroupContent::Text(b"Hello World ".to_vec()),
            GroupContent::Control(Control::Word {
                name: "b".to_string(),
                arg: Some(0),
            }),
            GroupContent::Control(Control::Word {
                name: "par".to_string(),
                arg: None,
            }),
            GroupContent::Newline,
            GroupContent::Text(b"This is a test ".to_vec()),
            GroupContent::Group(Group(vec![
                GroupContent::Control(Control::Symbol('*')),
                GroupContent::Control(Control::Word {
                    name: "nothing".to_string(),
                    arg: None,
                }),
            ])),
        ];
        let group_content_after_parse = CompleteByteSlice(b"");
        let group_content = group_content_list(group_content_str);
        assert_eq!(
            group_content,
            Ok((group_content_after_parse, valid_group_content))
        );
    }

    #[test]
    fn test_sample_doc() {
        let test_bytes = CompleteByteSlice(include_bytes!("../tests/sample.rtf"));
        match document(test_bytes) {
            Ok((unparsed, _)) => assert_eq!(unparsed.len(), 0, "Unparsed data: {:?}", unparsed),
            Err(e) => panic!("Parsing error: {:?}", e),
        }
    }

    // The spec doc is interested because it has unmatched "{}" groups
    #[test]
    fn test_spec_doc() {
        let test_bytes = CompleteByteSlice(include_bytes!("../tests/RTF-Spec-1.7.rtf"));
        match document(test_bytes) {
            Ok((unparsed, _)) => assert_eq!(unparsed.len(), 0, "Unparsed data: {:?}", unparsed),
            Err(e) => panic!("Parsing error: {:?}", e),
        }
    }
}
