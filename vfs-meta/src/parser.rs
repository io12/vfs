use nom::{
    bytes::complete::{escaped_transform, is_a, is_not, take_until},
    character::complete::char,
    combinator::eof,
    multi::separated_list1,
    sequence::{separated_pair, terminated},
    IResult,
};

fn path(s: &[u8]) -> IResult<&[u8], Vec<u8>> {
    escaped_transform(is_not(b"|".as_slice()), '\\', is_a(b"\\|".as_slice()))(s)
}

fn meta_component(s: &[u8]) -> IResult<&[u8], (&[u8], Vec<u8>)> {
    separated_pair(take_until(b":".as_slice()), char(':'), path)(s)
}

fn meta_path(s: &[u8]) -> IResult<&[u8], Vec<(&[u8], Vec<u8>)>> {
    separated_list1(char('|'), meta_component)(s)
}

pub(crate) fn parse(s: &[u8]) -> IResult<&[u8], Vec<(&[u8], Vec<u8>)>> {
    terminated(meta_path, eof)(s)
}
