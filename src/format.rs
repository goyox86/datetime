use std::fmt::Display;
use std::num::Int;
use std::old_io::IoResult;
use std::str::CharIndices;

use local;
use local::{LocalDate, DatePiece};

use pad::{PadStr, Alignment};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Field<'a> {
    Literal(&'a str),

    Year(NumArguments),
    YearOfCentury(NumArguments),

    MonthName(bool, TextArguments),

    Day(NumArguments),
    WeekdayName(bool, TextArguments),
}

impl<'a> Field<'a> {
    fn format(self, when: LocalDate, w: &mut Vec<u8>) -> IoResult<()> {
        match self {
            Field::Literal(s)             => w.write_str(s),
            Field::Year(a)                => a.format(w, when.year()),
            Field::YearOfCentury(a)       => a.format(w, when.year_of_century()),
            Field::MonthName(true, a)     => a.format(w, long_month_name(when.month())),
            Field::MonthName(false, a)    => a.format(w, short_month_name(when.month())),
            Field::Day(a)                 => a.format(w, when.day()),
            Field::WeekdayName(true, a)   => a.format(w, long_day_name(when.weekday())),
            Field::WeekdayName(false, a)  => a.format(w, short_day_name(when.weekday())),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct DateFormat<'a> {
    pub fields: Vec<Field<'a>>,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum FormatError {
    InvalidChar { c: char, colon: bool, pos: usize },
    OpenCurlyBrace { open_pos: usize },
    CloseCurlyBrace { close_pos: usize },
    MissingField { open_pos: usize, close_pos: usize },
}

impl Copy for FormatError { }

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct Arguments {
    alignment: Option<Alignment>,
    width:     Option<usize>,
    pad_char:  Option<char>,
}

impl Arguments {
    pub fn empty() -> Arguments {
        Arguments {
            alignment: None,
            width:     None,
            pad_char:  None,
        }
    }

    fn format(self, w: &mut Vec<u8>, string: &str) -> IoResult<()> {
        let width     = self.width.unwrap_or(0);
        let pad_char  = self.pad_char.unwrap_or(' ');
        let alignment = self.alignment.unwrap_or(Alignment::Left);
        let s         = string.pad(width, pad_char, alignment, false);

        w.write_str(&s)
    }

    pub fn is_empty(&self) -> bool {
        self.alignment.is_none() && self.width.is_none() && self.pad_char.is_none()
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct TextArguments { args: Arguments }

impl TextArguments {
    fn format(self, w: &mut Vec<u8>, string: &str) -> IoResult<()> {
        self.args.format(w, string)
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct NumArguments { args: Arguments }

impl NumArguments {
    fn format<N: Int + Display>(self, w: &mut Vec<u8>, number: N) -> IoResult<()> {
        self.args.format(w, &number.to_string())
    }
}

impl<'a> DateFormat<'a> {
    pub fn format(self, when: LocalDate) -> String {
        let mut buf = Vec::<u8>::new();

        for bit in self.fields.into_iter() {
            // It's safe to just ignore the error when writing to an in-memory
            // Vec<u8> buffer. If it fails then you have bigger problems
            match bit.format(when, &mut buf) { _ => {} }
        }

        String::from_utf8(buf).unwrap()  // Assume UTF-8
    }

    pub fn parse(input: &'a str) -> Result<DateFormat<'a>, FormatError> {
        let mut parser = FormatParser::new(input);
        try! { parser.parse_format_string() };

        Ok(DateFormat { fields: parser.fields })
    }
}

struct FormatParser<'a> {
    iter:   CharIndices<'a>,
    fields: Vec<Field<'a>>,
    input:  &'a str,
    anchor: Option<usize>,
}

impl<'a> FormatParser<'a> {
    fn new(input: &'a str) -> FormatParser<'a> {
        FormatParser {
            iter:   input.char_indices(),
            fields: Vec::new(),
            input:  input,
            anchor: None,
        }
    }

    fn next(&mut self) -> Option<(usize, char)> {
        self.iter.next()
    }

    fn collect_up_to_anchor(&mut self, position: Option<usize>) {
        if let Some(pos) = self.anchor {
            self.anchor = None;
            let text = match position {
                Some(new_pos) => &self.input[pos..new_pos],
                None          => &self.input[pos..],
            };
            self.fields.push(Field::Literal(text));
        }
    }

    fn parse_format_string(&mut self) -> Result<(), FormatError> {
        loop {
            match self.next() {
                Some((new_pos, '{')) => {
                    self.collect_up_to_anchor(Some(new_pos)	);

                    let field = try! { self.parse_a_thing(new_pos) };
                    self.fields.push(field);
                },
                Some((new_pos, '}')) => {
                    if let Some((_, '}')) = self.next() {
                        self.collect_up_to_anchor(Some(new_pos));

                        let field = Field::Literal(&self.input[new_pos .. new_pos + 1]);
                        self.fields.push(field);
                    }
                    else {
                        return Err(FormatError::CloseCurlyBrace { close_pos: new_pos });
                    }
                },
                Some((pos, _)) => {
                    if self.anchor.is_none() {
                        self.anchor = Some(pos);
                    }
                }
                None => break,
            }
        }

        // Finally, collect any literal characters after the last date field
        // that haven't been turned into a Literal field yet.
        self.collect_up_to_anchor(None);
        Ok(())
    }

    // The Literal strings are just slices of the original formatting string,
    // which shares a lifetime with the formatter object, requiring fewer
    // allocations. The parser is clever and combines consecutive literal
    // strings.
    //
    // However, because they're slices, we can't transform them
    // to escape {{ and }} characters. So instead, up to three adjacent
    // Literal fields can be used to serve '{' or '}' characters, including
    // one that's the *first character* of the "{{" part. This means it can
    // still use slices.

    fn parse_a_thing(&mut self, open_pos: usize) -> Result<Field<'a>, FormatError> {
        let args = Arguments::empty();
        let mut bit = None;
        let mut close_pos;
        let mut first = true;

        loop {
            match self.next() {
                Some((pos, '{')) if first => return Ok(Field::Literal(&self.input[pos .. pos + 1])),
                Some((_, ':')) => {
                    let bitlet = match self.next() {
                        Some((_, 'Y')) => Field::Year(NumArguments { args: args }),
                        Some((_, 'y')) => Field::YearOfCentury(NumArguments { args: args }),
                        Some((_, 'M')) => Field::MonthName(true, TextArguments { args: args }),
                        Some((_, 'D')) => Field::Day(NumArguments { args: args }),
                        Some((_, 'E')) => Field::WeekdayName(true, TextArguments { args: args }),
                        Some((pos, c)) => return Err(FormatError::InvalidChar { c: c, colon: true, pos: pos }),
                        None => return Err(FormatError::OpenCurlyBrace { open_pos: open_pos }),
                    };

                    bit = Some(bitlet);
                },
                Some((pos, '}')) => { close_pos = pos; break; },
                Some((pos, c)) => return Err(FormatError::InvalidChar { c: c, colon: false, pos: pos }),
                None => return Err(FormatError::OpenCurlyBrace { open_pos: open_pos }),
            };

            first = false;
        }

        match bit {
            Some(b) => Ok(b),
            None    => Err(FormatError::MissingField { open_pos: open_pos, close_pos: close_pos }),
        }
    }
}

fn long_month_name(month: local::Month) -> &'static str {
    use local::Month::*;
    match month {
        January   => "January",    February  => "February",
        March     => "March",      April     => "April",
        May       => "May",        June      => "June",
        July      => "July",       August    => "August",
        September => "September",  October   => "October",
        November  => "November",   December  => "December",
    }
}

fn short_month_name(month: local::Month) -> &'static str {
    use local::Month::*;
    match month {
        January   => "Jan",  February  => "Feb",
        March     => "Mar",  April     => "Apr",
        May       => "May",  June      => "Jun",
        July      => "Jul",  August    => "Aug",
        September => "Sep",  October   => "Oct",
        November  => "Nov",  December  => "Dec",
    }
}

fn long_day_name(day: local::Weekday) -> &'static str {
    use local::Weekday::*;
    match day {
        Monday    => "Monday",     Tuesday   => "Tuesday",
        Wednesday => "Wednesday",  Thursday  => "Thursday",
        Friday    => "Friday",     Saturday  => "Saturday",
        Sunday    => "Sunday",

    }
}

fn short_day_name(day: local::Weekday) -> &'static str {
    use local::Weekday::*;
    match day {
        Monday    => "Mon",  Tuesday   => "Tue",
        Wednesday => "Wed",  Thursday  => "Thu",
        Friday    => "Fri",  Saturday  => "Sat",
        Sunday    => "Sun",

    }
}

#[cfg(test)]
mod test {
    pub use super::{DateFormat, FormatError, Field, Arguments, NumArguments, TextArguments};
    pub use super::Field::*;

    mod parse {
        use super::*;

        macro_rules! test {
            ($name: ident: $input: expr => $result: expr) => {
                #[test]
                fn $name() {
                    assert_eq!(DateFormat::parse($input), $result)
                }
            };
        }

        fn year<'a>() -> Field<'a> {
            Year(NumArguments { args: Arguments::empty() })
        }

        fn day<'a>() -> Field<'a> {
            Day(NumArguments { args: Arguments::empty() })
        }

        fn month<'a>(thing: bool) -> Field<'a> {
            MonthName(thing, TextArguments { args: Arguments::empty() })
        }

        test!(empty_string: ""                      => Ok(DateFormat { fields: vec![] }));
        test!(entirely_literal: "Date!"             => Ok(DateFormat { fields: vec![ Literal("Date!") ] }));
        test!(single_element: "{:Y}"                => Ok(DateFormat { fields: vec![ year() ] }));
        test!(two_long_years: "{:Y}{:Y}"            => Ok(DateFormat { fields: vec![ year(), year() ] }));
        test!(surrounded: "({:D})"                  => Ok(DateFormat { fields: vec![ Literal("("), day(), Literal(")") ] }));
        test!(a_bunch_of_elements: "{:Y}-{:M}-{:D}" => Ok(DateFormat { fields: vec![ year(), Literal("-"), month(true), Literal("-"), day() ] }));

        test!(missing_field: "{}"                              => Err(FormatError::MissingField { open_pos: 0, close_pos: 1 }));
        test!(invalid_char: "{7}"                              => Err(FormatError::InvalidChar { c: '7', colon: false, pos: 1 }));
        test!(invalid_char_after_colon: "{:7}"                 => Err(FormatError::InvalidChar { c: '7', colon: true, pos: 2 }));
        test!(open_curly_brace: "{"                            => Err(FormatError::OpenCurlyBrace { open_pos: 0 }));
        test!(mystery_close_brace: "}"                         => Err(FormatError::CloseCurlyBrace { close_pos: 0 }));
        test!(another_mystery_close_brace: "This is a test: }" => Err(FormatError::CloseCurlyBrace { close_pos: 16 }));

        test!(escaping_open: "{{"  => Ok(DateFormat { fields: vec![ Literal("{") ] }));
        test!(escaping_close: "}}" => Ok(DateFormat { fields: vec![ Literal("}") ] }));

        test!(escaping_middle: "The character {{ is my favourite!" => Ok(DateFormat { fields: vec![ Literal("The character "), Literal("{"), Literal(" is my favourite!") ] }));
        test!(escaping_middle_2: "It's way better than }}."        => Ok(DateFormat { fields: vec![ Literal("It's way better than "), Literal("}"), Literal(".") ] }));
    }
}
