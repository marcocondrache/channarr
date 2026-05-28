use std::{collections::HashMap, error::Error, fmt, str::FromStr};

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_till, take_while1},
    character::complete::{char, digit1, line_ending, not_line_ending, one_of, space0, space1},
    combinator::{all_consuming, map, map_res, opt, recognize, rest, verify},
    multi::{fold_many0, many0},
    sequence::{delimited, pair, preceded, terminated},
};

pub type M3uResult<T> = Result<T, ParseError>;

#[derive(Debug, Clone, PartialEq)]
pub struct Playlist {
    pub extended: bool,
    pub attributes: HashMap<String, AttributeValue>,
    pub title: Option<String>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub location: String,
    pub duration: Option<f64>,
    pub title: Option<String>,
    pub attributes: HashMap<String, AttributeValue>,
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    Quoted(String),
    Unquoted(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtInf {
    pub duration: f64,
    pub title: Option<String>,
    pub attributes: HashMap<String, AttributeValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: Option<usize>,
    pub kind: ParseErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    InvalidPlaylist,
    InvalidLine(String),
    UnexpectedHeader,
    MissingLocation { extinf_line: usize },
}

#[derive(Debug, Clone, PartialEq)]
enum PlaylistLine {
    Header(HashMap<String, AttributeValue>),
    PlaylistTitle(String),
    ExtInf(ExtInf),
    ExtGrp(String),
    Comment,
    Location(String),
}

#[derive(Debug, Clone, PartialEq)]
struct PendingExtInf {
    line: usize,
    info: ExtInf,
}

pub fn parse(input: &str) -> M3uResult<Playlist> {
    let (_, lines) = parse_lines(input).map_err(|_| ParseError {
        line: None,
        kind: ParseErrorKind::InvalidPlaylist,
    })?;

    let mut playlist = Playlist {
        extended: false,
        attributes: HashMap::new(),
        title: None,
        entries: Vec::new(),
    };
    let mut seen_non_blank = false;
    let mut current_group = None;
    let mut pending_extinf: Option<PendingExtInf> = None;

    for (index, raw_line) in lines.into_iter().enumerate() {
        let line_number = index + 1;
        let line = raw_line.strip_prefix('\u{feff}').unwrap_or(raw_line).trim();

        if line.is_empty() {
            continue;
        }

        let parsed = parse_playlist_line(line).map_err(|_| ParseError {
            line: Some(line_number),
            kind: ParseErrorKind::InvalidLine(line.to_string()),
        })?;

        match parsed {
            PlaylistLine::Header(attributes) => {
                if seen_non_blank {
                    return Err(ParseError {
                        line: Some(line_number),
                        kind: ParseErrorKind::UnexpectedHeader,
                    });
                }

                playlist.extended = true;
                playlist.attributes = attributes;
            }
            PlaylistLine::PlaylistTitle(title) => {
                playlist.title = Some(title);
            }
            PlaylistLine::ExtInf(info) => {
                if let Some(pending) = pending_extinf.take() {
                    return Err(ParseError {
                        line: Some(line_number),
                        kind: ParseErrorKind::MissingLocation {
                            extinf_line: pending.line,
                        },
                    });
                }

                pending_extinf = Some(PendingExtInf {
                    line: line_number,
                    info,
                });
            }
            PlaylistLine::ExtGrp(group) => {
                current_group = Some(group);
            }
            PlaylistLine::Comment => {}
            PlaylistLine::Location(location) => {
                let entry = Entry::new(location, pending_extinf.take(), current_group.clone());
                playlist.entries.push(entry);
            }
        }

        seen_non_blank = true;
    }

    if let Some(pending) = pending_extinf {
        return Err(ParseError {
            line: None,
            kind: ParseErrorKind::MissingLocation {
                extinf_line: pending.line,
            },
        });
    }

    Ok(playlist)
}

impl Entry {
    fn new(
        location: String,
        pending: Option<PendingExtInf>,
        current_group: Option<String>,
    ) -> Self {
        let Some(pending) = pending else {
            return Self {
                location,
                duration: None,
                title: None,
                attributes: HashMap::new(),
                group: current_group,
            };
        };

        let group = attribute(&pending.info.attributes, "group-title")
            .or_else(|| attribute(&pending.info.attributes, "tvg-group"))
            .map(ToOwned::to_owned)
            .or(current_group);

        Self {
            location,
            duration: Some(pending.info.duration),
            title: pending.info.title,
            attributes: pending.info.attributes,
            group,
        }
    }

    pub fn attribute(&self, key: &str) -> Option<&str> {
        attribute(&self.attributes, key)
    }

    pub fn tvg_id(&self) -> Option<&str> {
        self.attribute("tvg-id")
    }

    pub fn tvg_name(&self) -> Option<&str> {
        self.attribute("tvg-name")
    }

    pub fn tvg_logo(&self) -> Option<&str> {
        self.attribute("tvg-logo")
    }

    pub fn tvg_chno(&self) -> Option<&str> {
        self.attribute("tvg-chno")
    }

    pub fn channel_number(&self) -> Option<u32> {
        self.tvg_chno()?.parse().ok()
    }

    pub fn tvg_country(&self) -> Option<&str> {
        self.attribute("tvg-country")
    }

    pub fn tvg_language(&self) -> Option<&str> {
        self.attribute("tvg-language")
    }

    pub fn is_radio(&self) -> bool {
        self.attribute("radio")
            .map(|value| matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(false)
    }
}

impl AttributeValue {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Quoted(value) | Self::Unquoted(value) => value,
        }
    }

    pub fn is_quoted(&self) -> bool {
        matches!(self, Self::Quoted(_))
    }
}

impl fmt::Display for AttributeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Quoted(value) => write!(f, "\"{value}\""),
            Self::Unquoted(value) => f.write_str(value),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.line, &self.kind) {
            (Some(line), ParseErrorKind::InvalidLine(content)) => {
                write!(f, "invalid M3U line {line}: {content}")
            }
            (Some(line), ParseErrorKind::UnexpectedHeader) => {
                write!(
                    f,
                    "#EXTM3U header must be the first non-empty line, found on line {line}"
                )
            }
            (_, ParseErrorKind::MissingLocation { extinf_line }) => {
                write!(
                    f,
                    "#EXTINF on line {extinf_line} is missing a following location"
                )
            }
            (_, ParseErrorKind::InvalidPlaylist) => f.write_str("invalid M3U playlist"),
            (None, ParseErrorKind::InvalidLine(content)) => {
                write!(f, "invalid M3U line: {content}")
            }
            (None, ParseErrorKind::UnexpectedHeader) => {
                f.write_str("#EXTM3U header must be the first non-empty line")
            }
        }
    }
}

