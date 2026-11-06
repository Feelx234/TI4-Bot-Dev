//! A minimal `.npy` (format 1.0) writer: C order, little endian, the dtypes the shards use. `numpy.load(path, mmap_mode="r")`
//! reads the files.

use std::io::Write;
use std::path::Path;

/// The element types of the shards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dtype {
    F16,
    F32,
    U8,
    I16,
    U32,
    I64,
}

impl Dtype {
    #[must_use]
    pub const fn descr(self) -> &'static str {
        match self {
            Self::F16 => "<f2",
            Self::F32 => "<f4",
            Self::U8 => "|u1",
            Self::I16 => "<i2",
            Self::U32 => "<u4",
            Self::I64 => "<i8",
        }
    }

    #[must_use]
    pub const fn size(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::F16 | Self::I16 => 2,
            Self::F32 | Self::U32 => 4,
            Self::I64 => 8,
        }
    }
}

/// IEEE half from a float, round to nearest even (overflow to infinity, NaN kept as a quiet NaN).
#[must_use]
pub fn f16_bits(x: f32) -> u16 {
    let bits = x.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32;
    let man = bits & 0x007f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if man != 0 { 0x0200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        // Subnormal: shift the mantissa with its implicit bit in.
        let m = man | 0x0080_0000;
        let shift = (14 - e) as u32;
        let half = m >> shift;
        let rest = m & ((1 << shift) - 1);
        let mid = 1 << (shift - 1);
        let round = u32::from(rest > mid || (rest == mid && half & 1 == 1));
        return sign | (half + round) as u16;
    }
    let half = ((e as u32) << 10) | (man >> 13);
    let rest = man & 0x1fff;
    let round = u32::from(rest > 0x1000 || (rest == 0x1000 && half & 1 == 1));
    sign | (half + round) as u16
}

/// The float of a half (for tests and readers).
#[must_use]
pub fn f16_value(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from((h >> 10) & 0x1f);
    let man = f32::from(h & 0x3ff);
    match exp {
        0 => sign * man * 2f32.powi(-24),
        0x1f => {
            if man == 0.0 {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => sign * (1.0 + man / 1024.0) * 2f32.powi(exp - 15),
    }
}

/// The header of an array (magic, version 1.0, the dict padded to a multiple of 64 bytes with a newline).
#[must_use]
pub fn header(dtype: Dtype, shape: &[usize]) -> Vec<u8> {
    let dims = match shape {
        [one] => format!("({one},)"),
        _ => format!("({})", shape.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")),
    };
    let mut dict = format!("{{'descr': '{}', 'fortran_order': False, 'shape': {dims}, }}", dtype.descr());
    let unpadded = 10 + dict.len() + 1;
    dict.push_str(&" ".repeat((64 - unpadded % 64) % 64));
    dict.push('\n');
    let mut out = b"\x93NUMPY\x01\x00".to_vec();
    out.extend_from_slice(&u16::try_from(dict.len()).unwrap_or(u16::MAX).to_le_bytes());
    out.extend_from_slice(dict.as_bytes());
    out
}

/// Write one array. `data` is the little-endian payload; its length must be the product of `shape` times the element size.
///
/// # Errors
/// The I/O error, or a payload of the wrong length.
pub fn write(path: &Path, dtype: Dtype, shape: &[usize], data: &[u8]) -> std::io::Result<()> {
    let want = shape.iter().product::<usize>() * dtype.size();
    if data.len() != want {
        return Err(std::io::Error::other(format!("{}: {} bytes for shape {shape:?} of {dtype:?} ({want} expected)", path.display(), data.len())));
    }
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(&header(dtype, shape))?;
    f.write_all(data)?;
    f.flush()
}

/// [`write`] through gzip (`path` should end in `.npy.gz`; `numpy.lib.format.read_array(gzip.open(path))` reads it).
///
/// # Errors
/// As [`write`].
pub fn write_gz(path: &Path, dtype: Dtype, shape: &[usize], data: &[u8]) -> std::io::Result<()> {
    let want = shape.iter().product::<usize>() * dtype.size();
    if data.len() != want {
        return Err(std::io::Error::other(format!("{}: {} bytes for shape {shape:?} of {dtype:?} ({want} expected)", path.display(), data.len())));
    }
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::new(3));
    gz.write_all(&header(dtype, shape))?;
    gz.write_all(data)?;
    gz.finish()?.flush()
}

/// Little-endian bytes of halves.
#[must_use]
pub fn f16_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| f16_bits(*v).to_le_bytes()).collect()
}

