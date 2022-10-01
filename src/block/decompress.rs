// Partially based on lz4_flex

use core::mem::MaybeUninit;

pub fn decompress(input: &[u8], max_size: usize) -> Result<Vec<u8>, DecompressError> {
    let mut v = Vec::with_capacity(max_size);
    let l = unsafe { decompress_into_uninit(input, v.spare_capacity_mut())?.len() };
    unsafe { v.set_len(l) }
    Ok(v)
}

unsafe fn decompress_into_uninit<'a>(
    input: &[u8],
    buf: &'a mut [MaybeUninit<u8>],
) -> Result<&'a mut [u8], DecompressError> {
    unsafe {
        let l = decompress_raw(
            input.as_ptr(),
            input.len(),
            buf.as_mut_ptr().cast(),
            buf.len(),
        )?;
        Ok(MaybeUninit::slice_assume_init_mut(
            buf.get_unchecked_mut(..l),
        ))
    }
}

unsafe fn decompress_raw<'a>(
    mut input_ptr: *const u8,
    input_len: usize,
    mut buf_ptr: *mut u8,
    buf_len: usize,
) -> Result<usize, DecompressError> {
    let input_end = input_ptr.add(input_len);
    let buf_start = buf_ptr;
    let buf_end = buf_ptr.add(buf_len);
    loop {
        // Get token
        if input_ptr >= input_end {
            return Err(DecompressError);
        }
        let t = input_ptr.read();
        input_ptr = input_ptr.add(1);

        // 7:4 = literals length, 3:0 = match length
        let mut lit_len = usize::from(t) >> 4;
        let mut match_len = (usize::from(t) & 15) + 4;

        // Special case "short" tokens, which are common
        if lit_len <= 14
            && match_len <= 18
            && input_end.sub_ptr(input_ptr) >= 14 + 2
            && buf_end.sub_ptr(buf_ptr) >= 14 + 18
        {
            input_ptr.copy_to_nonoverlapping(buf_ptr, 16);
            input_ptr = input_ptr.add(lit_len);
            buf_ptr = buf_ptr.add(lit_len);

            let offt = usize::from(input_ptr.cast::<u16>().read_unaligned().to_le());
            if offt == 0 || buf_ptr.sub_ptr(buf_start) < offt {
                return Err(DecompressError);
            }
            input_ptr = input_ptr.add(2);
            let buf_offt = buf_ptr.sub(offt);
            if match_len <= offt {
                buf_ptr.copy_from(buf_offt, 18);
                buf_ptr = buf_ptr.add(match_len);
            } else {
                copy_dup(&mut buf_ptr, buf_offt, match_len);
            }
            continue;
        }

        // Copy literals
        if lit_len == 0 {
            /* pass */
        } else if lit_len < 15
            && input_end.sub_ptr(input_ptr) >= 16
            && buf_end.sub_ptr(buf_ptr) >= 16
        {
            input_ptr.copy_to_nonoverlapping(buf_ptr, 16);
            buf_ptr = buf_ptr.add(lit_len);
        } else {
            if lit_len == 15 {
                lit_len += get_long_int(&mut input_ptr, input_end)?;
            }
            if input_end.sub_ptr(input_ptr) < lit_len || buf_end.sub_ptr(buf_ptr) < lit_len {
                return Err(DecompressError);
            }
            if input_end.sub_ptr(input_ptr) >= lit_len + 16
                && buf_end.sub_ptr(buf_ptr) >= lit_len + 16
            {
                inline_memcpy_16(&mut buf_ptr, input_ptr, lit_len);
            } else {
                buf_ptr.copy_from_nonoverlapping(input_ptr, lit_len);
                buf_ptr = buf_ptr.add(lit_len);
            }
        }
        input_ptr = input_ptr.add(lit_len);

        // Stop if the block ended.
        if input_ptr >= input_end {
            break;
        }

        // Copy match
        if input_ptr.add(2) >= input_end {
            return Err(DecompressError);
        }
        let offt = usize::from(u16::from_le(input_ptr.cast::<u16>().read_unaligned()));
        input_ptr = input_ptr.add(2);
        if match_len == 19 {
            match_len += get_long_int(&mut input_ptr, input_end)?;
        }
        if offt == 0 || buf_ptr.sub_ptr(buf_start) < offt || buf_end.sub_ptr(buf_ptr) < match_len {
            return Err(DecompressError);
        }
        let buf_offt = buf_ptr.sub(offt);

        if offt < match_len || buf_end.sub_ptr(buf_ptr) < match_len + 16 {
            copy_dup(&mut buf_ptr, buf_offt, match_len)
        } else {
            inline_memcpy_16(&mut buf_ptr, buf_offt, match_len)
        }
    }
    Ok(buf_ptr.sub_ptr(buf_start))
}

#[inline(always)]
unsafe fn get_long_int(
    input_ptr: &mut *const u8,
    input_end: *const u8,
) -> Result<usize, DecompressError> {
    let mut s = 0;
    while {
        if *input_ptr >= input_end {
            return Err(DecompressError);
        }
        let n = input_ptr.read();
        s += usize::from(n);
        *input_ptr = input_ptr.add(1);
        n == 255
    } {}
    Ok(s)
}

#[inline(always)]
unsafe fn copy_dup(to: &mut *mut u8, mut from: *const u8, count: usize) {
    let end = to.add(count);
    while *to < end {
        to.write(from.read());
        *to = to.add(1);
        from = from.add(1);
    }
}

/// # Safety
///
/// This function may write up to 15 bytes after count.
/// Only call this function if there is at least 15 bytes bytes of unused space after count.
#[inline(always)]
unsafe fn inline_memcpy_16(to: &mut *mut u8, mut from: *const u8, count: usize) {
    let mut ptr = *to;
    let end = to.add(count);
    *to = to.add(count);
    while ptr < end {
        ptr.copy_from(from, 16);
        ptr = ptr.add(16);
        from = from.add(16);
    }
}

#[derive(Debug)]
pub struct DecompressError;

#[cfg(test)]
mod test {
    use super::*;

    // Use miri to get any useful info

    #[test]
    fn fuzz_000() {
        let d = &[
            130, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255,
        ];
        let _ = decompress(d, 1 << 21);
    }

    #[test]
    fn fuzz_001() {
        let d = &[
            255, 22, 0, 0, 0, 38, 0, 137, 0, 255, 255, 39, 255, 255, 223, 255, 255, 255, 255, 255,
            255, 255, 255, 253, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255,
        ];
        let _ = decompress(d, 1 << 21);
    }

    #[test]
    fn fuzz_002() {
        let d = &[
            255, 0, 0, 126, 0, 0, 0, 0, 0, 3, 0, 33, 255, 254, 255, 254, 255, 1, 0, 255, 255, 255,
            255, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 79, 0, 1, 0, 0, 0, 1, 0, 0, 16, 0, 22, 0, 15, 0, 1,
            0, 248,
        ];
        let _ = decompress(d, 1 << 10);
    }

    #[test]
    fn fuzz_003() {
        let d = &[
            255, 36, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 64, 0, 0, 0, 0, 0, 20, 0, 0, 0, 0, 0, 23, 0, 0,
            0, 0, 4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 15, 0, 0, 0, 0, 0, 0, 50, 50, 50, 50, 50, 50,
            50, 0, 255, 255, 255, 174, 64, 0, 1, 0, 213, 213, 213, 213, 213, 213, 213, 213, 213,
            213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213,
            213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 213,
            213, 213, 213, 213, 213, 213, 213, 213, 213, 213, 0, 255, 255,
        ];
        let _ = decompress(d, 1 << 10);
    }
}
