// RTF document format tokenizer
//
// Written according to the RTF Format Specification 1.9.1, which carries
// the following copyright notice:
//
//     Copyright (c) 2008 Microsoft Corporation.  All Rights reserved.
//

use nom::types::CompleteByteSlice;
use nom::digit;

use crate::tokenizer::GroupContent;
use crate::tokenizer::group_content;

// Helper function for converting nom's CompleteByteSlice input into &str
#[allow(dead_code)]
fn complete_byte_slice_to_str(s: CompleteByteSlice) -> Result<&str, std::str::Utf8Error> {
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

// Helper function for parsing signed integers
named!(pub signed_int_raw<CompleteByteSlice, (Option<&str>, &str)>,
    pair!(
        opt!(map_res!(tag!("-"), complete_byte_slice_to_str)),
        map_res!(digit, complete_byte_slice_to_str)
    )
);

named!(pub signed_int<CompleteByteSlice, i32>,
    map_res!(
        signed_int_raw,
        |(sign, value)| { str_to_int(value, sign) }
    )
);

named!(pub control_symbol_raw<CompleteByteSlice, char>,
    preceded!(tag!("\\"), none_of!("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"))
);


named!(pub control_word_raw<CompleteByteSlice, (&str, Option<i32>)>,
    do_parse!(
        tag!("\\") >>
        name: map_res!(nom::alpha, complete_byte_slice_to_str) >>
        arg: opt!(signed_int) >>
        opt!(tag!(" ")) >>
        (name, arg)
    )
);

named!(pub control_bin_raw<CompleteByteSlice, CompleteByteSlice>,
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
        (out)
    )
);

// If the character is anything other than an opening brace ({), closing brace (}), backslash (\),
// or a CRLF (carriage return/line feed), the reader assumes that the character is plain text and
// writes the character to the current destination using the current formatting properties.
// See section "Conventions of an RTF Reader"
named!(pub rtf_text_raw<CompleteByteSlice, CompleteByteSlice>,
    recognize!(many0!(alt!(none_of!("\\}{\r\n"))))
);

named!(pub group_raw<CompleteByteSlice, Vec<GroupContent> >,
    alt!(
        delimited!(
            tag!("{"),
            many0!(group_content),
            tag!("}")
        ) |
        preceded!(
            tag!("{"),
            many1!(group_content)
        )
    )
);

named!(pub document_raw<CompleteByteSlice, Vec<GroupContent> >,
    ws!(group_raw)
);

#[cfg(test)]
mod tests {
    use super::*;

    use nom::types::CompleteByteSlice;

    named!(signed_ints<CompleteByteSlice, Vec<i32> >, separated_list_complete!(tag!(","), signed_int));

    #[test]
    fn test_signed_int() {
        let ints_str = CompleteByteSlice(br#"1,0,10,-15,-32765,16328,-73,-0"#);
        let valid_ints = vec![1, 0, 10, -15, -32765, 16328, -73, 0];
        let ints_after_parse = CompleteByteSlice(b"");
        let ints = signed_ints(ints_str);
        assert_eq!(ints, Ok((ints_after_parse, valid_ints)));
    }
}
