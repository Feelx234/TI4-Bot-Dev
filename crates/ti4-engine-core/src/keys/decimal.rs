//! A decimal number that sorts like its text.

use core::cmp::Ordering;

/// A number whose key text is its decimal digits without padding (`private:#moved:{seq}`).
///
/// The string-keyed map iterates such keys in *text* order (`"10" < "9"`), so this type's
/// `Ord` compares the digit strings, not the values. It is the one place where integer order and
/// string order differ. A number that is zero padded to a fixed width (`cards:staged:00000007`)
/// sorts like its value and is a plain `u32` in the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecimalTextOrd(pub u32);

/// The decimal digits of `n`, most significant first, and how many there are.
#[must_use]
pub fn digits(mut n: u32) -> ([u8; 10], usize) {
    let mut reversed = [0u8; 10];
    let mut len = 0;
    loop {
        reversed[len] = (n % 10) as u8;
        n /= 10;
        len += 1;
        if n == 0 {
            break;
        }
    }
    let mut out = [0u8; 10];
    for (i, digit) in reversed[..len].iter().rev().enumerate() {
        out[i] = *digit;
    }
    (out, len)
}

impl DecimalTextOrd {
    /// Compare the texts of `self` followed by the byte `term` (0 for the end of the key) and
    /// `other` followed by `other_term`. A digit string that is a proper prefix of the other is
    /// decided by the byte that follows it against the other's next digit.
    #[must_use]
    pub fn cmp_followed(self, term: u8, other: Self, other_term: u8) -> Ordering {
        let (a, alen) = digits(self.0);
        let (b, blen) = digits(other.0);
        let common = alen.min(blen);
        for i in 0..common {
            match a[i].cmp(&b[i]) {
                Ordering::Equal => {}
                unequal => return unequal,
            }
        }
        match alen.cmp(&blen) {
            Ordering::Equal => term.cmp(&other_term),
            Ordering::Less => term.cmp(&(b'0' + b[alen])),
            Ordering::Greater => (b'0' + a[blen]).cmp(&other_term),
        }
    }
}

impl PartialOrd for DecimalTextOrd {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DecimalTextOrd {
    /// Text order of the digits, nothing following (`"1" < "10" < "2"`).
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_followed(0, *other, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_of() {
        assert_eq!(digits(0), ([0; 10], 1));
        let (d, n) = digits(4_294_967_295);
        assert_eq!((&d[..n], n), (&[4, 2, 9, 4, 9, 6, 7, 2, 9, 5][..], 10));
    }

    #[test]
    fn text_order_not_numeric() {
        let mut v: Vec<u32> = vec![9, 10, 1, 100, 11, 2, 0, 99];
        v.sort_by_key(|n| DecimalTextOrd(*n));
        assert_eq!(v, vec![0, 1, 10, 100, 11, 2, 9, 99]);
    }

    #[test]
    fn a_following_byte_decides_a_prefix() {
        // "1|x" against "10": '|' sorts after '0'.
        assert_eq!(
            DecimalTextOrd(1).cmp_followed(b'|', DecimalTextOrd(10), b'|'),
            Ordering::Greater
        );
        // A ',' sorts before every digit.
        assert_eq!(
            DecimalTextOrd(1).cmp_followed(b',', DecimalTextOrd(10), b','),
            Ordering::Less
        );
        assert_eq!(
            DecimalTextOrd(1).cmp_followed(0, DecimalTextOrd(10), b'|'),
            Ordering::Less
        );
    }
}