#[must_use]
pub fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[must_use]
pub fn i16_bytes(values: &[i16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[must_use]
pub fn u32_bytes(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[must_use]
pub fn i64_bytes(values: &[i64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// An array read back: the element type, the shape and the values widened (floats to `f32`, integers to `i64`).
#[derive(Debug, Clone, PartialEq)]
pub struct Array {
    pub dtype: Dtype,
    pub shape: Vec<usize>,
    pub f: Vec<f32>,
    pub i: Vec<i64>,
}

/// Read a plain `.npy` file (format 1.0, C order, the dtypes of [`Dtype`]): what [`write`] writes and what NumPy's `np.save`
/// writes for those dtypes.
///
/// # Errors
/// I/O, or a header this reader does not understand.
pub fn read(path: &Path) -> std::io::Result<Array> {
    let raw = std::fs::read(path)?;
    let bad = |what: &str| std::io::Error::other(format!("{}: {what}", path.display()));
    if raw.len() < 10 || &raw[..6] != b"\x93NUMPY" {
        return Err(bad("not an .npy file"));
    }
    let (hlen, start) = match raw[6] {
        1 => (usize::from(u16::from_le_bytes([raw[8], raw[9]])), 10),
        2 | 3 => (u32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]) as usize, 12),
        _ => return Err(bad("unknown .npy version")),
    };
    let dict = std::str::from_utf8(raw.get(start..start + hlen).ok_or_else(|| bad("short header"))?).map_err(|_| bad("header not UTF-8"))?;
    if dict.contains("'fortran_order': True") {
        return Err(bad("Fortran order"));
    }
    let descr = dict.split("'descr':").nth(1).and_then(|r| r.split('\'').nth(1)).ok_or_else(|| bad("no descr"))?;
    let dtype = [Dtype::F16, Dtype::F32, Dtype::U8, Dtype::I16, Dtype::U32, Dtype::I64]
        .into_iter()
        .find(|d| d.descr() == descr || (descr == "<u1" && *d == Dtype::U8) || (descr == "|b1" && *d == Dtype::U8))
        .ok_or_else(|| bad("unsupported dtype"))?;
    let dims = dict.split("'shape':").nth(1).and_then(|r| r.split('(').nth(1)).and_then(|r| r.split(')').next()).ok_or_else(|| bad("no shape"))?;
    let shape: Vec<usize> = dims.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::parse).collect::<Result<_, _>>().map_err(|_| bad("bad shape"))?;
    let n: usize = shape.iter().product();
    let data = raw.get(start + hlen..start + hlen + n * dtype.size()).ok_or_else(|| bad("short payload"))?;
    let (mut f, mut i) = (Vec::new(), Vec::new());
    match dtype {
        Dtype::F16 => f = data.chunks_exact(2).map(|c| f16_value(u16::from_le_bytes([c[0], c[1]]))).collect(),
        Dtype::F32 => f = data.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect(),
        Dtype::U8 => i = data.iter().map(|b| i64::from(*b)).collect(),
        Dtype::I16 => i = data.chunks_exact(2).map(|c| i64::from(i16::from_le_bytes([c[0], c[1]]))).collect(),
        Dtype::U32 => i = data.chunks_exact(4).map(|c| i64::from(u32::from_le_bytes([c[0], c[1], c[2], c[3]]))).collect(),
        Dtype::I64 => i = data.chunks_exact(8).map(|c| i64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])).collect(),
    }
    Ok(Array { dtype, shape, f, i })
}

/// `x` rounded to the nearest half and back (the precision the shards store and the trainer sees).
#[must_use]
pub fn f16_round(x: f32) -> f32 {
    f16_value(f16_bits(x))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrays_read_back() {
        let dir = std::env::temp_dir().join(format!("ti4-npy-read-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let p = dir.join("a.npy");
        write(&p, Dtype::F16, &[2, 3], &f16_bytes(&[0.0, 1.0, -2.5, 0.25, 3.0, 1.0 / 3.0])).expect("write");
        let a = read(&p).expect("read");
        assert_eq!(a.shape, vec![2, 3]);
        assert_eq!(a.f[2], -2.5);
        assert_eq!(a.f[5], f16_round(1.0 / 3.0));
        write(&p, Dtype::I16, &[3], &i16_bytes(&[-1, 7, 300])).expect("write");
        assert_eq!(read(&p).expect("read").i, vec![-1, 7, 300]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn halves_round_trip_and_round_to_even() {
        for x in [0.0f32, 1.0, -1.0, 0.5, 0.25, 1.0 / 3.0, 2.0 / 3.0, 65504.0, 6.1e-5, 1e-7, -2.5, 0.1] {
            let back = f16_value(f16_bits(x));
            let tol = (x.abs() * 1e-3).max(6e-8);
            assert!((back - x).abs() <= tol, "{x} -> {back}");
        }
        assert_eq!(f16_bits(1.0), 0x3c00);
        assert_eq!(f16_bits(-2.0), 0xc000);
        assert_eq!(f16_bits(65504.0), 0x7bff);
        assert_eq!(f16_bits(1e6), 0x7c00);
        assert_eq!(f16_bits(0.0), 0);
        // 1 + 2^-11 is halfway between 1 and the next half: even (1.0) wins.
        assert_eq!(f16_bits(1.0 + 2f32.powi(-11)), 0x3c00);
        assert_eq!(f16_bits(1.0 + 3.0 * 2f32.powi(-11)), 0x3c02);
        // The smallest subnormal.
        assert_eq!(f16_bits(2f32.powi(-24)), 1);
    }

    #[test]
    fn the_header_is_aligned_and_names_the_shape() {
        for shape in [vec![3usize], vec![2, 40, 112], vec![0, 4]] {
            let h = header(Dtype::F16, &shape);
            assert_eq!(h.len() % 64, 0);
            assert_eq!(&h[..8], b"\x93NUMPY\x01\x00");
            assert_eq!(*h.last().unwrap_or(&0), b'\n');
            let text = String::from_utf8_lossy(&h[10..]);
            assert!(text.contains("'descr': '<f2'"));
            if shape.len() == 1 {
                assert!(text.contains("(3,)"), "{text}");
            }
        }
    }

    #[test]
    fn a_wrong_payload_is_refused() {
        let dir = std::env::temp_dir().join(format!("npy-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        assert!(write(&dir.join("x.npy"), Dtype::I16, &[3], &[0u8; 5]).is_err());
        assert!(write(&dir.join("y.npy"), Dtype::I16, &[3], &[0u8; 6]).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
