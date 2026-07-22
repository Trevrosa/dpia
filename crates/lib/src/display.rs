use heapless::{
    LenType, String, format,
    string::{StringInner, StringStorage},
};

use core::fmt::Write;

pub fn fmt_f32(f: f32) -> (String<8>, u8, usize) {
    debug_assert!(f >= 0.0);
    debug_assert!(f <= 100.0);

    let mut s = format!(8; "{f:.1}").unwrap();

    let len_before = s.len();

    // len_before should be <= 5 and must be <=8
    pad(&mut s, ' ', 8 - len_before);

    // there must be 1 decimal point. including the dot, minus 2
    // 3 <= len_before <= 8, (min of 0.0, max defined by capacity)
    // if in=100.0, len=5
    // dist = len - 3 = 2
    // write from the left, so:
    // 7 - dist = 5
    // 0b0010_0000
    let dots = 1 << (7 - (len_before - 3));

    (s, dots, len_before)
}

pub fn fmt_pad_u8(u: u8, after: Option<usize>) -> String<8> {
    let mut s = String::new();

    if let Some(after) = after {
        // theoretically could have been 100.0 with len=5
        debug_assert!(after <= 6);
        pad(&mut s, ' ', after);
    }

    debug_assert!(u <= 100);
    // max +len of 3
    write!(&mut s, "{u}").unwrap();

    let len = s.len();
    if len < 8 {
        pad(&mut s, ' ', 8 - len);
    }

    s
}

/// Append character `char`, `num` times to string `s`.
///
/// # Panics
///
/// Fails if the string does not have the capacity for `num` additional chars.
pub fn pad<LenT, S>(s: &mut StringInner<LenT, S>, char: char, num: usize)
where
    LenT: LenType,
    S: StringStorage,
{
    debug_assert!(s.len() + num <= s.capacity());

    for _ in 0..num {
        s.push(char).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding() {
        let mut a = String::<12>::new();
        pad(&mut a, ' ', 10);
        a.push_str("hi").unwrap();

        assert_eq!(a.as_str(), std::format!("{}hi", " ".repeat(10)));

        let mut b = String::<12>::new();
        b.push_str("hi").unwrap();
        pad(&mut b, ' ', 10);

        assert_eq!(b.as_str(), std::format!("hi{}", " ".repeat(10)));
    }

    #[test]
    fn f32() {
        let test = |f, exp_dots| {
            let (s, dots, len) = fmt_f32(f);
            assert_eq!(s.len(), 8);

            assert_eq!(s.as_str(), std::format!("{f:.1}{}", " ".repeat(8 - len)));
            assert_eq!(dots, exp_dots);
            assert_eq!(len, std::format!("{f:.1}").len());
        };

        test(0.0, 0b1000_0000);
        test(3.2, 0b1000_0000);
        test(32.0, 0b0100_0000);
        test(32.3, 0b0100_0000);
        test(100.0, 0b0010_0000);
    }

    #[test]
    #[should_panic]
    fn neg_f32() {
        fmt_f32(-1.0);
    }

    #[test]
    #[should_panic]
    fn big_f32() {
        fmt_f32(101.0);
    }

    #[test]
    fn u8() {
        let test = |u, after| {
            let s = fmt_pad_u8(u, Some(after));
            assert_eq!(s.len(), 8);
            assert_eq!(
                s.as_str(),
                std::format!(
                    "{}{u}{}",
                    " ".repeat(after),
                    " ".repeat(8 - std::format!("{u}").len() - after)
                )
            );
        };

        test(10, 0);
        test(10, 1);
        test(55, 3);
        test(55, 6);
        test(100, 5);

        assert_eq!(fmt_pad_u8(10, None), fmt_pad_u8(10, Some(0)));
    }

    #[test]
    #[should_panic]
    fn bad_after_u8() {
        fmt_pad_u8(10, Some(7));
    }

    #[test]
    #[should_panic]
    fn big_u8() {
        fmt_pad_u8(101, None);
    }
}
