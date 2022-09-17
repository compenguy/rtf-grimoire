// RTF document format tokenizer
//
// Written according to the RTF Format Specification 1.9.1, which carries
// the following copyright notice:
//
//     Copyright (c) 2008 Microsoft Corporation.  All Rights reserved.
//

use std;

use nom::branch::alt;
use nom::bytes::complete::{tag, take, take_while_m_n};
use nom::character::complete::{alpha1, char, crlf, digit1, none_of};
use nom::character::is_hex_digit;
use nom::combinator::{map, map_res, opt, recognize};
use nom::multi::many1;
use nom::sequence::{pair, preceded, tuple};
use nom::IResult;

// Helper function for converting &str into a signed int
#[allow(dead_code)]
fn str_to_int(s: &str, sign: Option<&str>) -> Result<i32, std::num::ParseIntError> {
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
pub fn signed_int_raw(input: &[u8]) -> IResult<&[u8], (Option<&str>, &str)> {
    pair(
        opt(map_res(tag("-"), std::str::from_utf8)),
        map_res(digit1, std::str::from_utf8),
    )(input)
}

// Helper function for parsing hexadecimal bytes
pub fn hexbyte_raw(input: &[u8]) -> IResult<&[u8], &str> {
    map_res(take_while_m_n(2, 2, is_hex_digit), std::str::from_utf8)(input)
}

pub fn hexbyte(input: &[u8]) -> IResult<&[u8], u8> {
    map_res(hexbyte_raw, hex_str_to_int)(input)
}

pub fn signed_int(input: &[u8]) -> IResult<&[u8], i32> {
    map_res(signed_int_raw, |(sign, value)| str_to_int(value, sign))(input)
}

pub fn control_symbol_raw(input: &[u8]) -> IResult<&[u8], std::primitive::char> {
    preceded(
        tag("\\"),
        none_of("'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"),
    )(input)
}

pub fn control_word_raw(input: &[u8]) -> IResult<&[u8], (&str, Option<i32>)> {
    let (input, (_, name, arg, _)) = tuple((
        tag("\\"),
        map_res(alpha1, std::str::from_utf8), // name
        opt(signed_int),                      // arg
        opt(tag(" ")),
    ))(input)?;
    Ok((input, (name, arg)))
}

// Sample.rtf's contents and rendering suggest that \'XX *doesn't* absorb a trailing space
// like other control words do

pub fn control_word_hexbyte_raw(input: &[u8]) -> IResult<&[u8], (&str, Option<i32>)> {
    let (input, (_, name, arg)) = tuple((
        tag("\\"),
        map_res(tag("'"), std::str::from_utf8), // name
        map(hexbyte, |x| Some(i32::from(x))),   // arg
    ))(input)?;
    Ok((input, (name, arg)))
}

pub fn control_bin_raw(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let (input, (_, len)) = tuple((
        tag("\\bin"),
        opt(map(pair(signed_int, opt(tag(" "))), |(s, _)| s)),
    ))(input)?;
    take(len.unwrap_or(0) as usize)(input)
}

// If the character is anything other than an opening brace ({), closing brace (}), backslash (\),
// or a CRLF (carriage return/line feed), the reader assumes that the character is plain text and
// writes the character to the current destination using the current formatting properties.
// See section "Conventions of an RTF Reader"

pub fn rtf_text_raw(input: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(many1(none_of("\\}{\r\n")))(input)
}

pub fn start_group_raw(input: &[u8]) -> IResult<&[u8], std::primitive::char> {
    char('{')(input)
}

pub fn end_group_raw(input: &[u8]) -> IResult<&[u8], std::primitive::char> {
    char('}')(input)
}

// Oddly enough, the copy of the RTF spec we have has at least one carriage return without its
// matching line feed, so it looks like we need to be more permissive about newlines than the spec
// says.
pub fn newline_raw(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((crlf, tag("\n"), tag("\r")))(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::error::ErrorKind;

    #[test]
    fn test_str_to_int() {
        let test_data: [(&str, Option<&str>, i32); 3] = [
            ("1234", Some("+"), 1234),  // Positive
            ("1234", Some("-"), -1234), // Negative
            ("1234", None, 1234),       // No sign
        ];
        for (input_str, input_sign, parsed_output) in test_data {
            assert_eq!(Ok(parsed_output), str_to_int(input_str, input_sign));
        }
    }

    #[test]
    #[should_panic(expected = "Unsupported integer sign char: p")]
    fn test_str_to_int_invalid_sign() {
        let input_str = "1234";
        let input_sign = Some("p");
        let parsed_output: i32 = 1234;
        assert_eq!(Ok(parsed_output), str_to_int(input_str, input_sign));
    }

    #[test]
    fn test_str_to_int_invalid_str() {
        let input_str = "BF";
        let input_sign = Some("+");
        let err_debug_str = "Err(ParseIntError { kind: InvalidDigit })";
        assert_eq!(
            format!("{:?}", str_to_int(input_str, input_sign)),
            err_debug_str
        );
    }

    #[test]
    // We dont really need to test the conversion from hex to int, but let check the results /
    // errors returned.
    fn test_hex_str_to_int() {
        let input = "0F";
        let parsed_output: u8 = 15;
        assert_eq!(Ok(parsed_output), hex_str_to_int(input));
    }

    #[test]
    fn test_hex_str_to_int_invalid() {
        let test_data = [
            ("uj", "Err(ParseIntError { kind: InvalidDigit })"), // Invalid input
            ("9D4B", "Err(ParseIntError { kind: PosOverflow })"), // Overflow
            ("", "Err(ParseIntError { kind: Empty })"),          // Empty input
        ];
        for (input, err_debug_str) in test_data {
            assert_eq!(format!("{:?}", hex_str_to_int(input)), err_debug_str);
        }
    }

    #[test]
    fn test_signed_int_raw() {
        let test_data: [(&[u8], &[u8], (Option<&str>, &str)); 2] = [
            (b"123ab", b"ab", (None, "123")),       // Positive
            (b"-123ab", b"ab", (Some("-"), "123")), // Negative
        ];
        for (input, remaining_input, parsed_output) in test_data {
            assert_eq!(Ok((remaining_input, parsed_output)), signed_int_raw(input));
        }
    }

    #[test]
    fn test_signed_int_raw_invalid() {
        let test_data: [(&[u8], &[u8], ErrorKind); 2] = [
            (b"ab123", b"ab123", ErrorKind::Digit),   // Positive
            (b"ab-123", b"ab-123", ErrorKind::Digit), // Negative
        ];
        for (input, remaining_input, error_kind) in test_data {
            assert_eq!(
                Err(nom::Err::Error(nom::error::Error {
                    input: remaining_input,
                    code: error_kind
                })),
                signed_int_raw(input)
            );
        }
    }

    #[test]
    fn test_hexbyte_raw() {
        let test_data: [(&[u8], &[u8], &str); 2] = [
            (b"0F4E", b"4E", "0F"), // Uppercase
            (b"4e0f", b"0f", "4e"), // Lowercase
        ];
        for (input, remaining_input, parsed_output) in test_data {
            assert_eq!(Ok((remaining_input, parsed_output)), hexbyte_raw(input));
        }
    }

    #[test]
    fn test_hexbyte_raw_invalid() {
        let test_data: [(&[u8], &[u8], ErrorKind); 2] = [
            (b"ge0f", b"ge0f", ErrorKind::TakeWhileMN), // First byte invalid
            (b"eg0f", b"eg0f", ErrorKind::TakeWhileMN), // Second byte invalid
        ];
        for (input, remaining_input, error_kind) in test_data {
            assert_eq!(
                Err(nom::Err::Error(nom::error::Error {
                    input: remaining_input,
                    code: error_kind
                })),
                hexbyte_raw(input)
            );
        }
    }

    #[test]
    fn test_hexbyte() {
        let input: &[u8] = b"4E2B";
        let remaining_input: &[u8] = b"2B";
        let parsed_output: u8 = 78;
        assert_eq!(Ok((remaining_input, parsed_output)), hexbyte(input));
    }

    #[test]
    fn test_hexbyte_invalid() {
        let input: &[u8] = b"4G2B";
        let remaining_input: &[u8] = b"4G2B";
        let error_kind = ErrorKind::TakeWhileMN;
        assert_eq!(
            Err(nom::Err::Error(nom::error::Error {
                input: remaining_input,
                code: error_kind
            })),
            hexbyte(input)
        );
    }

    #[test]
    fn test_signed_int_positive() {
        let test_data: [(&[u8], &[u8], i32); 2] = [
            (b"456a", b"a", 456),   // Positive
            (b"-920b", b"b", -920), // Negative
        ];
        for (input, remaining_input, parsed_output) in test_data {
            assert_eq!(Ok((remaining_input, parsed_output)), signed_int(input));
        }
    }

    #[test]
    fn test_signed_int_invalid() {
        let test_data: [(&[u8], &[u8], ErrorKind); 2] = [
            (b"2147483648b", b"2147483648b", ErrorKind::MapRes), // Overflow
            (b"a456", b"a456", ErrorKind::Digit),                // First char invalid
        ];
        for (input, remaining_input, error_kind) in test_data {
            assert_eq!(
                Err(nom::Err::Error(nom::error::Error {
                    input: remaining_input,
                    code: error_kind
                })),
                signed_int(input)
            );
        }
    }

    #[test]
    fn test_control_symbol_raw_valid() {
        let input: &[u8] = br#"\^t"#;
        let remaining_input: &[u8] = b"t";
        let parsed_output = '^';
        assert_eq!(
            Ok((remaining_input, parsed_output)),
            control_symbol_raw(input)
        );
    }

    #[test]
    fn test_control_symbol_raw_invalid() {
        let test_data: [(&[u8], &[u8], ErrorKind); 2] = [
            (b"hx", b"hx", ErrorKind::Tag),        // No starting slash
            (br#"\hx"#, b"hx", ErrorKind::NoneOf), // Excluded char
        ];
        for (input, remaining_input, error_kind) in test_data {
            assert_eq!(
                Err(nom::Err::Error(nom::error::Error {
                    input: remaining_input,
                    code: error_kind
                })),
                control_symbol_raw(input)
            );
        }
    }

    #[test]
    fn test_control_word_raw_valid() {
        let test_data: [(&[u8], &[u8], (&str, Option<i32>)); 6] = [
            (br#"\tag\tag67"#, br#"\tag67"#, ("tag", None)), // No int, no space
            (br#"\tag \tag67"#, br#"\tag67"#, ("tag", None)), // No int, optional space
            (br#"\tag45\tag67"#, br#"\tag67"#, ("tag", Some(45))), // Positive int, no space
            (br#"\tag45 \tag67"#, br#"\tag67"#, ("tag", Some(45))), // Positive int, optional space
            (br#"\tag-45\tag67"#, br#"\tag67"#, ("tag", Some(-45))), // Negative int, no space
            (br#"\tag-45 \tag67"#, br#"\tag67"#, ("tag", Some(-45))), // Negative int, optional space
        ];
        for (input, remaining_input, parsed_output) in test_data {
            assert_eq!(
                Ok((remaining_input, parsed_output)),
                control_word_raw(input)
            );
        }
    }

    #[test]
    fn test_control_word_raw_invalid() {
        let test_data: [(&[u8], &[u8], ErrorKind); 2] = [
            (br#"dfg-45 \tag67"#, br#"dfg-45 \tag67"#, ErrorKind::Tag), // No slash
            (br#"\*#~-45 \tag67"#, br#"*#~-45 \tag67"#, ErrorKind::Alpha), // Invalid chars in control word
        ];
        for (input, remaining_input, error_kind) in test_data {
            assert_eq!(
                Err(nom::Err::Error(nom::error::Error {
                    input: remaining_input,
                    code: error_kind
                })),
                control_word_raw(input)
            );
        }
    }

    #[test]
    fn test_control_word_hexbyte_raw() {
        let input: &[u8] = br#"\'9F4E"#;
        let remaining_input: &[u8] = b"4E";
        let parsed_output = ("'", Some(159i32));
        assert_eq!(
            Ok((remaining_input, parsed_output)),
            control_word_hexbyte_raw(input)
        );
    }

    #[test]
    fn test_control_word_hexbyte_raw_invalid() {
        let test_data: [(&[u8], &[u8], ErrorKind); 3] = [
            (b"'9F4E", b"'9F4E", ErrorKind::Tag),              // No slash
            (br#"\9F4E"#, b"9F4E", ErrorKind::Tag),            // No apostrophe
            (br#"\'R9F4E"#, b"R9F4E", ErrorKind::TakeWhileMN), // Invalid hex
        ];
        for (input, remaining_input, error_kind) in test_data {
            assert_eq!(
                Err(nom::Err::Error(nom::error::Error {
                    input: remaining_input,
                    code: error_kind
                })),
                control_word_hexbyte_raw(input)
            );
        }
    }

    #[test]
    fn test_control_bin_raw() {
        let test_data: [(&[u8], &[u8], &[u8]); 3] = [
            (br#"\bin2 ABCD"#, b"CD", b"AB"), // Optional length & space
            (br#"\bin2ABCD"#, b"CD", b"AB"),  // Optional length, no space
            (br#"\binABCD"#, b"ABCD", b""),   // No length, no space
        ];
        for (input, remaining_input, parsed_output) in test_data {
            assert_eq!(Ok((remaining_input, parsed_output)), control_bin_raw(input));
        }
    }

    #[test]
    fn test_control_bin_raw_invalid_tag() {
        let input: &[u8] = br#"\abcABCD"#;
        let remaining_input: &[u8] = br#"\abcABCD"#;
        let error_kind = ErrorKind::Tag;
        assert_eq!(
            Err(nom::Err::Error(nom::error::Error {
                input: remaining_input,
                code: error_kind
            })),
            control_bin_raw(input)
        );
    }

    #[test]
    fn test_rtf_text_raw() {
        let test_data: [(&[u8], &[u8], &[u8]); 5] = [
            (br#"123\abc"#, br#"\abc"#, b"123"), // Parse upto slash
            (b"123}abc", b"}abc", b"123"),       // Parse upto closing curly brace
            (b"123{abc", b"{abc", b"123"),       // Parse upto opening curly brace
            (b"123\rabc", b"\rabc", b"123"),     // CR
            (b"123\nabc", b"\nabc", b"123"),     // LF
        ];
        for (input, remaining_input, parsed_output) in test_data {
            assert_eq!(Ok((remaining_input, parsed_output)), rtf_text_raw(input));
        }
    }

    #[test]
    fn test_start_group_raw() {
        let input: &[u8] = b"{abc";
        let remaining_input: &[u8] = b"abc";
        let parsed_output = '{';
        assert_eq!(Ok((remaining_input, parsed_output)), start_group_raw(input));
    }

    #[test]
    fn test_start_group_raw_invalid() {
        let input: &[u8] = b"a{bc";
        let remaining_input: &[u8] = b"a{bc";
        let error_kind = ErrorKind::Char;
        assert_eq!(
            Err(nom::Err::Error(nom::error::Error {
                input: remaining_input,
                code: error_kind
            })),
            start_group_raw(input)
        );
    }

    #[test]
    fn test_end_group_raw() {
        let input: &[u8] = b"}abc";
        let remaining_input: &[u8] = b"abc";
        let parsed_output = '}';
        assert_eq!(Ok((remaining_input, parsed_output)), end_group_raw(input));
    }

    #[test]
    fn test_end_group_raw_invalid() {
        let input: &[u8] = b"a}bc";
        let remaining_input: &[u8] = b"a}bc";
        let error_kind = ErrorKind::Char;
        assert_eq!(
            Err(nom::Err::Error(nom::error::Error {
                input: remaining_input,
                code: error_kind
            })),
            end_group_raw(input)
        );
    }

    #[test]
    fn test_newline_raw() {
        let test_data: [(&[u8], &[u8], &[u8]); 3] = [
            (b"\r\nabc", br#"abc"#, b"\r\n"), // CLRF
            (b"\nabc", b"abc", b"\n"),        // \n
            (b"\rabc", b"abc", b"\r"),        // \r
        ];
        for (input, remaining_input, parsed_output) in test_data {
            assert_eq!(Ok((remaining_input, parsed_output)), newline_raw(input));
        }
    }

    #[test]
    fn test_newline_raw_invalid() {
        let test_data: [(&[u8], &[u8], ErrorKind); 3] = [
            (b"a\r\nbc", b"a\r\nbc", ErrorKind::Tag), //CLRF
            (b"a\nbc", b"a\nbc", ErrorKind::Tag),     // \n
            (b"a\rbc", b"a\rbc", ErrorKind::Tag),     // \r
        ];
        for (input, remaining_input, error_kind) in test_data {
            assert_eq!(
                Err(nom::Err::Error(nom::error::Error {
                    input: remaining_input,
                    code: error_kind
                })),
                newline_raw(input)
            );
        }
    }
}
