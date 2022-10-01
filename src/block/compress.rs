use core::mem::MaybeUninit;
use std::collections::hash_map::{Entry, HashMap};

pub fn compress(input: &[u8], max_size: usize) -> Result<Vec<u8>, CompressError> {
    let mut v = Vec::with_capacity(max_size);
    let l = compress_into_uninit(input, v.spare_capacity_mut())?;
    unsafe { v.set_len(l) };
    Ok(v)
}

fn compress_into_uninit(
    input: &[u8],
    output: &mut [MaybeUninit<u8>],
) -> Result<usize, CompressError> {
    // Super inefficient but w/e
    let mut dict = HashMap::new();

    let mut start @ mut i @ mut o = 0;
    let mut push = |c| {
        output.get_mut(o).ok_or(CompressError)?.write(c);
        o += 1;
        Ok(())
    };

    while i <= input.len() {
        if start + 4 > i {
            // match length is at least 4
            //dict.insert(&input[start..i], start);
            i += 1;
            continue;
        }
        for k in start..i - 4 {
            match dict.entry(&input[k..i]) {
                Entry::Occupied(mut e) if k - *e.get() <= 0xffff => {
                    // Determine lengths
                    let offt = k - *e.get();
                    let mut lit_len = k - start;
                    let mut match_len = i - k - 4;

                    // Find the upper bound on matching bytes
                    {
                        let mut n = k + match_len + 4;
                        while input.get(n).map_or(false, |&c| c == input[n - offt]) {
                            match_len += 1;
                            n += 1;
                            i += 1;
                        }
                    }

                    // Make token
                    let lit_len_tk = lit_len.min(15);
                    let match_len_tk = match_len.min(15);
                    push((lit_len_tk << 4 | match_len_tk) as _)?;
                    lit_len -= lit_len_tk;
                    match_len -= match_len_tk;

                    // Push long literal length
                    if lit_len_tk == 15 {
                        while {
                            let l = lit_len.min(255);
                            push(l as _)?;
                            lit_len -= l;
                            l == 255
                        } {}
                    }

                    // Copy literals
                    input[start..k].iter().copied().try_for_each(&mut push)?;

                    // Push match offset
                    push(offt as _)?;
                    push((offt >> 8) as _)?;

                    // Push long match length
                    if match_len_tk == 15 {
                        while {
                            let l = match_len.min(255);
                            push(l as _)?;
                            match_len -= l;
                            l == 255
                        } {}
                    }

                    e.insert(k);
                    start = i;
                }
                Entry::Occupied(mut e) => {
                    e.insert(k);
                }
                Entry::Vacant(e) => {
                    e.insert(k);
                }
            }
        }
        i += 1;
    }

    // Insert remaining literals
    let mut lit_len = i - start - 1;

    // Make token
    let lit_len_tk = lit_len.min(15);
    push((lit_len_tk << 4) as _)?;
    lit_len -= lit_len_tk;

    // Push long literal length
    if lit_len_tk == 15 {
        while {
            let l = lit_len.min(255);
            push(l as _)?;
            lit_len -= l;
            l == 255
        } {}
    }

    // Copy literals
    input[start..].iter().copied().try_for_each(&mut push)?;

    Ok(o)
}

#[derive(Debug)]
pub struct CompressError;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn compress_hello() {
        let s = b"Hello, world!";
        let e = compress(s, 50000).unwrap();
        dbg!(&e);
        dbg!(String::from_utf8_lossy(&e));
        let d = super::super::decompress(&e, s.len()).unwrap();
        dbg!(String::from_utf8_lossy(&d));
        dbg!(s.len(), e.len(), d.len());
        todo!();
    }

    #[test]
    fn compress_dup() {
        let s = b"hellohello";
        let e = compress(s, 50000).unwrap();
        dbg!(&e);
        dbg!(String::from_utf8_lossy(&e));
        let d = super::super::decompress(&e, s.len()).unwrap();
        dbg!(String::from_utf8_lossy(&d));
        dbg!(s.len(), e.len(), d.len());
        todo!();
    }

    #[test]
    fn compress_small() {
        let s = b"The File Allocation Table (FAT) file system was introduced with DOS v1.0 (and possibly CP/M). Supposedly written by Bill Gates, FAT is a very simple file system -- nothing more than a singly-linked list of clusters in a gigantic table. A FAT file system uses very little memory (unless the OS caches the whole allocation table in memory) and is one of, if not the, most basic file system in use today.";
        let e = compress(s, 50000).unwrap();
        let d = super::super::decompress(&e, s.len()).unwrap();
        dbg!(String::from_utf8_lossy(&d));
        dbg!(s.len(), e.len(), d.len());
        todo!();
    }

    #[test]
    fn compress_big() {
        let s = std::fs::read("../lz4_Block_format.md").unwrap();
        let e = compress(&s, s.len()).unwrap();
        let d = super::super::decompress(&e, s.len()).unwrap();
        dbg!(bstr::BStr::new(&e));
        dbg!(bstr::BStr::new(&d));
        dbg!(s.len(), e.len());
        todo!();
    }
}
