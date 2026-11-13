use std::sync::OnceLock;

const UNIT: f64 = std::f64::consts::PI / 1024.0;

fn tables() -> &'static ([i32; 2048], [i32; 2048]) {
    static TABLES: OnceLock<([i32; 2048], [i32; 2048])> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut sine = [0i32; 2048];
        let mut cosine = [0i32; 2048];
        for i in 0..2048 {
            sine[i] = (65536.0 * (i as f64 * UNIT).sin()) as i32;
            cosine[i] = (65536.0 * (i as f64 * UNIT).cos()) as i32;
        }
        (sine, cosine)
    })
}

pub fn sine(index: i32) -> i32 {
    tables().0[(index & 2047) as usize]
}

pub fn cosine(index: i32) -> i32 {
    tables().1[(index & 2047) as usize]
}