impl Error for ParseError {}

fn parse_lines(input: &str) -> IResult<&str, Vec<&str>> {
    all_consuming(many0(raw_line)).parse(input)
}

fn raw_line(input: &str) -> IResult<&str, &str> {
    alt((
        terminated(not_line_ending, line_ending),
        take_while1(|c| c != '\r' && c != '\n'),
    ))
    .parse(input)
}

fn parse_playlist_line(input: &str) -> M3uResult<PlaylistLine> {
    all_consuming(alt((
        header_line,
        playlist_title_line,
        extinf_line,
        extgrp_line,
        comment_line,
        location_line,
    )))
    .parse(input)
    .map(|(_, line)| line)
    .map_err(|_| ParseError {
        line: None,
        kind: ParseErrorKind::InvalidLine(input.to_string()),
    })
}

fn header_line(input: &str) -> IResult<&str, PlaylistLine> {
    let (input, _) = tag("#EXTM3U").parse(input)?;
    let (input, attributes) = spaced_attributes(input)?;
    let (input, _) = space0(input)?;

    Ok((input, PlaylistLine::Header(attributes)))
}

fn playlist_title_line(input: &str) -> IResult<&str, PlaylistLine> {
    map(preceded(tag("#PLAYLIST:"), rest), |title: &str| {
        PlaylistLine::PlaylistTitle(title.trim().to_string())
    })
    .parse(input)
}

