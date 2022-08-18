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
    fn test_input_to_str() {
        let input = Input(b"123abc");
        let parsed_output = "123abc";
        assert_eq!(Ok(parsed_output), input_to_str(input));
    }

    #[test]
    fn test_input_to_str_invalid_utf8() {
        let input = Input(&[0x8F][..]);
        let err_debug_str = "Err(Utf8Error { valid_up_to: 0, error_len: Some(1) })";
        assert_eq!(err_debug_str, format!("{:?}", input_to_str(input)));
    }

    #[test]
    fn test_str_to_int() {
        for (input_str, input_sign, parsed_output) in [
            ("1234", Some("+"), 1234i32), // Positive
            ("1234", Some("-"), -1234i32), // Negative
            ("1234", None, 1234i32), // No sign
        ] {
            assert_eq!(Ok(parsed_output), str_to_int(input_str, input_sign));
        }
    }

    #[test]
    #[should_panic(expected = "Unsupported integer sign char: p")]
    fn test_str_to_int_invalid_sign() {
        for (input_str, input_sign, parsed_output) in [
            ("1234", Some("p"), 1234i32), // Invalid sign
        ] {
            assert_eq!(Ok(parsed_output), str_to_int(input_str, input_sign));
        }
    }

    #[test]
    fn test_str_to_int_invalid_str() {
        for (input_str, input_sign, err_debug_str) in [
            ("BF", Some("+"), "Err(ParseIntError { kind: InvalidDigit })"), // Invalid sign

        ] {
            assert_eq!(format!("{:?}", str_to_int(input_str, input_sign)), err_debug_str);
        }
    }

    #[test]
    // We dont really need to test the conversion from hex to int, but let check the results /
    // errors returned.
    fn test_hex_str_to_int() {
        let input = "0F";
        let parsed_output = 15u8;
        assert_eq!(Ok(parsed_output), hex_str_to_int(input));
    }

    #[test]
    fn test_hex_str_to_int_invalid() {
        for (input, err_debug_str) in [
            ("uj", "Err(ParseIntError { kind: InvalidDigit })"), // Invalid input
            ("9D4B", "Err(ParseIntError { kind: PosOverflow })"), // Overflow
            ("", "Err(ParseIntError { kind: Empty })"), // Empty input
    ] {
            assert_eq!(format!("{:?}", hex_str_to_int(input)), err_debug_str );
        }
    }

    #[test]
    fn test_signed_int_raw() {
        for (input, remaining_input, parsed_output) in [
            (Input(b"123ab"), Input(b"ab"), (None, "123")), // Positive
            (Input(b"-123ab"), Input(b"ab"), (Some("-"), "123")), // Negative
        ] {
            assert_eq!(Ok((remaining_input, parsed_output)), signed_int_raw(input));
        }
    }

    #[test]
    fn test_signed_int_raw_invalid() {
        for (input, remaining_input, error_kind) in [
            (Input(b"ab123"), Input(b"ab123"), ErrorKind::Digit), // Positive
            (Input(b"ab-123"), Input(b"ab-123"), ErrorKind::Digit) // Negative
        ] {
            assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), signed_int_raw(input));
        }
    }

    #[test]
    fn test_hexbyte_raw() {
        for (input, remaining_input, parsed_output) in [
            (Input(b"0F4E"), Input(b"4E"), "0F"), // Uppercase
            (Input(b"4e0f"), Input(b"0f"), "4e"), // Lowercase
        ] {
            assert_eq!(Ok((remaining_input, parsed_output)), hexbyte_raw(input));
        }
    }

    #[test]
    fn test_hexbyte_raw_invalid() {
        for (input, remaining_input, error_kind) in [
            (Input(b"ge0f"), Input(b"ge0f"), ErrorKind::TakeWhileMN), // First byte invalid
            (Input(b"eg0f"), Input(b"eg0f"), ErrorKind::TakeWhileMN) // Second byte invalid
            ]{
            assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), hexbyte_raw(input));
        }
    }

    #[test]
    fn test_hexbyte() {
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
        for (input, remaining_input, parsed_output) in [
            (Input(b"456a"), Input(b"a"), 456i32), // Positive
            (Input(b"-920b"), Input(b"b"), -920i32) // Negative
        ]{
            assert_eq!(Ok((remaining_input, parsed_output)), signed_int(input));
        }
    }

    #[test]
    fn test_signed_int_invalid() {
        for (input, remaining_input, error_kind) in [
            (Input(b"2147483648b"), Input(b"2147483648b"), ErrorKind::MapRes), // Overflow
            (Input(b"a456"), Input(b"a456"), ErrorKind::Digit) // First char invalid
        ] {
            assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), signed_int(input));
        }
    }

    #[test]
    fn test_control_symbol_raw_valid() {
        let input = Input(br#"\^t"#);
        let remaining_input = Input(b"t");
        let parsed_output = '^';
        assert_eq!(Ok((remaining_input, parsed_output)), control_symbol_raw(input));
    }

    #[test]
    fn test_control_symbol_raw_invalid() {
        for (input, remaining_input, error_kind) in [
            (Input(b"hx"), Input(b"hx"), ErrorKind::Tag), // No starting slash
            (Input(br#"\hx"#), Input(b"hx"), ErrorKind::NoneOf) // Excluded char
            ]{
            assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), control_symbol_raw(input));
        }
    }

    #[test]
    fn test_control_word_raw_valid() {
        for (input, remaining_input, parsed_output) in [
            (Input(br#"\tag\tag67"#), Input(br#"\tag67"#), ("tag", None)), // No int, no space
            (Input(br#"\tag \tag67"#), Input(br#"\tag67"#), ("tag", None)), // No int, optional space
            (Input(br#"\tag45\tag67"#), Input(br#"\tag67"#), ("tag", Some(45i32))), // Positive int, no space
            (Input(br#"\tag45 \tag67"#), Input(br#"\tag67"#), ("tag", Some(45i32))), // Positive int, optional space
            (Input(br#"\tag-45\tag67"#), Input(br#"\tag67"#), ("tag", Some(-45i32))), // Negative int, no space
            (Input(br#"\tag-45 \tag67"#), Input(br#"\tag67"#), ("tag", Some(-45i32))) // Negative int, optional space
            ] {
            assert_eq!(Ok((remaining_input, parsed_output)), control_word_raw(input));
        }
    }

    #[test]
    fn test_control_word_raw_invalid() {
        for (input, remaining_input, error_kind) in [
            (Input(br#"dfg-45 \tag67"#), Input(br#"dfg-45 \tag67"#), ErrorKind::Tag), // No slash
            (Input(br#"\*#~-45 \tag67"#), Input(br#"*#~-45 \tag67"#), ErrorKind::Alpha), // Invalid chars in control word
        ] {
            assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), control_word_raw(input));
        }
    }

    #[test]
    fn test_control_word_hexbyte_raw() {
        let input = Input(br#"\'9F4E"#);
        let remaining_input = Input(b"4E");
        let parsed_output = ("'", Some(159i32));
        assert_eq!(Ok((remaining_input, parsed_output)), control_word_hexbyte_raw(input));
    }

    #[test]
    fn test_control_word_hexbyte_raw_invalid() {
        for (input, remaining_input, error_kind) in [
            (Input(b"'9F4E"), Input(b"'9F4E"), ErrorKind::Tag), // No slash
            (Input(br#"\9F4E"#), Input(b"9F4E"), ErrorKind::Tag), // No apostrophe
            (Input(br#"\'R9F4E"#), Input(b"R9F4E"), ErrorKind::TakeWhileMN), // Invalid hex
        ] {
            assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), control_word_hexbyte_raw(input));
        }
    }

    #[test]
    fn test_control_bin_raw() {
        for (input, remaining_input, parsed_output) in [
            (Input(br#"\bin2 ABCD"#), Input(b"CD"), &b"AB"[..]), // Optional length & space
            (Input(br#"\bin2ABCD"#), Input(b"CD"), &b"AB"[..]), // Optional length, no space
            (Input(br#"\binABCD"#), Input(b"ABCD"), &b""[..]), // No length, no space
        ] {
            assert_eq!(Ok((remaining_input, parsed_output)), control_bin_raw(input));
        }
    }

    #[test]
    fn test_control_bin_raw_invalid_tag() {
        let input = Input(br#"\abcABCD"#);
        let remaining_input = Input(br#"\abcABCD"#);
        let error_kind = ErrorKind::Tag;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), control_bin_raw(input));
    }

    #[test]
    fn test_rtf_text_raw() {
        for (input, remaining_input, parsed_output) in [
            (Input(br#"123\abc"#), Input(br#"\abc"#), &b"123"[..]), // Parse upto slash
            (Input(b"123}abc"), Input(b"}abc"), &b"123"[..]), // Parse upto closing curly brace
            (Input(b"123{abc"), Input(b"{abc"), &b"123"[..]), // Parse upto opening curly brace
            (Input(b"123\rabc"), Input(b"\rabc"), &b"123"[..]), // CR
            (Input(b"123\nabc"), Input(b"\nabc"), &b"123"[..]), // LF
        ] {
            assert_eq!(Ok((remaining_input, parsed_output)), rtf_text_raw(input));
        }
    }

    #[test]
    fn test_start_group_raw() {
        let input = Input(b"{abc");
        let remaining_input = Input(b"abc");
        let parsed_output = '{';
        assert_eq!(Ok((remaining_input, parsed_output)), start_group_raw(input));
    }

    #[test]
    fn test_start_group_raw_invalid() {
        let input = Input(b"a{bc");
        let remaining_input = Input(b"a{bc");
        let error_kind= ErrorKind::Char;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), start_group_raw(input));
    }

    #[test]
    fn test_end_group_raw() {
        let input = Input(b"}abc");
        let remaining_input = Input(b"abc");
        let parsed_output = '}';
        assert_eq!(Ok((remaining_input, parsed_output)), end_group_raw(input));
    }

    #[test]
    fn test_end_group_raw_invalid() {
        let input = Input(b"a}bc");
        let remaining_input = Input(b"a}bc");
        let error_kind= ErrorKind::Char;
        assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), end_group_raw(input));
    }

    #[test]
    fn test_newline_raw() {
        for (input, remaining_input, parsed_output) in [
            (Input(b"\r\nabc"), Input(br#"abc"#), &b"\r\n"[..]), // CLRF
            (Input(b"\nabc"), Input(b"abc"), &b"\n"[..]), // \n
            (Input(b"\rabc"), Input(b"abc"), &b"\r"[..]), // \r
        ] {
            assert_eq!(Ok((remaining_input, parsed_output)), newline_raw(input));
        }
    }

    #[test]
    fn test_newline_raw_invalid() {
        for (input, remaining_input, error_kind) in [
            (Input(b"a\r\nbc"), Input(b"a\r\nbc"), ErrorKind::Alt), //CLRF
            (Input(b"a\nbc"), Input(b"a\nbc"), ErrorKind::Alt), // \n
            (Input(b"a\rbc"), Input(b"a\rbc"), ErrorKind::Alt), // \r
            ] {
            assert_eq!(Err(nom::Err::Error(nom::Context::Code(remaining_input, error_kind))), newline_raw(input));
        }
    }

}