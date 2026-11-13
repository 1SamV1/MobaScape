/// RuneScape's XTEA variant, operating on big-endian 32-bit words.
pub fn decrypt(data: &mut [u8], key: [u32; 4]) {
    const DELTA: u32 = 0x9e37_79b9;

    for block in data.chunks_exact_mut(8) {
        let mut v0 = u32::from_be_bytes([block[0], block[1], block[2], block[3]]);
        let mut v1 = u32::from_be_bytes([block[4], block[5], block[6], block[7]]);
        let mut sum = DELTA.wrapping_mul(32);

        for _ in 0..32 {
            v1 = v1.wrapping_sub(
                (((v0 << 4) ^ (v0 >> 5)).wrapping_add(v0))
                    ^ sum.wrapping_add(key[((sum >> 11) & 3) as usize]),
            );
            sum = sum.wrapping_sub(DELTA);
            v0 = v0.wrapping_sub(
                (((v1 << 4) ^ (v1 >> 5)).wrapping_add(v1))
                    ^ sum.wrapping_add(key[(sum & 3) as usize]),
            );
        }

        block[0..4].copy_from_slice(&v0.to_be_bytes());
        block[4..8].copy_from_slice(&v1.to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_encrypt_decrypt() {
        // Encryption is the inverse of decryption; test by implementing encrypt locally.
        const DELTA: u32 = 0x9e37_79b9;
        fn encrypt(data: &mut [u8], key: [u32; 4]) {
            for block in data.chunks_exact_mut(8) {
                let mut v0 = u32::from_be_bytes([block[0], block[1], block[2], block[3]]);
                let mut v1 = u32::from_be_bytes([block[4], block[5], block[6], block[7]]);
                let mut sum = 0u32;
                for _ in 0..32 {
                    v0 = v0.wrapping_add(
                        (((v1 << 4) ^ (v1 >> 5)).wrapping_add(v1))
                            ^ sum.wrapping_add(key[(sum & 3) as usize]),
                    );
                    sum = sum.wrapping_add(DELTA);
                    v1 = v1.wrapping_add(
                        (((v0 << 4) ^ (v0 >> 5)).wrapping_add(v0))
                            ^ sum.wrapping_add(key[((sum >> 11) & 3) as usize]),
                    );
                }
                block[0..4].copy_from_slice(&v0.to_be_bytes());
                block[4..8].copy_from_slice(&v1.to_be_bytes());
            }
        }

        let key = [1, 2, 3, 4];
        let mut data = *b"0123456789abcdef";
        let original = data;
        encrypt(&mut data, key);
        decrypt(&mut data, key);
        assert_eq!(data, original);
    }
}