fn extgrp_line(input: &str) -> IResult<&str, PlaylistLine> {
    map(preceded(tag("#EXTGRP:"), rest), |group: &str| {
        PlaylistLine::ExtGrp(group.trim().to_string())
    })
    .parse(input)
}

fn extinf_line(input: &str) -> IResult<&str, PlaylistLine> {
    map(
        preceded(tag("#EXTINF:"), extinf_payload),
        PlaylistLine::ExtInf,
    )
    .parse(input)
}

fn extinf_payload(input: &str) -> IResult<&str, ExtInf> {
    let (input, _) = space0(input)?;
    let (input, duration) = signed_float(input)?;
    let (input, attributes) = spaced_attributes(input)?;
    let (input, _) = space0(input)?;
    let (input, title) = opt(preceded(char(','), rest)).parse(input)?;

    let title = title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToOwned::to_owned);

    Ok((
        input,
        ExtInf {
            duration,
            title,
            attributes,
        },
    ))
}

fn comment_line(input: &str) -> IResult<&str, PlaylistLine> {
    map(preceded(char('#'), rest), |_| PlaylistLine::Comment).parse(input)
}

fn location_line(input: &str) -> IResult<&str, PlaylistLine> {
    map(
        verify(rest, |line: &str| {
            !line.trim().is_empty() && !line.starts_with('#')
        }),
        |location: &str| PlaylistLine::Location(location.trim().to_string()),
    )
    .parse(input)
}

fn spaced_attributes(input: &str) -> IResult<&str, HashMap<String, AttributeValue>> {
    fold_many0(
        preceded(space1, attribute_pair),
        HashMap::new,
        |mut attributes, (key, value)| {
            attributes.insert(key, value);
            attributes
        },
    )
    .parse(input)
}

fn attribute_pair(input: &str) -> IResult<&str, (String, AttributeValue)> {
    let (input, key) = map(attribute_key, ToOwned::to_owned).parse(input)?;
    let (input, _) = char('=').parse(input)?;
    let (input, value) = alt((quoted_value, unquoted_value)).parse(input)?;

    Ok((input, (key, value)))
}

fn attribute_key(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')).parse(input)
}

fn quoted_value(input: &str) -> IResult<&str, AttributeValue> {
    map(
        delimited(char('"'), take_till(|c| c == '"'), char('"')),
        |value: &str| AttributeValue::Quoted(value.to_string()),
    )
    .parse(input)
}

fn unquoted_value(input: &str) -> IResult<&str, AttributeValue> {
    map(
        take_while1(|c: char| !c.is_whitespace() && c != ','),
        |value: &str| AttributeValue::Unquoted(value.to_string()),
    )
    .parse(input)
}

fn signed_float(input: &str) -> IResult<&str, f64> {
    map_res(
        recognize((opt(one_of("+-")), digit1, opt(pair(char('.'), digit1)))),
        f64::from_str,
    )
    .parse(input)
}

