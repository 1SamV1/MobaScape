use crate::error::{Error, Result};

/// A cursor over cache bytes, mirroring the RuneScape client buffer semantics.
#[derive(Clone, Debug)]
pub struct Buffer<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Buffer<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn slice(&self, start: usize, end: usize) -> Result<&'a [u8]> {
        self.data.get(start..end).ok_or(Error::Eof(start.max(end)))
    }

    pub fn peek_u8(&self) -> Result<u8> {
        self.data.get(self.pos).copied().ok_or(Error::Eof(self.pos))
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        let v = self.peek_u8()?;
        self.pos += 1;
        Ok(v)
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_u8()? != 0)
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub fn read_i16(&mut self) -> Result<i16> {
        Ok(self.read_u16()? as i16)
    }

    pub fn read_u24(&mut self) -> Result<u32> {
        let b = self.read_bytes(3)?;
        Ok(((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32)
    }

    pub fn read_i24(&mut self) -> Result<i32> {
        let v = self.read_u24()? as i32;
        Ok(if v > 0x7f_ffff { v - 0x100_0000 } else { v })
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        Ok(self.read_u32()? as i32)
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        let b = self.read_bytes(8)?;
        Ok(u64::from_be_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| Error::parse("buffer overflow"))?;
        let s = self.data.get(self.pos..end).ok_or(Error::Eof(self.pos))?;
        self.pos = end;
        Ok(s)
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.read_bytes(n).map(|_| ())
    }

    /// Signed 1/2-byte smart used by model deltas.
    pub fn read_short_smart(&mut self) -> Result<i32> {
        if self.peek_u8()? < 128 {
            Ok(self.read_u8()? as i32 - 64)
        } else {
            Ok(self.read_u16()? as i32 - 0xc000)
        }
    }

    /// Unsigned 1/2-byte smart.
    pub fn read_unsigned_short_smart(&mut self) -> Result<u16> {
        if self.peek_u8()? < 128 {
            Ok(self.read_u8()? as u16)
        } else {
            Ok(self.read_u16()?.wrapping_sub(0x8000))
        }
    }

    /// RuneScape's "smart int" that chains 32767 sentinels.
    pub fn read_unsigned_int_smart_short_compat(&mut self) -> Result<u32> {
        let mut value: u32 = 0;
        loop {
            let v = self.read_unsigned_short_smart()? as u32;
            if v == 32767 {
                value = value.saturating_add(32767);
            } else {
                return Ok(value.saturating_add(v));
            }
        }
    }

    pub fn read_big_smart(&mut self) -> Result<i32> {
        if self.peek_u8()? & 0x80 != 0 {
            Ok((self.read_u32()? & 0x7fff_ffff) as i32)
        } else {
            Ok(self.read_u16()? as i32)
        }
    }

    pub fn read_string(&mut self) -> Result<String> {
        const CP1252: [char; 32] = [
            '\u{20ac}', '\u{0}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}',
            '\u{2021}', '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{0}',
            '\u{17d}', '\u{0}', '\u{0}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}',
            '\u{2022}', '\u{2013}', '\u{2014}', '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}',
            '\u{153}', '\u{0}', '\u{17e}', '\u{178}',
        ];

        let mut out = String::new();
        loop {
            let b = self.read_u8()?;
            if b == 0 {
                break;
            }
            if (128..160).contains(&b) {
                let c = CP1252[(b - 128) as usize];
                out.push(if c == '\0' { '?' } else { c });
            } else {
                out.push(b as char);
            }
        }
        Ok(out)
    }
}
