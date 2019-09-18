use std::collections::HashMap;

use nom::branch::alt;
use nom::bytes::streaming::{tag, take_until};
use nom::character::complete::space1;
use nom::character::streaming::{alpha1, alphanumeric1, digit1};
use nom::combinator::{map, recognize};
use nom::IResult;
use nom::multi::many0;
use nom::sequence::tuple;

#[derive(Debug)]
pub struct Response {
    pub status_code: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

pub(crate) fn status_line(input: &str) -> IResult<&str, &str> {
    map(tuple((
        tag("HTTP/1.1 "),
        digit1,
        tag(" "),
        take_until("\r\n")
    )), |(_, code, _, _)| { code })(input)
}

pub(crate) fn header_entry(input: &str) -> IResult<&str, (&str, &str)> {
    map(tuple((
        recognize(many0(alt((tag("-"), tag("_"), alpha1, alphanumeric1)))),
        tag(":"),
        take_until("\r\n")
    )), |(key, _, value): (&str, _, &str)| (key.trim(), value.trim()))(input)
}

pub(crate) fn headers(input: &str) -> IResult<&str, HashMap<String, String>> {
    map(many0(tuple((header_entry, tag("\r\n")))),
        |header_vec: Vec<((&str, &str), _)>| {
            let mut result = HashMap::new();
            for ((key, value), _) in header_vec {
                result.insert(key.to_owned(), value.to_owned());
            }
            result
        })(input)
}

pub(crate) fn parse(input: &str) -> Response {
    let (rest, status_code) = status_line(input).unwrap();
    let (rest, headers_map) = headers(rest.trim_start()).unwrap();
    let body = rest.trim_start().to_string();
    Response {
        status_code: status_code.to_string(),
        headers: headers_map,
        body,
    }
}
