//! Deterministic mutation/property regressions; no IO, import, grant or execution.
use super::*;
use crate::packages::tools::media::{
    parse_inspection, ProgressDecoder, MAX_DURATION_MS, MAX_MEDIA_BYTES,
};
use crate::protocol::{decode_frame, Call, ErrorCode, Request, MAX_FRAME_BYTES};

struct Corpus(u64);
impl Corpus {
    fn next(&mut self) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 as usize
    }
    fn mutate(&mut self, seed: &[u8]) -> Vec<u8> {
        let mut bytes = seed.to_vec();
        match self.next() % 4 {
            0 => {
                let index = self.next() % bytes.len();
                bytes[index] ^= 1 << (self.next() % 8);
            }
            1 => {
                bytes.truncate(self.next() % bytes.len());
            }
            2 => {
                let index = self.next() % (bytes.len() + 1);
                bytes.insert(index, self.next() as u8);
            }
            _ => {
                bytes.extend_from_slice(b"\0{}");
            }
        }
        bytes
    }
}

#[test]
fn security_property_wire_roundtrips_and_rejects_ambiguous_or_oversized_json() {
    let seed = br#"{"version":1,"requestId":"property","session":null,"call":{"method":"runtime.status"}}"#;
    assert!(matches!(
        decode_frame::<Request>(seed).unwrap().call,
        Call::Status
    ));
    for value in [
        br#"{"key":1,"key":2}"#.as_slice(),
        br#"{"input":{"scope":"read","scope":"write"}}"#.as_slice(),
        br#"{"input":{"name":1,"na\u006de":2}}"#.as_slice(),
        b"[] trailing",
        b"\xff",
        b"",
    ] {
        assert!(decode_frame::<Value>(value).is_err());
    }
    assert!(matches!(
        decode_frame::<Request>(&vec![b' '; MAX_FRAME_BYTES + 1]),
        Err(ErrorCode::FrameTooLarge)
    ));
    let deep = format!("{}0{}", "[".repeat(256), "]".repeat(256));
    assert!(decode_frame::<Value>(deep.as_bytes()).is_err());
    let mut corpus = Corpus(0x8a31_32b7_c409_0701);
    let mut refused = 0;
    for _ in 0..4096 {
        let bytes = corpus.mutate(seed);
        match decode_frame::<Request>(&bytes) {
            Ok(request) => {
                let encoded = serde_json::to_vec(&request).unwrap();
                let roundtrip = decode_frame::<Request>(&encoded).unwrap();
                assert_eq!(
                    serde_json::to_value(request).unwrap(),
                    serde_json::to_value(roundtrip).unwrap()
                );
            }
            Err(_) => refused += 1,
        }
    }
    assert!(refused > 0);
}

#[test]
fn security_property_signed_archive_mutations_cannot_change_accepted_files() {
    use zip::CompressionMethod;
    for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        let mut fixture = super::archive_tests::fixture(method);
        let original = fixture.archive.clone();
        let descriptor = fixture.verify().unwrap();
        let expected = archive::validate_archive(&descriptor, &fixture.manifest, &original)
            .unwrap()
            .files()
            .clone();
        let mut corpus = Corpus(0x8a31_a2c4_1907_0051);
        let mut refused = 0;
        for _ in 0..256 {
            fixture.archive = corpus.mutate(&original);
            fixture.package.archive.sha256 = hash(&fixture.archive);
            fixture.package.archive.size = fixture.archive.len() as u64;
            // Re-sign each mutation so the ZIP parser is actually reached.
            let descriptor = fixture.verify().unwrap();
            match archive::validate_archive(&descriptor, &fixture.manifest, &fixture.archive) {
                Ok(accepted) => assert_eq!(accepted.files(), &expected),
                Err(_) => refused += 1,
            }
        }
        assert!(refused > 0);
    }
}

#[test]
fn security_property_media_projection_stays_bounded_and_progress_is_chunk_invariant() {
    let seed = br#"{"format":{"format_name":"wav","duration":"1.0","size":"100"},"streams":[{"index":0,"codec_type":"audio","codec_name":"pcm_s16le","sample_rate":"48000","channels":2}]}"#;
    assert!(parse_inspection(seed).is_ok());
    let mut corpus = Corpus(0x8a31_50e4_0701_0003);
    for _ in 0..4096 {
        if let Ok(value) = parse_inspection(&corpus.mutate(seed)) {
            assert!(!value.streams.is_empty() && value.streams.len() <= 32);
            assert!(value.bytes.is_none_or(|size| size <= MAX_MEDIA_BYTES));
            assert!(value.duration_ms.is_none_or(|time| time <= MAX_DURATION_MS));
        }
    }
    let progress = b"out_time_us=1000000\ntotal_size=100\nprogress=continue\nout_time_us=2000000\ntotal_size=200\nprogress=end\n";
    let mut whole = ProgressDecoder::default();
    whole.push(progress).unwrap();
    let expected = whole.finish().unwrap();
    for width in 1..=progress.len() {
        let mut split = ProgressDecoder::default();
        for chunk in progress.chunks(width) {
            split.push(chunk).unwrap();
        }
        assert_eq!(split.finish().unwrap(), expected);
    }
    for width in 1..=32 {
        let mut poisoned = ProgressDecoder::default();
        assert!(poisoned.push(&vec![0xff; width]).is_err());
        assert!(poisoned.push(progress).is_err());
        assert!(poisoned.finish().is_err());
    }
}
