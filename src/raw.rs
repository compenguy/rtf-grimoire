// RTF document format tokenizer
//
// Written according to the RTF Format Specification 1.9.1, which carries
// the following copyright notice:
//
//     Copyright (c) 2008 Microsoft Corporation.  All Rights reserved.
//

use std;
use nom;

use nom::crlf;
use nom::digit;
use nom::is_hex_digit;

use nom::types::CompleteByteSlice as Input;

// Helper function to convert from Input to &str
fn input_to_str(s: Input) -> Result<&str, std::str::Utf8Error> {
    std::str::from_utf8(s.0)
}

// Helper function for converting &str into a signed int
#[allow(dead_code)]
fn str_to_int<'a>(s: &'a str, sign: Option<&str>) -> Result<i32, std::num::ParseIntError> {
    s.parse::<i32>().map(|x| {
        x * sign.map_or(1, |x| match x {
            "-" => -1,
            "+" => 1,
            _ => panic!("Unsupported integer sign char: {}", x),
        })
    })
}

// Helper function for converting hex &str into a u8
#[allow(dead_code)]
fn hex_str_to_int(s: &str) -> Result<u8, std::num::ParseIntError> {
    u8::from_str_radix(s, 16)
}

// Helper function for parsing signed integers
named!(pub signed_int_raw<Input, (Option<&str>, &str)>,
    pair!(
        opt!(map_res!(tag!("-"), input_to_str)),
        map_res!(digit, input_to_str)
    )
);

// Helper function for parsing hexadecimal bytes
named!(pub hexbyte_raw<Input, &str>,
    map_res!(take_while_m_n!(2, 2, is_hex_digit), input_to_str)
);

named!(pub hexbyte<Input, u8>,
    map_res!(hexbyte_raw, hex_str_to_int)
);

named!(signed_int<Input, i32>,
    map_res!(
        signed_int_raw,
        |(sign, value)| { str_to_int(value, sign) }
    )
);

named!(pub control_symbol_raw<Input, char>,
    preceded!(tag!("\\"), none_of!("'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"))
);

named!(pub control_word_raw<Input, (&str, Option<i32>)>,
    do_parse!(
        tag!("\\") >>
        name: map_res!(nom::alpha, input_to_str) >>
        arg: opt!(signed_int) >>
        opt!(tag!(" ")) >>
        (name, arg)
    )
);

// Sample.rtf's contents and rendering suggest that \'XX *doesn't* absorb a trailing space
// like other control words do
named!(pub control_word_hexbyte_raw<Input, (&str, Option<i32>)>,
    do_parse!(
        tag!("\\") >>
        name: map_res!(tag!("'"), input_to_str) >>
        arg: map!(hexbyte, |x| Some(x as i32)) >>
        (name, arg)
    )
);

named!(pub control_bin_raw<Input, &[u8]>,
    do_parse!(
        tag!("\\bin") >>
        len: opt!(
            map!(
                pair!(
                    signed_int,
                    opt!(tag!(" "))
                ), |(s, _)| s
            )
        ) >>
        out: take!(len.unwrap_or(0)) >>
        (&out)
    )
);

// If the character is anything other than an opening brace ({), closing brace (}), backslash (\),
// or a CRLF (carriage return/line feed), the reader assumes that the character is plain text and
// writes the character to the current destination using the current formatting properties.
// See section "Conventions of an RTF Reader"
named!(pub rtf_text_raw<Input, &[u8]>,
    map!(
        recognize!(many0!(alt!(none_of!("\\}{\r\n")))),
        |i| i.0
    )
);

named!(pub start_group_raw<Input, char>,
    char!('{')
);

named!(pub end_group_raw<Input, char>,
    char!('}')
);

// Oddly enough, the copy of the RTF spec we have has at least one carriage return without its
// matching line feed, so it looks like we need to be more permissive about newlines than the spec
// says.
named!(pub newline_raw<Input, &[u8]>,
    map!(
        alt!(crlf | tag!("\n") | tag!("\r")),
        |i| i.0
    )
);

#[cfg(test)]
mod tests {
    use nom::ErrorKind;
    use super::*;

    #[test]
    fn test_hexbyte_raw_upper() {
        let input = Input(b"0F4E");
        let parsed_output = "0F";
        let remaining_input = Input(b"4E");
        assert_eq!(Ok((remaining_input, parsed_output)), hexbyte_raw(input));
    }

    #[test]
    fn test_hexbyte_raw_lower() {
        let input = Input(b"4e0f");
        let parsed_output = "4e";
        let remaining_input = Input(b"0f");
        assert_eq!(Ok((remaining_input, parsed_output)), hexbyte_raw(input));
    }

