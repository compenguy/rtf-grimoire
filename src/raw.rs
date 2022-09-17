// RTF document format tokenizer
//
// Written according to the RTF Format Specification 1.9.1, which carries
// the following copyright notice:
//
//     Copyright (c) 2008 Microsoft Corporation.  All Rights reserved.
//

use nom;
use std;

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
    use super::*;

    named!(signed_ints<Input, Vec<i32> >, separated_list_complete!(tag!(","), signed_int));

    #[test]
    fn test_signed_int() {
        let ints_str = br#"1,0,10,-15,-32765,16328,-73,-0"#;
        let valid_ints = vec![1, 0, 10, -15, -32765, 16328, -73, 0];
        let ints_after_parse = Input(b"");
        let ints = signed_ints(Input(ints_str));
        assert_eq!(ints, Ok((ints_after_parse, valid_ints)));
    }
}
