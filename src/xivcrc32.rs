use std::ops::Add;
use std::ops::AddAssign;
use std::ops::BitXor;
use std::ops::BitXorAssign;

fn crc32(crc: u32, s: &[u8]) -> u32 {
    unsafe { cloudflare_zlib_sys::crc32(crc as u64, s.as_ptr(), s.len() as u32) as u32 }
}

fn crc32_combine(crc1: u32, crc2: u32, len2: usize) -> u32 {
    unsafe { cloudflare_zlib_sys::crc32_combine(crc1 as u64, crc2 as u64, len2 as isize) as u32 }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct XivCrc32 {
    pub crc: u32,
    pub len: usize,
}

impl XivCrc32 {
    pub fn new(crc: u32, len: usize) -> Self {
        Self {
            crc,
            len,
        }
    }
    pub fn zero(len: usize) -> Self {
        Self {
            crc: 0,
            len,
        }
    }
}

impl From<&[u8]> for XivCrc32 {
    fn from(s: &[u8]) -> Self {
        Self::new(!crc32(0xFFFFFFFF, s), s.len())
    }
}

impl <const N: usize> From<&[u8; N]> for XivCrc32 {
    fn from(s: &[u8; N]) -> Self {
        Self::new(!crc32(0xFFFFFFFF, s), N)
    }
}

impl From<&str> for XivCrc32 {
    fn from(s: &str) -> Self {
        Self::from(s.as_bytes())
    }
}

impl Add<XivCrc32> for XivCrc32 {
    type Output = XivCrc32;

    fn add(self, rhs: XivCrc32) -> Self::Output {
        Self::new(crc32_combine(self.crc, rhs.crc, rhs.len), self.len + rhs.len)
    }
}

impl AddAssign<XivCrc32> for XivCrc32 {
    fn add_assign(&mut self, rhs: XivCrc32) {
        self.crc = crc32_combine(self.crc, rhs.crc, rhs.len);
        self.len += rhs.len;
    }
}

impl BitXor<XivCrc32> for XivCrc32 {
    type Output = XivCrc32;

    fn bitxor(self, rhs: XivCrc32) -> Self::Output {
        Self::new(self.crc ^ rhs.crc, self.len.max(rhs.len))
    }
}

impl BitXorAssign<XivCrc32> for XivCrc32 {
    fn bitxor_assign(&mut self, rhs: XivCrc32) {
        self.crc ^= rhs.crc;
        self.len = self.len.max(rhs.len);
    }
}