    #[test]
    fn test_hexbyte_raw_invalid_first() {
        let input = Input(b"ge0f");
        let remaining_input = Input(b"ge0f");
        let error_kind = ErrorKind::TakeWhileMN;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), hexbyte_raw(input));
    }

    #[test]
    fn test_hexbyte_raw_invalid_second() {
        let input = Input(b"eg0f");
        let remaining_input = Input(b"eg0f");
        let error_kind = ErrorKind::TakeWhileMN;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), hexbyte_raw(input));
    }

    #[test]
    fn test_hexbyte_valid() {
        let input = Input(b"4E2B");
        let remaining_input = Input(b"2B");
        let parsed_output = 78u8;
        assert_eq!(Ok((remaining_input, parsed_output)), hexbyte(input));
    }

    #[test]
    fn test_hexbyte_invalid() {
        let input = Input(b"4G2B");
        let remaining_input = Input(b"4G2B");
        let error_kind = ErrorKind::TakeWhileMN;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), hexbyte(input));
    }

    #[test]
    fn test_signed_int_positive() {
        let input = Input(b"456a");
        let remaining_input = Input(b"a");
        let parsed_output = 456i32;
        assert_eq!(Ok((remaining_input, parsed_output)), signed_int(input));
    }

    #[test]
    fn test_signed_int_negative() {
        let input = Input(b"-920b");
        let remaining_input = Input(b"b");
        let parsed_output = -920i32;
        assert_eq!(Ok((remaining_input, parsed_output)), signed_int(input));
    }

    #[test]
    fn test_signed_int_overflow() {
        let input = Input(b"2147483648b");
        let remaining_input = Input(b"2147483648b");
        let error_kind = ErrorKind::MapRes;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), signed_int(input));
    }

    #[test]
    fn test_control_symbol_raw_valid() {
        let input = Input(br#"\^t"#);
        let remaining_input = Input(b"t");
        let parsed_output = '^';
        assert_eq!(Ok((remaining_input, parsed_output)), control_symbol_raw(input));
    }

    #[test]
    fn test_control_symbol_raw_invalid_tag() {
        let input = Input(br#"hx"#);
        let remaining_input = Input(br#"hx"#);
        let error_kind = ErrorKind::Tag;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), control_symbol_raw(input));
    }

    #[test]
    fn test_control_symbol_raw_invalid_noneof() {
        let input = Input(br#"\hx"#);
        let remaining_input = Input(br#"hx"#);
        let error_kind = ErrorKind::NoneOf;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), control_symbol_raw(input));
    }

    #[test]
    fn test_control_word_raw_valid_no_int() {
        let input = Input(br#"\tag\tag67"#);
        let remaining_input = Input(br#"\tag67"#);
        let parsed_output = ("tag", None);
        assert_eq!(Ok((remaining_input, parsed_output)), control_word_raw(input));
    }

    #[test]
    fn test_control_word_raw_valid_no_int_space() {
        let input = Input(br#"\tag \tag67"#);
        let remaining_input = Input(br#"\tag67"#);
        let parsed_output = ("tag", None);
        assert_eq!(Ok((remaining_input, parsed_output)), control_word_raw(input));
    }

    #[test]
    fn test_control_word_raw_valid_positive_int() {
        let input = Input(br#"\tag45\tag67"#);
        let remaining_input = Input(br#"\tag67"#);
        let parsed_output = ("tag", Some(45i32));
        assert_eq!(Ok((remaining_input, parsed_output)), control_word_raw(input));
    }

    #[test]
    fn test_control_word_raw_valid_positive_int_space() {
        let input = Input(br#"\tag45 \tag67"#);
        let remaining_input = Input(br#"\tag67"#);
        let parsed_output = ("tag", Some(45i32));
        assert_eq!(Ok((remaining_input, parsed_output)), control_word_raw(input));
    }

    #[test]
    fn test_control_word_raw_valid_negative_int() {
        let input = Input(br#"\tag-45\tag67"#);
        let remaining_input = Input(br#"\tag67"#);
        let parsed_output = ("tag", Some(-45i32));
        assert_eq!(Ok((remaining_input, parsed_output)), control_word_raw(input));
    }

    #[test]
    fn test_control_word_raw_valid_negative_int_space() {
        let input = Input(br#"\tag-45 \tag67"#);
        let remaining_input = Input(br#"\tag67"#);
        let parsed_output = ("tag", Some(-45i32));
        assert_eq!(Ok((remaining_input, parsed_output)), control_word_raw(input));
    }

    #[test]
    fn test_control_word_raw_no_slash() {
        let input = Input(br#"dfg-45 \tag67"#);
        let remaining_input = Input(br#"dfg-45 \tag67"#);
        let error_kind = ErrorKind::Tag;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), control_word_raw(input));
    }

    #[test]
    fn test_control_word_raw_invalid_word() {
        let input = Input(br#"\*#~-45 \tag67"#);
        let remaining_input = Input(br#"*#~-45 \tag67"#);
        let error_kind = ErrorKind::Alpha;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), control_word_raw(input));
    }

    #[test]
    fn test_control_word_hexbyte_raw() {
        let input = Input(br#"\'9F4E"#);
        let remaining_input = Input(br#"4E"#);
        let parsed_output = ("'", Some(159i32));
        assert_eq!(Ok((remaining_input, parsed_output)), control_word_hexbyte_raw(input));
    }

    #[test]
    fn test_control_word_hexbyte_raw_no_slash() {
        let input = Input(br#"'9F4E"#);
        let remaining_input = Input(br#"'9F4E"#);
        let error_kind = ErrorKind::Tag;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), control_word_hexbyte_raw(input));
    }

    #[test]
    fn test_control_word_hexbyte_raw_no_apostrophy() {
        let input = Input(br#"\9F4E"#);
        let remaining_input = Input(br#"9F4E"#);
        let error_kind = ErrorKind::Tag;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), control_word_hexbyte_raw(input));
    }

    #[test]
    fn test_control_word_hexbyte_raw_invalid_hex() {
        let input = Input(br#"\'R9F4E"#);
        let remaining_input = Input(br#"R9F4E"#);
        let error_kind = ErrorKind::TakeWhileMN;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), control_word_hexbyte_raw(input));
    }
}
