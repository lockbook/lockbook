//! Delta-debug a buffer that triggers a failure. Tries removing single
//! bytes and lowering each byte's value, repeating until no further
//! reduction is possible.

pub fn shrink(initial: Vec<u8>, fails: impl Fn(&[u8]) -> bool) -> Vec<u8> {
    let mut buf = initial;
    loop {
        let prev_len = buf.len();
        let prev_sum: u64 = buf.iter().map(|&b| b as u64).sum();

        // remove individual bytes (right to left to keep prefixes stable)
        let mut i = buf.len();
        while i > 0 {
            i -= 1;
            let mut shrunk = buf.clone();
            shrunk.remove(i);
            if fails(&shrunk) {
                buf = shrunk;
            }
        }

        // lower each byte to the smallest still-failing value
        for i in 0..buf.len() {
            for v in 0..buf[i] {
                let mut shrunk = buf.clone();
                shrunk[i] = v;
                if fails(&shrunk) {
                    buf[i] = v;
                    break;
                }
            }
        }

        if buf.len() == prev_len && buf.iter().map(|&b| b as u64).sum::<u64>() == prev_sum {
            break;
        }
    }
    buf
}
