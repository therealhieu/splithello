#![allow(dead_code)]

use std::ops::Range;

use thiserror::Error;

use crate::domain::DomainName;

const TLS_RECORD_HEADER_BYTES: usize = 5;
const HANDSHAKE_HEADER_BYTES: usize = 4;
const MAX_PLAINTEXT_RECORD_BYTES: usize = 16_384;
const HANDSHAKE_CONTENT_TYPE: u8 = 22;
const CLIENT_HELLO_TYPE: u8 = 1;
const SERVER_NAME_EXTENSION: u16 = 0;
const HOST_NAME_TYPE: u8 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PassReason {
    NotTlsHandshake,
    NotClientHello,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ParseProgress<T> {
    NeedMore,
    Complete(T),
    PassThrough(PassReason),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParsedClientHello {
    record_version: [u8; 2],
    handshake: Vec<u8>,
    consumed_input: usize,
    sni: DomainName,
    sni_range: Range<usize>,
    trailing_record_version: [u8; 2],
    trailing_handshake: Vec<u8>,
}

impl ParsedClientHello {
    pub(crate) fn record_version(&self) -> [u8; 2] {
        self.record_version
    }

    pub(crate) fn handshake(&self) -> &[u8] {
        &self.handshake
    }

    pub(crate) fn consumed_input(&self) -> usize {
        self.consumed_input
    }

    pub(crate) fn sni(&self) -> &DomainName {
        &self.sni
    }

    pub(crate) fn sni_range(&self) -> Range<usize> {
        self.sni_range.clone()
    }

    pub(crate) fn trailing_record_version(&self) -> [u8; 2] {
        self.trailing_record_version
    }

    pub(crate) fn trailing_handshake(&self) -> &[u8] {
        &self.trailing_handshake
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum TlsError {
    #[error("TLS input exceeds the configured limit of {limit} bytes")]
    InputTooLarge { limit: usize },
    #[error("TLS input reached the configured limit of {limit} bytes before a decision")]
    InputLimitReached { limit: usize },
    #[error("TLS record version is invalid")]
    InvalidRecordVersion { version: [u8; 2] },
    #[error("TLS plaintext record length {length} exceeds 16384 bytes")]
    RecordTooLarge { length: usize },
    #[error("ClientHello handshake length {length} exceeds the configured limit")]
    HandshakeTooLarge { length: usize },
    #[error("unexpected TLS record content type {content_type} before ClientHello completion")]
    UnexpectedRecordType { content_type: u8 },
    #[error("ClientHello structure contains inconsistent lengths")]
    InvalidClientHelloLength,
    #[error("ClientHello cipher suite vector is invalid")]
    InvalidCipherSuites,
    #[error("ClientHello compression methods are invalid")]
    InvalidCompressionMethods,
    #[error("ClientHello contains more than one server-name extension or host name")]
    AmbiguousSni,
    #[error("ClientHello server name is missing")]
    MissingSni,
    #[error("ClientHello server name is invalid")]
    InvalidSni,
    #[error("ClientHello server name does not match the CONNECT host")]
    SniMismatch,
    #[error("ClientHello server name is too short for an internal record split")]
    SniTooShort,
    #[error("rewritten TLS record payload exceeds 16384 bytes")]
    OutputRecordTooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RewriteOutcome {
    NeedMore,
    Rewritten(Vec<u8>),
    PassThrough { prefix: Vec<u8>, reason: PassReason },
    Reject(TlsError),
}

pub(crate) fn rewrite_client_hello(
    input: &[u8],
    expected: &DomainName,
    limit: usize,
) -> RewriteOutcome {
    let parsed = match parse_client_hello(input, limit) {
        Ok(ParseProgress::NeedMore) => return RewriteOutcome::NeedMore,
        Ok(ParseProgress::PassThrough(reason)) => {
            return RewriteOutcome::PassThrough {
                prefix: input.to_vec(),
                reason,
            };
        }
        Ok(ParseProgress::Complete(parsed)) => parsed,
        Err(error) => return RewriteOutcome::Reject(error),
    };

    if parsed.sni() != expected {
        return RewriteOutcome::Reject(TlsError::SniMismatch);
    }

    match encode_split_records(&parsed, &input[parsed.consumed_input()..]) {
        Ok(output) => RewriteOutcome::Rewritten(output),
        Err(error) => RewriteOutcome::Reject(error),
    }
}

fn encode_split_records(
    parsed: &ParsedClientHello,
    opaque_suffix: &[u8],
) -> Result<Vec<u8>, TlsError> {
    let sni_range = parsed.sni_range();
    let sni_length = sni_range
        .end
        .checked_sub(sni_range.start)
        .ok_or(TlsError::InvalidClientHelloLength)?;
    if sni_length < 2 {
        return Err(TlsError::SniTooShort);
    }
    let split = checked_add(sni_range.start, sni_length / 2)?;
    if split <= sni_range.start || split >= sni_range.end {
        return Err(TlsError::SniTooShort);
    }

    let mut output = Vec::with_capacity(
        parsed
            .handshake()
            .len()
            .saturating_add(parsed.trailing_handshake().len())
            .saturating_add(opaque_suffix.len())
            .saturating_add(TLS_RECORD_HEADER_BYTES * 3),
    );
    append_handshake_record(
        &mut output,
        parsed.record_version(),
        &parsed.handshake()[..split],
    )?;
    append_handshake_record(
        &mut output,
        parsed.record_version(),
        &parsed.handshake()[split..],
    )?;
    if !parsed.trailing_handshake().is_empty() {
        append_handshake_record(
            &mut output,
            parsed.trailing_record_version(),
            parsed.trailing_handshake(),
        )?;
    }
    output.extend_from_slice(opaque_suffix);
    Ok(output)
}

fn append_handshake_record(
    output: &mut Vec<u8>,
    version: [u8; 2],
    payload: &[u8],
) -> Result<(), TlsError> {
    if payload.len() > MAX_PLAINTEXT_RECORD_BYTES {
        return Err(TlsError::OutputRecordTooLarge);
    }
    let length = u16::try_from(payload.len()).map_err(|_| TlsError::OutputRecordTooLarge)?;
    output.push(HANDSHAKE_CONTENT_TYPE);
    output.extend_from_slice(&version);
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(payload);
    Ok(())
}

pub(crate) fn parse_client_hello(
    input: &[u8],
    limit: usize,
) -> Result<ParseProgress<ParsedClientHello>, TlsError> {
    if input.len() > limit {
        return Err(TlsError::InputTooLarge { limit });
    }
    if input.is_empty() {
        return need_more(input.len(), limit);
    }
    if input[0] != HANDSHAKE_CONTENT_TYPE {
        return Ok(ParseProgress::PassThrough(PassReason::NotTlsHandshake));
    }

    let mut input_offset = 0_usize;
    let mut handshake = Vec::new();
    let mut expected_handshake_bytes = None;
    let mut record_version = None;

    loop {
        let header_end = checked_add(input_offset, TLS_RECORD_HEADER_BYTES)?;
        if header_end > input.len() {
            return need_more(input.len(), limit);
        }

        let content_type = input[input_offset];
        if content_type != HANDSHAKE_CONTENT_TYPE {
            if input_offset == 0 {
                return Ok(ParseProgress::PassThrough(PassReason::NotTlsHandshake));
            }
            return Err(TlsError::UnexpectedRecordType { content_type });
        }

        let version = [input[input_offset + 1], input[input_offset + 2]];
        if version[0] != 3 || !(1..=3).contains(&version[1]) {
            return Err(TlsError::InvalidRecordVersion { version });
        }
        record_version.get_or_insert(version);

        let record_length = usize::from(u16::from_be_bytes([
            input[input_offset + 3],
            input[input_offset + 4],
        ]));
        if record_length > MAX_PLAINTEXT_RECORD_BYTES {
            return Err(TlsError::RecordTooLarge {
                length: record_length,
            });
        }
        let record_end = checked_add(header_end, record_length)?;
        if record_end > input.len() {
            return need_more(input.len(), limit);
        }

        let payload = &input[header_end..record_end];
        let handshake_length_before_record = handshake.len();
        if expected_handshake_bytes.is_none() {
            let needed_header = HANDSHAKE_HEADER_BYTES.saturating_sub(handshake.len());
            let take = needed_header.min(payload.len());
            handshake.extend_from_slice(&payload[..take]);

            if handshake.len() == HANDSHAKE_HEADER_BYTES {
                if handshake[0] != CLIENT_HELLO_TYPE {
                    return Ok(ParseProgress::PassThrough(PassReason::NotClientHello));
                }
                let body_length = read_u24(&handshake[1..4]);
                let total_length = checked_add(HANDSHAKE_HEADER_BYTES, body_length)?;
                if total_length > limit {
                    return Err(TlsError::HandshakeTooLarge {
                        length: body_length,
                    });
                }
                expected_handshake_bytes = Some(total_length);
            }

            if take < payload.len()
                && let Some(expected) = expected_handshake_bytes
            {
                let remaining = expected.saturating_sub(handshake.len());
                let handshake_take = remaining.min(payload.len() - take);
                handshake.extend_from_slice(&payload[take..take + handshake_take]);
            }
        } else if let Some(expected) = expected_handshake_bytes {
            let remaining = expected.saturating_sub(handshake.len());
            let take = remaining.min(payload.len());
            handshake.extend_from_slice(&payload[..take]);
        }

        if let Some(expected) = expected_handshake_bytes
            && handshake.len() == expected
        {
            let handshake_bytes_from_record = expected - handshake_length_before_record;
            let trailing_start = header_end + handshake_bytes_from_record;
            let (sni, sni_range) = parse_sni(&handshake)?;
            return Ok(ParseProgress::Complete(ParsedClientHello {
                record_version: record_version.unwrap_or(version),
                handshake,
                consumed_input: record_end,
                sni,
                sni_range,
                trailing_record_version: version,
                trailing_handshake: input[trailing_start..record_end].to_vec(),
            }));
        }

        input_offset = record_end;
        if input_offset == input.len() {
            return need_more(input.len(), limit);
        }
    }
}

fn need_more<T>(input_length: usize, limit: usize) -> Result<ParseProgress<T>, TlsError> {
    if input_length >= limit {
        Err(TlsError::InputLimitReached { limit })
    } else {
        Ok(ParseProgress::NeedMore)
    }
}

fn checked_add(left: usize, right: usize) -> Result<usize, TlsError> {
    left.checked_add(right)
        .ok_or(TlsError::InvalidClientHelloLength)
}

fn read_u24(bytes: &[u8]) -> usize {
    (usize::from(bytes[0]) << 16) | (usize::from(bytes[1]) << 8) | usize::from(bytes[2])
}

fn parse_sni(handshake: &[u8]) -> Result<(DomainName, Range<usize>), TlsError> {
    let mut cursor = Cursor::new(handshake, HANDSHAKE_HEADER_BYTES);
    cursor.skip(2)?;
    cursor.skip(32)?;

    let session_id_length = usize::from(cursor.read_u8()?);
    cursor.skip(session_id_length)?;

    let cipher_suites_length = usize::from(cursor.read_u16()?);
    if cipher_suites_length < 2 || cipher_suites_length % 2 != 0 {
        return Err(TlsError::InvalidCipherSuites);
    }
    cursor.skip(cipher_suites_length)?;

    let compression_length = usize::from(cursor.read_u8()?);
    if compression_length == 0 {
        return Err(TlsError::InvalidCompressionMethods);
    }
    cursor.skip(compression_length)?;

    if cursor.is_at_end() {
        return Err(TlsError::MissingSni);
    }
    let extensions_length = usize::from(cursor.read_u16()?);
    let extensions_end = checked_add(cursor.position(), extensions_length)?;
    if extensions_end != handshake.len() {
        return Err(TlsError::InvalidClientHelloLength);
    }

    let mut found = None;
    while cursor.position() < extensions_end {
        let extension_type = cursor.read_u16()?;
        let extension_length = usize::from(cursor.read_u16()?);
        let extension_end = checked_add(cursor.position(), extension_length)?;
        if extension_end > extensions_end {
            return Err(TlsError::InvalidClientHelloLength);
        }
        if extension_type == SERVER_NAME_EXTENSION {
            if found.is_some() {
                return Err(TlsError::AmbiguousSni);
            }
            found = Some(parse_server_name_extension(&mut cursor, extension_end)?);
        } else {
            cursor.set_position(extension_end);
        }
    }

    found.ok_or(TlsError::MissingSni)
}

fn parse_server_name_extension(
    cursor: &mut Cursor<'_>,
    extension_end: usize,
) -> Result<(DomainName, Range<usize>), TlsError> {
    let list_length = usize::from(cursor.read_u16()?);
    let list_end = checked_add(cursor.position(), list_length)?;
    if list_end != extension_end {
        return Err(TlsError::InvalidClientHelloLength);
    }

    let mut host = None;
    while cursor.position() < list_end {
        let name_type = cursor.read_u8()?;
        let name_length = usize::from(cursor.read_u16()?);
        let name_start = cursor.position();
        let name_end = checked_add(name_start, name_length)?;
        if name_end > list_end {
            return Err(TlsError::InvalidClientHelloLength);
        }
        if name_type == HOST_NAME_TYPE {
            if host.is_some() {
                return Err(TlsError::AmbiguousSni);
            }
            let bytes = cursor.slice(name_start, name_end)?;
            let text = std::str::from_utf8(bytes).map_err(|_| TlsError::InvalidSni)?;
            let domain = DomainName::parse(text).map_err(|_| TlsError::InvalidSni)?;
            host = Some((domain, name_start..name_end));
        }
        cursor.set_position(name_end);
    }

    host.ok_or(TlsError::MissingSni)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], position: usize) -> Self {
        Self { bytes, position }
    }

    fn position(&self) -> usize {
        self.position
    }

    fn set_position(&mut self, position: usize) {
        self.position = position;
    }

    fn is_at_end(&self) -> bool {
        self.position == self.bytes.len()
    }

    fn read_u8(&mut self) -> Result<u8, TlsError> {
        let byte = *self
            .bytes
            .get(self.position)
            .ok_or(TlsError::InvalidClientHelloLength)?;
        self.position = checked_add(self.position, 1)?;
        Ok(byte)
    }

    fn read_u16(&mut self) -> Result<u16, TlsError> {
        let end = checked_add(self.position, 2)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(TlsError::InvalidClientHelloLength)?;
        self.position = end;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn skip(&mut self, length: usize) -> Result<(), TlsError> {
        let end = checked_add(self.position, length)?;
        if end > self.bytes.len() {
            return Err(TlsError::InvalidClientHelloLength);
        }
        self.position = end;
        Ok(())
    }

    fn slice(&self, start: usize, end: usize) -> Result<&'a [u8], TlsError> {
        self.bytes
            .get(start..end)
            .ok_or(TlsError::InvalidClientHelloLength)
    }
}

#[cfg(test)]
mod tests {
    use std::panic;

    use crate::domain::DomainName;

    use super::{
        ParseProgress, PassReason, RewriteOutcome, TlsError, parse_client_hello,
        rewrite_client_hello,
    };

    const LIMIT: usize = 4096;

    fn client_hello(sni: &str) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&[0x03, 0x03]);
        body.extend_from_slice(&[0x11; 32]);
        body.push(0);
        body.extend_from_slice(&2_u16.to_be_bytes());
        body.extend_from_slice(&[0x13, 0x01]);
        body.push(1);
        body.push(0);

        let mut server_name = Vec::new();
        server_name.push(0);
        server_name.extend_from_slice(&(sni.len() as u16).to_be_bytes());
        server_name.extend_from_slice(sni.as_bytes());

        let mut sni_extension = Vec::new();
        sni_extension.extend_from_slice(&(server_name.len() as u16).to_be_bytes());
        sni_extension.extend_from_slice(&server_name);

        let mut extensions = Vec::new();
        extensions.extend_from_slice(&0_u16.to_be_bytes());
        extensions.extend_from_slice(&(sni_extension.len() as u16).to_be_bytes());
        extensions.extend_from_slice(&sni_extension);
        body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
        body.extend_from_slice(&extensions);

        let mut handshake = vec![1];
        let length = body.len();
        handshake.extend_from_slice(&[
            ((length >> 16) & 0xff) as u8,
            ((length >> 8) & 0xff) as u8,
            (length & 0xff) as u8,
        ]);
        handshake.extend_from_slice(&body);
        handshake
    }

    fn records(handshake: &[u8], splits: &[usize]) -> Vec<u8> {
        let mut output = Vec::new();
        let mut start = 0;
        for end in splits
            .iter()
            .copied()
            .chain(std::iter::once(handshake.len()))
        {
            let payload = &handshake[start..end];
            output.push(22);
            output.extend_from_slice(&[0x03, 0x03]);
            output.extend_from_slice(&(payload.len() as u16).to_be_bytes());
            output.extend_from_slice(payload);
            start = end;
        }
        output
    }

    #[test]
    fn parses_equivalent_client_hello_across_one_and_many_records() {
        let handshake = client_hello("ReDdIt.CoM.");
        let one_record = records(&handshake, &[]);
        let many_records = records(&handshake, &[1, 3, 17, handshake.len() - 2]);

        for input in [&one_record, &many_records] {
            let ParseProgress::Complete(parsed) =
                parse_client_hello(input, LIMIT).expect("valid ClientHello")
            else {
                panic!("expected complete ClientHello");
            };
            assert_eq!(parsed.handshake(), handshake);
            assert_eq!(parsed.sni().as_str(), "reddit.com");
            assert_eq!(&parsed.handshake()[parsed.sni_range()], b"ReDdIt.CoM.");
            assert_eq!(parsed.consumed_input(), input.len());
            assert!(parsed.trailing_handshake().is_empty());
        }
    }

    #[test]
    fn reports_need_more_for_every_truncated_valid_prefix() {
        let input = records(&client_hello("reddit.com"), &[2, 19]);
        for end in 0..input.len() {
            assert_eq!(
                parse_client_hello(&input[..end], LIMIT),
                Ok(ParseProgress::NeedMore),
                "prefix length {end}"
            );
        }
    }

    #[test]
    fn preserves_consumed_extent_and_trailing_handshake_bytes() {
        let handshake = client_hello("reddit.com");
        let trailing = [2, 0, 0, 0];
        let mut payload = handshake.clone();
        payload.extend_from_slice(&trailing);
        let mut input = records(&payload, &[]);
        input.extend_from_slice(&[23, 0x03, 0x03, 0, 2, 0xaa, 0xbb]);

        let ParseProgress::Complete(parsed) =
            parse_client_hello(&input, LIMIT).expect("valid buffered prefix")
        else {
            panic!("expected complete ClientHello");
        };
        assert_eq!(parsed.handshake(), handshake);
        assert_eq!(parsed.trailing_handshake(), trailing);
        assert_eq!(parsed.consumed_input(), 5 + payload.len());
        assert_eq!(
            &input[parsed.consumed_input()..],
            &[23, 0x03, 0x03, 0, 2, 0xaa, 0xbb]
        );
    }

    #[test]
    fn distinguishes_safe_pass_through_from_rejection() {
        assert_eq!(
            parse_client_hello(b"GET /", LIMIT),
            Ok(ParseProgress::PassThrough(PassReason::NotTlsHandshake))
        );
        assert_eq!(
            parse_client_hello(&records(&[2, 0, 0, 0], &[]), LIMIT),
            Ok(ParseProgress::PassThrough(PassReason::NotClientHello))
        );

        let handshake = client_hello("reddit.com");
        let first = records(&handshake[..12], &[]);
        let mut interleaved = first;
        interleaved.extend_from_slice(&[23, 0x03, 0x03, 0, 1, 0]);
        assert_eq!(
            parse_client_hello(&interleaved, LIMIT),
            Err(TlsError::UnexpectedRecordType { content_type: 23 })
        );

        let zero_length_then_application_data = [22, 0x03, 0x03, 0, 0, 23, 0x03, 0x03, 0, 1, 0];
        assert_eq!(
            parse_client_hello(&zero_length_then_application_data, LIMIT),
            Err(TlsError::UnexpectedRecordType { content_type: 23 })
        );
    }

    #[test]
    fn rejects_malformed_lengths_extensions_and_sni() {
        let handshake = client_hello("reddit.com");
        for version in [[2, 3], [3, 0], [3, 4]] {
            let mut invalid_record_version = records(&handshake, &[]);
            invalid_record_version[1..3].copy_from_slice(&version);
            assert_eq!(
                parse_client_hello(&invalid_record_version, LIMIT),
                Err(TlsError::InvalidRecordVersion { version })
            );
        }

        let mut oversized_record = vec![22, 0x03, 0x03];
        oversized_record.extend_from_slice(&16_385_u16.to_be_bytes());
        assert_eq!(
            parse_client_hello(&oversized_record, LIMIT),
            Err(TlsError::RecordTooLarge { length: 16_385 })
        );

        let mut invalid_handshake_length = records(&handshake, &[]);
        invalid_handshake_length[6..9].copy_from_slice(&[0xff, 0xff, 0xff]);
        assert_eq!(
            parse_client_hello(&invalid_handshake_length, LIMIT),
            Err(TlsError::HandshakeTooLarge { length: 16_777_215 })
        );

        let missing_sni = client_hello_without_extensions();
        assert_eq!(
            parse_client_hello(&records(&missing_sni, &[]), LIMIT),
            Err(TlsError::MissingSni)
        );

        let mut malformed_extensions = handshake.clone();
        let extension_length_offset = 4 + 2 + 32 + 1 + 2 + 2 + 1 + 1;
        malformed_extensions[extension_length_offset..extension_length_offset + 2]
            .copy_from_slice(&u16::MAX.to_be_bytes());
        assert_eq!(
            parse_client_hello(&records(&malformed_extensions, &[]), LIMIT),
            Err(TlsError::InvalidClientHelloLength)
        );
    }

    #[test]
    fn enforces_input_limit_and_never_panics_for_arbitrary_bounded_bytes() {
        assert_eq!(
            parse_client_hello(&[0; 9], 8),
            Err(TlsError::InputTooLarge { limit: 8 })
        );
        assert_eq!(
            parse_client_hello(&[22, 0x03, 0x03, 0, 8, 1, 0, 0], 8),
            Err(TlsError::InputLimitReached { limit: 8 })
        );

        let expected = DomainName::parse("reddit.com").unwrap();
        let mut state = 0x9e37_79b9_u32;
        for length in 0..=128 {
            let mut bytes = vec![0; length];
            for byte in &mut bytes {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *byte = state as u8;
            }
            let parse_result = panic::catch_unwind(|| parse_client_hello(&bytes, 128));
            assert!(parse_result.is_ok(), "parser panicked for length {length}");
            let rewrite_result =
                panic::catch_unwind(|| rewrite_client_hello(&bytes, &expected, 128));
            assert!(
                rewrite_result.is_ok(),
                "rewriter panicked for length {length}"
            );
        }
    }

    #[test]
    fn rewrites_matching_sni_into_two_valid_records_with_identical_handshake() {
        let handshake = client_hello("reddit.com");
        let input = records(&handshake, &[2, 19]);
        let expected = DomainName::parse("ReDdIt.CoM.").unwrap();

        let RewriteOutcome::Rewritten(output) = rewrite_client_hello(&input, &expected, LIMIT)
        else {
            panic!("expected rewritten output");
        };
        let decoded = decode_records(&output);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].0, 22);
        assert_eq!(decoded[1].0, 22);
        assert_eq!(decoded[0].1, [0x03, 0x03]);
        assert_eq!(decoded[1].1, [0x03, 0x03]);

        let split = decoded[0].2.len();
        let parsed = match parse_client_hello(&input, LIMIT).unwrap() {
            ParseProgress::Complete(parsed) => parsed,
            _ => panic!("expected parsed ClientHello"),
        };
        assert!(split > parsed.sni_range().start);
        assert!(split < parsed.sni_range().end);

        let reassembled: Vec<u8> = decoded
            .iter()
            .flat_map(|(_, _, payload)| payload.iter().copied())
            .collect();
        assert_eq!(reassembled, handshake);
    }

    #[test]
    fn preserves_trailing_handshake_and_opaque_buffered_bytes() {
        let handshake = client_hello("reddit.com");
        let trailing_handshake = [2, 0, 0, 0];
        let mut completion_payload = handshake[10..].to_vec();
        completion_payload.extend_from_slice(&trailing_handshake);
        let mut input = records(&handshake[..10], &[]);
        input[2] = 1;
        input.extend_from_slice(&records(&completion_payload, &[]));
        let opaque = [23, 0x03, 0x03, 0, 3, 0xaa, 0xbb, 0xcc, 0xde, 0xad];
        input.extend_from_slice(&opaque);

        let expected = DomainName::parse("reddit.com").unwrap();
        let RewriteOutcome::Rewritten(output) = rewrite_client_hello(&input, &expected, LIMIT)
        else {
            panic!("expected rewritten output");
        };
        let decoded = decode_records(&output[..output.len() - opaque.len()]);
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].1, [0x03, 0x01]);
        assert_eq!(decoded[1].1, [0x03, 0x01]);
        assert_eq!(decoded[2].0, 22);
        assert_eq!(decoded[2].1, [0x03, 0x03]);
        assert_eq!(decoded[2].2, trailing_handshake);
        assert_eq!(&output[output.len() - opaque.len()..], &opaque);

        let reassembled: Vec<u8> = decoded[..2]
            .iter()
            .flat_map(|(_, _, payload)| payload.iter().copied())
            .collect();
        assert_eq!(reassembled, handshake);
    }

    #[test]
    fn returns_explicit_need_more_pass_through_and_rejection_outcomes() {
        let handshake = client_hello("reddit.com");
        let input = records(&handshake, &[]);
        let expected = DomainName::parse("reddit.com").unwrap();
        assert_eq!(
            rewrite_client_hello(&input[..input.len() - 1], &expected, LIMIT),
            RewriteOutcome::NeedMore
        );
        assert_eq!(
            rewrite_client_hello(b"plain text", &expected, LIMIT),
            RewriteOutcome::PassThrough {
                prefix: b"plain text".to_vec(),
                reason: PassReason::NotTlsHandshake,
            }
        );

        let wrong_host = DomainName::parse("medium.com").unwrap();
        assert_eq!(
            rewrite_client_hello(&input, &wrong_host, LIMIT),
            RewriteOutcome::Reject(TlsError::SniMismatch)
        );

        let short_handshake = client_hello("a");
        assert_eq!(
            rewrite_client_hello(
                &records(&short_handshake, &[]),
                &DomainName::parse("a").unwrap(),
                LIMIT
            ),
            RewriteOutcome::Reject(TlsError::SniTooShort)
        );
    }

    fn decode_records(mut bytes: &[u8]) -> Vec<(u8, [u8; 2], Vec<u8>)> {
        let mut decoded = Vec::new();
        while !bytes.is_empty() {
            assert!(bytes.len() >= 5);
            let length = usize::from(u16::from_be_bytes([bytes[3], bytes[4]]));
            assert!(length <= 16_384);
            assert!(bytes.len() >= 5 + length);
            decoded.push((
                bytes[0],
                [bytes[1], bytes[2]],
                bytes[5..5 + length].to_vec(),
            ));
            bytes = &bytes[5 + length..];
        }
        decoded
    }

    fn client_hello_without_extensions() -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&[0x03, 0x03]);
        body.extend_from_slice(&[0x22; 32]);
        body.push(0);
        body.extend_from_slice(&2_u16.to_be_bytes());
        body.extend_from_slice(&[0x13, 0x01]);
        body.push(1);
        body.push(0);

        let mut handshake = vec![1];
        let length = body.len();
        handshake.extend_from_slice(&[
            ((length >> 16) & 0xff) as u8,
            ((length >> 8) & 0xff) as u8,
            (length & 0xff) as u8,
        ]);
        handshake.extend_from_slice(&body);
        handshake
    }
}
