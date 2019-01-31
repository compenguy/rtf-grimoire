#[macro_use]
extern crate nom;

use nom::types::CompleteByteSlice;
use nom::{digit};

#[derive(Debug, PartialEq)]
enum Control {
    Symbol(char),
    Word {
        name: String,
        arg: Option<i32>,
    },
    Bin(Vec<u8>),
}

fn complete_byte_slice_to_str<'a>(s: CompleteByteSlice<'a>) -> Result<&'a str, std::str::Utf8Error> {
    std::str::from_utf8(s.0)
}

fn str_to_int<'a>(s: &'a str, sign: Option<char>) -> Result<i32, std::num::ParseIntError> {
    s.parse::<i32>()
        .map(|x| x * sign.map_or(1, |x| match x {
            '-' => -1,
            '+' => 1,
            _   => panic!("Unsupported integer sign char: {}", x)
        }))
}

/*
named!(control<&[u8], Control>,
    alt!(
        control_symbol |
        control_bin |
        control_word
    )
);
*/

named!(control_symbol<CompleteByteSlice, Control>,
    map!(
        pair!(char!('\\'), none_of!("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ")),
        |(_, x)| Control::Symbol(x)
    )
);

/*
named!(control_word<CompleteByteSlice, Control>,
    do_parse!(
        char!('\\')
        >> name: map_res!(nom::alpha, complete_byte_slice_to_str)
        >> arg: opt!(map_res!(signed_int))
        >> Control::Word { name: name, arg: arg }
    )
);
*/

named!(maybe_signed_int<CompleteByteSlice, &str>,
    /* TODO: switch to function that calls signed int, and parses it out to an i32 */
    map_res!(recognize!(signed_int), complete_byte_slice_to_str)
);

named!(signed_int<CompleteByteSlice, (Option<&str>, &str)>,
    pair!(
        opt!(map_res!(alt!(tag!("+") | tag!("-")), complete_byte_slice_to_str)),
        map_res!(digit, complete_byte_slice_to_str)
    )
);

/*
named!(control_bin<CompleteByteSlice, Control>,
    map!(
        length_bytes!(
            map!(control_word,
                |(_, word)| word.arg.unwrap_or(0)
            )
        ),
        |(_, data)| Control::Bin(data.collect())
    )
);
*/

#[cfg(test)]
mod tests {
    use super::*;

    use nom::types::CompleteByteSlice;

    named!(control_symbols<CompleteByteSlice, Vec<Control> >, many1!(control_symbol));

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
        let syms = control_symbols(syms_str);
        assert_eq!(syms, Ok((syms_after_parse,valid_syms)));
    }

    named!(signed_ints<CompleteByteSlice, Vec<&str> >, separated_list_complete!(tag!(","), maybe_signed_int));

    #[test]
    fn test_signed_int() {
        let ints_str = CompleteByteSlice(br#"1,0,10,-15,-32765,16328,-73,-0"#);
        let valid_ints = vec![ "1", "0", "10", "-15", "-32765", "16328", "-73", "-0" ];
        let ints_after_parse = CompleteByteSlice(b"");
        let ints = signed_ints(ints_str);
        assert_eq!(ints, Ok((ints_after_parse,valid_ints)));
    }
    /*
    named!(control_words<CompleteByteSlice, Vec<Control> >, many1!(control_word));

    #[test]
    fn test_control_word() {
        let words_str = CompleteByteSlice(br#"\par\b0\b\uncle\foo-5\applepi314159"#);
        let valid_words = vec![
            Control::Word { name: "par", arg: None },
            Control::Word { name: "b", arg: Some(0) },
            Control::Word { name: "b", arg: None },
            Control::Word { name: "uncle", arg: None },
            Control::Word { name: "foo", arg: Some(-5) },
            Control::Word { name: "applepi", arg: Some(314159) },
        ];
        let words_after_parse = CompleteByteSlice(b"");
        let words = control_symbols(words_str);
        assert_eq!(words, Ok((words_after_parse,valid_words)));
    }
    */
}
