use std::cmp::Ordering;

/// A number whose `Ord` is the order of its decimal *text* (`"10" < "9"`), for key families such as
/// `private:#moved:{seq}` that iterate in text order in the string state (principle 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DecimalTextOrd(pub u32);

fn digits(mut n: u32, out: &mut [u8; 10]) -> usize {
    if n == 0 {
        out[0] = b'0';
        return 1;
    }
    let mut len = 0;
    let mut rev = [0u8; 10];
    while n > 0 {
        rev[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    for i in 0..len {
        out[i] = rev[len - 1 - i];
    }
    len
}

impl Ord for DecimalTextOrd {
    fn cmp(&self, other: &Self) -> Ordering {
        let (mut a, mut b) = ([0u8; 10], [0u8; 10]);
        let la = digits(self.0, &mut a);
        let lb = digits(other.0, &mut b);
        a[..la].cmp(&b[..lb])
    }
}

impl PartialOrd for DecimalTextOrd {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_as_decimal_text() {
        let mut v: Vec<u32> = vec![9, 10, 100, 2, 19, 0, 1, 99, 1000, u32::MAX, 4_000_000_000];
        v.sort_by_key(|n| DecimalTextOrd(*n));
        let mut t: Vec<String> = v.iter().map(ToString::to_string).collect();
        t.sort();
        assert_eq!(v.iter().map(ToString::to_string).collect::<Vec<_>>(), t);
        assert!(DecimalTextOrd(10) < DecimalTextOrd(9));
        assert!(DecimalTextOrd(9) > DecimalTextOrd(10));
    }

    #[test]
    fn exhaustive_small() {
        let mut v: Vec<u32> = (0..3000).collect();
        v.sort_by_key(|n| DecimalTextOrd(*n));
        let mut s: Vec<String> = (0..3000u32).map(|n| n.to_string()).collect();
        s.sort();
        assert_eq!(v.iter().map(ToString::to_string).collect::<Vec<_>>(), s);
    }
}