fn attribute<'a>(attributes: &'a HashMap<String, AttributeValue>, key: &str) -> Option<&'a str> {
    attributes.get(key).map(AttributeValue::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_m3u_locations() {
        let playlist = parse(
            r#"# plain comment
C:\Music\Heavysets.mp3
relative/song.mp3
https://example.com/radio
"#,
        )
        .unwrap();

        assert!(!playlist.extended);
        assert_eq!(playlist.entries.len(), 3);
        assert_eq!(playlist.entries[0].location, r"C:\Music\Heavysets.mp3");
        assert_eq!(playlist.entries[1].location, "relative/song.mp3");
        assert_eq!(playlist.entries[2].location, "https://example.com/radio");
        assert_eq!(playlist.entries[0].duration, None);
    }

    #[test]
    fn parses_extended_m3u_entries() {
        let playlist = parse(
            "#EXTM3U\r\n#PLAYLIST:Music TV\r\n#EXTINF:419,Alice in Chains - Rotten Apple\r\nAlice in Chains_Jar of Flies_01_Rotten Apple.mp3\r\n",
        )
        .unwrap();

        assert!(playlist.extended);
        assert_eq!(playlist.title.as_deref(), Some("Music TV"));
        assert_eq!(playlist.entries.len(), 1);

        let entry = &playlist.entries[0];
        assert_eq!(entry.duration, Some(419.0));
        assert_eq!(
            entry.title.as_deref(),
            Some("Alice in Chains - Rotten Apple")
        );
        assert_eq!(
            entry.location,
            "Alice in Chains_Jar of Flies_01_Rotten Apple.mp3"
        );
    }

    #[test]
    fn parses_iptv_extinf_attributes() {
        let playlist = parse(
            r#"#EXTM3U x-tvg-url="https://example.com/epg.xml"
#EXTINF:-1 tvg-id="123" tvg-name="Channel Name" tvg-logo="http://example.com/logo.png" group-title="Examples" tvg-chno="12" tvg-country="NZ" tvg-language="English" radio=true, Channel Name
rtsp://example.com/stream
"#,
        )
        .unwrap();

        assert_eq!(
            playlist
                .attributes
                .get("x-tvg-url")
                .map(AttributeValue::as_str),
            Some("https://example.com/epg.xml")
        );

        let entry = &playlist.entries[0];
        assert_eq!(entry.duration, Some(-1.0));
        assert_eq!(entry.title.as_deref(), Some("Channel Name"));
        assert_eq!(entry.location, "rtsp://example.com/stream");
        assert_eq!(entry.group.as_deref(), Some("Examples"));
        assert_eq!(entry.tvg_id(), Some("123"));
        assert_eq!(entry.tvg_name(), Some("Channel Name"));
        assert_eq!(entry.tvg_logo(), Some("http://example.com/logo.png"));
        assert_eq!(entry.tvg_chno(), Some("12"));
        assert_eq!(entry.channel_number(), Some(12));
        assert_eq!(entry.tvg_country(), Some("NZ"));
        assert_eq!(entry.tvg_language(), Some("English"));
        assert!(entry.is_radio());
        assert_eq!(
            entry.attributes.get("radio"),
            Some(&AttributeValue::Unquoted("true".to_string()))
        );
    }

    #[test]
    fn applies_extgrp_to_following_locations() {
        let playlist = parse(
            r#"#EXTM3U
#EXTGRP:Foreign Channels
#EXTINF:-1,Channel
http://example.com/channel
http://example.com/backup
"#,
        )
        .unwrap();

        assert_eq!(playlist.entries.len(), 2);
        assert_eq!(
            playlist.entries[0].group.as_deref(),
            Some("Foreign Channels")
        );
        assert_eq!(
            playlist.entries[1].group.as_deref(),
            Some("Foreign Channels")
        );
    }

    #[test]
    fn iptv_group_attribute_overrides_extgrp() {
        let playlist = parse(
            r#"#EXTM3U
#EXTGRP:Fallback Group
#EXTINF:-1 group-title="News",Channel
http://example.com/news
"#,
        )
        .unwrap();

        assert_eq!(playlist.entries[0].group.as_deref(), Some("News"));
    }

    #[test]
    fn errors_when_extinf_has_no_location() {
        let error = parse("#EXTM3U\n#EXTINF:-1,Missing\n").unwrap_err();

        assert_eq!(
            error.kind,
            ParseErrorKind::MissingLocation { extinf_line: 2 }
        );
    }

    #[test]
    fn errors_when_header_is_not_first_non_empty_line() {
        let error = parse("# comment\n#EXTM3U\nhttp://example.com").unwrap_err();

        assert_eq!(error.line, Some(2));
        assert_eq!(error.kind, ParseErrorKind::UnexpectedHeader);
    }
}
