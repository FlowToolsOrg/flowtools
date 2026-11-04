//! Deterministic baseline H.264 I_PCM frames in an AVI. No encoder process,
//! downloaded media, raw-video build option or media path is needed for inputs.
#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    position: u8,
}
impl Bits {
    fn bit(&mut self, value: bool) {
        if self.position == 0 {
            self.bytes.push(0);
        }
        if value {
            *self.bytes.last_mut().unwrap() |= 1 << (7 - self.position);
        }
        self.position = (self.position + 1) % 8;
    }
    fn number(&mut self, value: u32, width: u32) {
        for shift in (0..width).rev() {
            self.bit(value & (1 << shift) != 0);
        }
    }
    fn ue(&mut self, value: u32) {
        let code = value.checked_add(1).unwrap();
        let width = 32 - code.leading_zeros();
        for _ in 1..width {
            self.bit(false);
        }
        self.number(code, width);
    }
    fn align(&mut self) {
        while self.position != 0 {
            self.bit(false);
        }
    }
    fn finish(mut self, header: u8) -> Vec<u8> {
        self.bit(true);
        self.align();
        let mut nal = vec![0, 0, 0, 1, header];
        let mut zeros = 0;
        for byte in self.bytes {
            if zeros == 2 && byte <= 3 {
                nal.push(3);
                zeros = 0;
            }
            nal.push(byte);
            zeros = if byte == 0 { zeros + 1 } else { 0 };
        }
        nal
    }
}
fn parameters(side: usize) -> Vec<u8> {
    let mut sps = Bits::default();
    sps.number(66, 8);
    sps.number(0, 8);
    sps.number(30, 8);
    sps.ue(0);
    sps.ue(0);
    sps.ue(2);
    sps.ue(1);
    sps.bit(false);
    sps.ue((side / 16 - 1) as u32);
    sps.ue((side / 16 - 1) as u32);
    sps.bit(true);
    sps.bit(true);
    sps.bit(false);
    sps.bit(false);
    let mut bytes = sps.finish(0x67);
    let mut pps = Bits::default();
    pps.ue(0);
    pps.ue(0);
    pps.bit(false);
    pps.bit(false);
    pps.ue(0);
    pps.ue(0);
    pps.ue(0);
    pps.bit(false);
    pps.number(0, 2);
    pps.ue(0);
    pps.ue(0);
    pps.ue(0);
    pps.bit(false);
    pps.bit(false);
    pps.bit(false);
    bytes.extend(pps.finish(0x68));
    bytes
}
fn frame(side: usize, index: usize) -> Vec<u8> {
    let mut slice = Bits::default();
    slice.ue(0);
    slice.ue(2);
    slice.ue(0);
    slice.number(0, 4);
    slice.ue(0);
    slice.bit(false);
    slice.bit(false);
    slice.ue(0);
    for block in 0..side * side / 256 {
        slice.ue(25); // I_PCM, with a byte-aligned 16x16 luma + 8x8 chroma.
        slice.align();
        slice
            .bytes
            .extend((0..256usize).map(|pixel| (pixel * 71 + index * 37 + block * 19) as u8));
        slice.bytes.extend(std::iter::repeat_n(128u8, 128));
    }
    slice.finish(0x65)
}
fn chunk(id: &[u8; 4], bytes: &[u8]) -> Vec<u8> {
    let mut result = id.to_vec();
    result.extend((bytes.len() as u32).to_le_bytes());
    result.extend_from_slice(bytes);
    if !bytes.len().is_multiple_of(2) {
        result.push(0);
    }
    result
}
fn numbers(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|n| n.to_le_bytes()).collect()
}
pub fn video(frames: usize, side: usize) -> Vec<u8> {
    assert!(matches!(side, 16 | 64) && (1..=5000).contains(&frames));
    let mut headers = b"hdrl".to_vec();
    headers.extend(chunk(
        b"avih",
        &numbers(&[
            1_000_000,
            0,
            0,
            0,
            frames as u32,
            0,
            1,
            0,
            side as u32,
            side as u32,
            0,
            0,
            0,
            0,
        ]),
    ));
    let mut stream = b"strl".to_vec();
    let mut strh = b"vidsH264".to_vec();
    strh.extend(numbers(&[0, 0, 0, 1, 1, 0, frames as u32, 0, u32::MAX, 0]));
    for value in [0, 0, side as u16, side as u16] {
        strh.extend(value.to_le_bytes());
    }
    assert_eq!(strh.len(), 56);
    stream.extend(chunk(b"strh", &strh));
    let mut strf = numbers(&[40, side as u32, side as u32]);
    strf.extend(1u16.to_le_bytes());
    strf.extend(24u16.to_le_bytes());
    strf.extend_from_slice(b"H264");
    strf.extend(numbers(&[(side * side * 3) as u32, 0, 0, 0, 0]));
    assert_eq!(strf.len(), 40);
    stream.extend(chunk(b"strf", &strf));
    headers.extend(chunk(b"LIST", &stream));
    let params = parameters(side);
    let mut movi = b"movi".to_vec();
    for index in 0..frames {
        let mut data = params.clone();
        data.extend(frame(side, index));
        movi.extend(chunk(b"00dc", &data));
    }
    let mut data = b"AVI ".to_vec();
    data.extend(chunk(b"LIST", &headers));
    data.extend(chunk(b"LIST", &movi));
    chunk(b"RIFF", &data)
}
