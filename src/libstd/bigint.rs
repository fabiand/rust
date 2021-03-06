// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!

A Big integer (signed version: BigInt, unsigned version: BigUint).

A BigUint is represented as an array of BigDigits.
A BigInt is a combination of BigUint and Sign.
*/

use core::cmp::{Eq, Ord};
use core::num::{IntConvertible, Zero, One};
use core::*;

/**
A BigDigit is a BigUint's composing element.

A BigDigit is half the size of machine word size.
*/
#[cfg(target_arch = "x86")]
#[cfg(target_arch = "arm")]
#[cfg(target_arch = "mips")]
pub type BigDigit = u16;

/**
A BigDigit is a BigUint's composing element.

A BigDigit is half the size of machine word size.
*/
#[cfg(target_arch = "x86_64")]
pub type BigDigit = u32;

pub mod BigDigit {
    use bigint::BigDigit;

    #[cfg(target_arch = "x86")]
    #[cfg(target_arch = "arm")]
    #[cfg(target_arch = "mips")]
    pub const bits: uint = 16;

    #[cfg(target_arch = "x86_64")]
    pub const bits: uint = 32;

    pub const base: uint = 1 << bits;
    priv const hi_mask: uint = (-1 as uint) << bits;
    priv const lo_mask: uint = (-1 as uint) >> bits;

    priv pure fn get_hi(n: uint) -> BigDigit { (n >> bits) as BigDigit }
    priv pure fn get_lo(n: uint) -> BigDigit { (n & lo_mask) as BigDigit }

    /// Split one machine sized unsigned integer into two BigDigits.
    pub pure fn from_uint(n: uint) -> (BigDigit, BigDigit) {
        (get_hi(n), get_lo(n))
    }

    /// Join two BigDigits into one machine sized unsigned integer
    pub pure fn to_uint(hi: BigDigit, lo: BigDigit) -> uint {
        (lo as uint) | ((hi as uint) << bits)
    }
}

/**
A big unsigned integer type.

A BigUint-typed value BigUint { data: @[a, b, c] } represents a number
(a + b * BigDigit::base + c * BigDigit::base^2).
*/
pub struct BigUint {
    priv data: ~[BigDigit]
}

impl Eq for BigUint {
    pure fn eq(&self, other: &BigUint) -> bool { self.cmp(other) == 0 }
    pure fn ne(&self, other: &BigUint) -> bool { self.cmp(other) != 0 }
}

impl Ord for BigUint {
    pure fn lt(&self, other: &BigUint) -> bool { self.cmp(other) <  0 }
    pure fn le(&self, other: &BigUint) -> bool { self.cmp(other) <= 0 }
    pure fn ge(&self, other: &BigUint) -> bool { self.cmp(other) >= 0 }
    pure fn gt(&self, other: &BigUint) -> bool { self.cmp(other) >  0 }
}

impl ToStr for BigUint {
    pure fn to_str(&self) -> ~str { self.to_str_radix(10) }
}

impl from_str::FromStr for BigUint {
    static pure fn from_str(s: &str) -> Option<BigUint> {
        BigUint::from_str_radix(s, 10)
    }
}

impl Shl<uint, BigUint> for BigUint {
    pure fn shl(&self, rhs: &uint) -> BigUint {
        let n_unit = *rhs / BigDigit::bits;
        let n_bits = *rhs % BigDigit::bits;
        return self.shl_unit(n_unit).shl_bits(n_bits);
    }
}

impl Shr<uint, BigUint> for BigUint {
    pure fn shr(&self, rhs: &uint) -> BigUint {
        let n_unit = *rhs / BigDigit::bits;
        let n_bits = *rhs % BigDigit::bits;
        return self.shr_unit(n_unit).shr_bits(n_bits);
    }
}

impl Zero for BigUint {
    static pure fn zero() -> BigUint { BigUint::new(~[]) }
}

impl One for BigUint {
    static pub pure fn one() -> BigUint { BigUint::new(~[1]) }
}

impl Add<BigUint, BigUint> for BigUint {
    pure fn add(&self, other: &BigUint) -> BigUint {
        let new_len = uint::max(self.data.len(), other.data.len());

        let mut carry = 0;
        let sum = do vec::from_fn(new_len) |i| {
            let ai = if i < self.data.len()  { self.data[i]  } else { 0 };
            let bi = if i < other.data.len() { other.data[i] } else { 0 };
            let (hi, lo) = BigDigit::from_uint(
                (ai as uint) + (bi as uint) + (carry as uint)
            );
            carry = hi;
            lo
        };
        if carry == 0 { return BigUint::new(sum) };
        return BigUint::new(sum + [carry]);
    }
}

impl Sub<BigUint, BigUint> for BigUint {
    pure fn sub(&self, other: &BigUint) -> BigUint {
        let new_len = uint::max(self.data.len(), other.data.len());

        let mut borrow = 0;
        let diff = do vec::from_fn(new_len) |i| {
            let ai = if i < self.data.len()  { self.data[i]  } else { 0 };
            let bi = if i < other.data.len() { other.data[i] } else { 0 };
            let (hi, lo) = BigDigit::from_uint(
                (BigDigit::base) +
                (ai as uint) - (bi as uint) - (borrow as uint)
            );
            /*
            hi * (base) + lo == 1*(base) + ai - bi - borrow
            => ai - bi - borrow < 0 <=> hi == 0
            */
            borrow = if hi == 0 { 1 } else { 0 };
            lo
        };

        fail_unless!(borrow == 0);     // <=> fail_unless!((self >= other));
        return BigUint::new(diff);
    }
}

impl Mul<BigUint, BigUint> for BigUint {
    pure fn mul(&self, other: &BigUint) -> BigUint {
        if self.is_zero() || other.is_zero() { return Zero::zero(); }

        let s_len = self.data.len(), o_len = other.data.len();
        if s_len == 1 { return mul_digit(other, self.data[0]);  }
        if o_len == 1 { return mul_digit(self,  other.data[0]); }

        // Using Karatsuba multiplication
        // (a1 * base + a0) * (b1 * base + b0)
        // = a1*b1 * base^2 +
        //   (a1*b1 + a0*b0 - (a1-b0)*(b1-a0)) * base +
        //   a0*b0
        let half_len = uint::max(s_len, o_len) / 2;
        let (sHi, sLo) = cut_at(self,  half_len);
        let (oHi, oLo) = cut_at(other, half_len);

        let ll = sLo * oLo;
        let hh = sHi * oHi;
        let mm = {
            let (s1, n1) = sub_sign(sHi, sLo);
            let (s2, n2) = sub_sign(oHi, oLo);
            if s1 * s2 < 0 {
                hh + ll + (n1 * n2)
            } else if s1 * s2 > 0 {
                hh + ll - (n1 * n2)
            } else {
                hh + ll
            }
        };

        return ll + mm.shl_unit(half_len) + hh.shl_unit(half_len * 2);

        pure fn mul_digit(a: &BigUint, n: BigDigit) -> BigUint {
            if n == 0 { return Zero::zero(); }
            if n == 1 { return copy *a; }

            let mut carry = 0;
            let prod = do vec::map(a.data) |ai| {
                let (hi, lo) = BigDigit::from_uint(
                    (*ai as uint) * (n as uint) + (carry as uint)
                );
                carry = hi;
                lo
            };
            if carry == 0 { return BigUint::new(prod) };
            return BigUint::new(prod + [carry]);
        }

        pure fn cut_at(a: &BigUint, n: uint) -> (BigUint, BigUint) {
            let mid = uint::min(a.data.len(), n);
            return (BigUint::from_slice(vec::slice(a.data, mid,
                                                   a.data.len())),
                    BigUint::from_slice(vec::slice(a.data, 0, mid)));
        }

        pure fn sub_sign(a: BigUint, b: BigUint) -> (int, BigUint) {
            match a.cmp(&b) {
                s if s < 0 => (s, b - a),
                s if s > 0 => (s, a - b),
                _          => (0, Zero::zero())
            }
        }
    }
}

impl Div<BigUint, BigUint> for BigUint {
    pure fn div(&self, other: &BigUint) -> BigUint {
        let (d, _) = self.divmod(other);
        return d;
    }
}

impl Modulo<BigUint, BigUint> for BigUint {
    pure fn modulo(&self, other: &BigUint) -> BigUint {
        let (_, m) = self.divmod(other);
        return m;
    }
}

impl Neg<BigUint> for BigUint {
    pure fn neg(&self) -> BigUint { fail!() }
}

impl IntConvertible for BigUint {
    pure fn to_int(&self) -> int {
        uint::min(self.to_uint(), int::max_value as uint) as int
    }

    static pure fn from_int(n: int) -> BigUint {
        if (n < 0) { Zero::zero() } else { BigUint::from_uint(n as uint) }
    }
}

pub impl BigUint {
    /// Creates and initializes an BigUint.
    static pub pure fn new(v: ~[BigDigit]) -> BigUint {
        // omit trailing zeros
        let new_len = v.rposition(|n| *n != 0).map_default(0, |p| *p + 1);

        if new_len == v.len() { return BigUint { data: v }; }
        let mut v = v;
        unsafe { v.truncate(new_len); }
        return BigUint { data: v };
    }

    /// Creates and initializes an BigUint.
    static pub pure fn from_uint(n: uint) -> BigUint {
        match BigDigit::from_uint(n) {
            (0,  0)  => Zero::zero(),
            (0,  n0) => BigUint::new(~[n0]),
            (n1, n0) => BigUint::new(~[n0, n1])
        }
    }

    /// Creates and initializes an BigUint.
    static pub pure fn from_slice(slice: &[BigDigit]) -> BigUint {
        return BigUint::new(vec::from_slice(slice));
    }

    /// Creates and initializes an BigUint.
    static pub pure fn from_str_radix(s: &str, radix: uint)
        -> Option<BigUint> {
        BigUint::parse_bytes(str::to_bytes(s), radix)
    }

    /// Creates and initializes an BigUint.
    static pub pure fn parse_bytes(buf: &[u8], radix: uint)
        -> Option<BigUint> {
        let (base, unit_len) = get_radix_base(radix);
        let base_num: BigUint = BigUint::from_uint(base);

        let mut end             = buf.len();
        let mut n: BigUint      = Zero::zero();
        let mut power: BigUint  = One::one();
        loop {
            let start = uint::max(end, unit_len) - unit_len;
            match uint::parse_bytes(vec::slice(buf, start, end), radix) {
                Some(d) => n += BigUint::from_uint(d) * power,
                None    => return None
            }
            if end <= unit_len {
                return Some(n);
            }
            end -= unit_len;
            power *= base_num;
        }
    }

    pure fn abs(&self) -> BigUint { copy *self }

    /// Compare two BigUint value.
    pure fn cmp(&self, other: &BigUint) -> int {
        let s_len = self.data.len(), o_len = other.data.len();
        if s_len < o_len { return -1; }
        if s_len > o_len { return  1;  }

        for vec::rev_eachi(self.data) |i, elm| {
            match (*elm, other.data[i]) {
                (l, r) if l < r => return -1,
                (l, r) if l > r => return  1,
                _               => loop
            };
        }
        return 0;
    }

    pure fn divmod(&self, other: &BigUint) -> (BigUint, BigUint) {
        if other.is_zero() { fail!() }
        if self.is_zero() { return (Zero::zero(), Zero::zero()); }
        if *other == One::one() { return (copy *self, Zero::zero()); }

        match self.cmp(other) {
            s if s < 0 => return (Zero::zero(), copy *self),
            0          => return (One::one(), Zero::zero()),
            _          => {} // Do nothing
        }

        let mut shift = 0;
        let mut n = *other.data.last();
        while n < (1 << BigDigit::bits - 2) {
            n <<= 1;
            shift += 1;
        }
        fail_unless!(shift < BigDigit::bits);
        let (d, m) = divmod_inner(self << shift, other << shift);
        return (d, m >> shift);

        pure fn divmod_inner(a: BigUint, b: BigUint) -> (BigUint, BigUint) {
            let mut r = a;
            let mut d = Zero::zero::<BigUint>();
            let mut n = 1;
            while r >= b {
                let mut (d0, d_unit, b_unit) = div_estimate(&r, &b, n);
                let mut prod = b * d0;
                while prod > r {
                    d0   -= d_unit;
                    prod -= b_unit;
                }
                if d0.is_zero() {
                    n = 2;
                    loop;
                }
                n = 1;
                d += d0;
                r -= prod;
            }
            return (d, r);
        }

        pure fn div_estimate(a: &BigUint, b: &BigUint, n: uint)
            -> (BigUint, BigUint, BigUint) {
            if a.data.len() < n {
                return (Zero::zero(), Zero::zero(), copy *a);
            }

            let an = vec::slice(a.data, a.data.len() - n, a.data.len());
            let bn = *b.data.last();
            let mut d = ~[];
            let mut carry = 0;
            for vec::rev_each(an) |elt| {
                let ai = BigDigit::to_uint(carry, *elt);
                let di = ai / (bn as uint);
                fail_unless!(di < BigDigit::base);
                carry = (ai % (bn as uint)) as BigDigit;
                d = ~[di as BigDigit] + d;
            }

            let shift = (a.data.len() - an.len()) - (b.data.len() - 1);
            if shift == 0 {
                return (BigUint::new(d), One::one(), copy *b);
            }
            return (BigUint::from_slice(d).shl_unit(shift),
                    One::one::<BigUint>().shl_unit(shift),
                    b.shl_unit(shift));
        }
    }

    pure fn quot(&self, other: &BigUint) -> BigUint {
        let (q, _) = self.quotrem(other);
        return q;
    }
    pure fn rem(&self, other: &BigUint) -> BigUint {
        let (_, r) = self.quotrem(other);
        return r;
    }
    pure fn quotrem(&self, other: &BigUint) -> (BigUint, BigUint) {
        self.divmod(other)
    }

    pure fn is_zero(&self) -> bool { self.data.is_empty() }
    pure fn is_not_zero(&self) -> bool { !self.data.is_empty() }
    pure fn is_positive(&self) -> bool { self.is_not_zero() }
    pure fn is_negative(&self) -> bool { false }
    pure fn is_nonpositive(&self) -> bool { self.is_zero() }
    pure fn is_nonnegative(&self) -> bool { true }

    pure fn to_uint(&self) -> uint {
        match self.data.len() {
            0 => 0,
            1 => self.data[0] as uint,
            2 => BigDigit::to_uint(self.data[1], self.data[0]),
            _ => uint::max_value
        }
    }

    pure fn to_str_radix(&self, radix: uint) -> ~str {
        fail_unless!(1 < radix && radix <= 16);
        let (base, max_len) = get_radix_base(radix);
        if base == BigDigit::base {
            return fill_concat(self.data, radix, max_len)
        }
        return fill_concat(convert_base(copy *self, base), radix, max_len);

        pure fn convert_base(n: BigUint, base: uint) -> ~[BigDigit] {
            let divider    = BigUint::from_uint(base);
            let mut result = ~[];
            let mut r      = n;
            while r > divider {
                let (d, r0) = r.divmod(&divider);
                result += [r0.to_uint() as BigDigit];
                r = d;
            }
            if r.is_not_zero() {
                result += [r.to_uint() as BigDigit];
            }
            return result;
        }

        pure fn fill_concat(v: &[BigDigit], radix: uint, l: uint) -> ~str {
            if v.is_empty() { return ~"0" }
            str::trim_left_chars(str::concat(vec::reversed(v).map(|n| {
                let s = uint::to_str_radix(*n as uint, radix);
                str::from_chars(vec::from_elem(l - s.len(), '0')) + s
            })), ['0'])
        }
    }

    priv pure fn shl_unit(self, n_unit: uint) -> BigUint {
        if n_unit == 0 || self.is_zero() { return self; }

        return BigUint::new(vec::from_elem(n_unit, 0) + self.data);
    }

    priv pure fn shl_bits(self, n_bits: uint) -> BigUint {
        if n_bits == 0 || self.is_zero() { return self; }

        let mut carry = 0;
        let shifted = do vec::map(self.data) |elem| {
            let (hi, lo) = BigDigit::from_uint(
                (*elem as uint) << n_bits | (carry as uint)
            );
            carry = hi;
            lo
        };
        if carry == 0 { return BigUint::new(shifted); }
        return BigUint::new(shifted + [carry]);
    }

    priv pure fn shr_unit(self, n_unit: uint) -> BigUint {
        if n_unit == 0 { return self; }
        if self.data.len() < n_unit { return Zero::zero(); }
        return BigUint::from_slice(
            vec::slice(self.data, n_unit, self.data.len())
        );
    }

    priv pure fn shr_bits(self, n_bits: uint) -> BigUint {
        if n_bits == 0 || self.data.is_empty() { return self; }

        let mut borrow = 0;
        let mut shifted = ~[];
        for vec::rev_each(self.data) |elem| {
            shifted = ~[(*elem >> n_bits) | borrow] + shifted;
            borrow = *elem << (uint::bits - n_bits);
        }
        return BigUint::new(shifted);
    }
}

#[cfg(target_arch = "x86_64")]
priv pure fn get_radix_base(radix: uint) -> (uint, uint) {
    fail_unless!(1 < radix && radix <= 16);
    match radix {
        2  => (4294967296, 32),
        3  => (3486784401, 20),
        4  => (4294967296, 16),
        5  => (1220703125, 13),
        6  => (2176782336, 12),
        7  => (1977326743, 11),
        8  => (1073741824, 10),
        9  => (3486784401, 10),
        10 => (1000000000, 9),
        11 => (2357947691, 9),
        12 => (429981696,  8),
        13 => (815730721,  8),
        14 => (1475789056, 8),
        15 => (2562890625, 8),
        16 => (4294967296, 8),
        _  => fail!()
    }
}

#[cfg(target_arch = "arm")]
#[cfg(target_arch = "x86")]
#[cfg(target_arch = "mips")]
priv pure fn get_radix_base(radix: uint) -> (uint, uint) {
    fail_unless!(1 < radix && radix <= 16);
    match radix {
        2  => (65536, 16),
        3  => (59049, 10),
        4  => (65536, 8),
        5  => (15625, 6),
        6  => (46656, 6),
        7  => (16807, 5),
        8  => (32768, 5),
        9  => (59049, 5),
        10 => (10000, 4),
        11 => (14641, 4),
        12 => (20736, 4),
        13 => (28561, 4),
        14 => (38416, 4),
        15 => (50625, 4),
        16 => (65536, 4),
        _  => fail!()
    }
}

/// A Sign is a BigInt's composing element.
pub enum Sign { Minus, Zero, Plus }

impl Eq for Sign {
    pure fn eq(&self, other: &Sign) -> bool { self.cmp(other) == 0 }
    pure fn ne(&self, other: &Sign) -> bool { self.cmp(other) != 0 }
}

impl Ord for Sign {
    pure fn lt(&self, other: &Sign) -> bool { self.cmp(other) <  0 }
    pure fn le(&self, other: &Sign) -> bool { self.cmp(other) <= 0 }
    pure fn ge(&self, other: &Sign) -> bool { self.cmp(other) >= 0 }
    pure fn gt(&self, other: &Sign) -> bool { self.cmp(other) >  0 }
}

pub impl Sign {
    /// Compare two Sign.
    pure fn cmp(&self, other: &Sign) -> int {
        match (*self, *other) {
          (Minus, Minus) | (Zero,  Zero) | (Plus, Plus) =>  0,
          (Minus, Zero)  | (Minus, Plus) | (Zero, Plus) => -1,
          _                                             =>  1
        }
    }

    /// Negate Sign value.
    pure fn neg(&self) -> Sign {
        match *self {
          Minus => Plus,
          Zero  => Zero,
          Plus  => Minus
        }
    }
}

/// A big signed integer type.
pub struct BigInt {
    priv sign: Sign,
    priv data: BigUint
}

impl Eq for BigInt {
    pure fn eq(&self, other: &BigInt) -> bool { self.cmp(other) == 0 }
    pure fn ne(&self, other: &BigInt) -> bool { self.cmp(other) != 0 }
}

impl Ord for BigInt {
    pure fn lt(&self, other: &BigInt) -> bool { self.cmp(other) <  0 }
    pure fn le(&self, other: &BigInt) -> bool { self.cmp(other) <= 0 }
    pure fn ge(&self, other: &BigInt) -> bool { self.cmp(other) >= 0 }
    pure fn gt(&self, other: &BigInt) -> bool { self.cmp(other) >  0 }
}

impl ToStr for BigInt {
    pure fn to_str(&self) -> ~str { self.to_str_radix(10) }
}

impl from_str::FromStr for BigInt {
    static pure fn from_str(s: &str) -> Option<BigInt> {
        BigInt::from_str_radix(s, 10)
    }
}

impl Shl<uint, BigInt> for BigInt {
    pure fn shl(&self, rhs: &uint) -> BigInt {
        BigInt::from_biguint(self.sign, self.data << *rhs)
    }
}

impl Shr<uint, BigInt> for BigInt {
    pure fn shr(&self, rhs: &uint) -> BigInt {
        BigInt::from_biguint(self.sign, self.data >> *rhs)
    }
}

impl Zero for BigInt {
    static pub pure fn zero() -> BigInt {
        BigInt::from_biguint(Zero, Zero::zero())
    }
}

impl One for BigInt {
    static pub pure fn one() -> BigInt {
        BigInt::from_biguint(Plus, One::one())
    }
}

impl Add<BigInt, BigInt> for BigInt {
    pure fn add(&self, other: &BigInt) -> BigInt {
        match (self.sign, other.sign) {
            (Zero, _)      => copy *other,
            (_,    Zero)   => copy *self,
            (Plus, Plus)   => BigInt::from_biguint(Plus,
                                                   self.data + other.data),
            (Plus, Minus)  => self - (-*other),
            (Minus, Plus)  => other - (-*self),
            (Minus, Minus) => -((-self) + (-*other))
        }
    }
}

impl Sub<BigInt, BigInt> for BigInt {
    pure fn sub(&self, other: &BigInt) -> BigInt {
        match (self.sign, other.sign) {
            (Zero, _)    => -other,
            (_,    Zero) => copy *self,
            (Plus, Plus) => match self.data.cmp(&other.data) {
                s if s < 0 =>
                    BigInt::from_biguint(Minus, other.data - self.data),
                s if s > 0 =>
                    BigInt::from_biguint(Plus, self.data - other.data),
                _ =>
                    Zero::zero()
            },
            (Plus, Minus) => self + (-*other),
            (Minus, Plus) => -((-self) + *other),
            (Minus, Minus) => (-other) - (-*self)
        }
    }
}

impl Mul<BigInt, BigInt> for BigInt {
    pure fn mul(&self, other: &BigInt) -> BigInt {
        match (self.sign, other.sign) {
            (Zero, _)     | (_,     Zero)  => Zero::zero(),
            (Plus, Plus)  | (Minus, Minus) => {
                BigInt::from_biguint(Plus, self.data * other.data)
            },
            (Plus, Minus) | (Minus, Plus) => {
                BigInt::from_biguint(Minus, self.data * other.data)
            }
        }
    }
}

impl Div<BigInt, BigInt> for BigInt {
    pure fn div(&self, other: &BigInt) -> BigInt {
        let (d, _) = self.divmod(other);
        return d;
    }
}

impl Modulo<BigInt, BigInt> for BigInt {
    pure fn modulo(&self, other: &BigInt) -> BigInt {
        let (_, m) = self.divmod(other);
        return m;
    }
}

impl Neg<BigInt> for BigInt {
    pure fn neg(&self) -> BigInt {
        BigInt::from_biguint(self.sign.neg(), copy self.data)
    }
}

impl IntConvertible for BigInt {
    pure fn to_int(&self) -> int {
        match self.sign {
            Plus  => uint::min(self.to_uint(), int::max_value as uint) as int,
            Zero  => 0,
            Minus => uint::min((-self).to_uint(),
                               (int::max_value as uint) + 1) as int
        }
    }

    static pure fn from_int(n: int) -> BigInt {
        if n > 0 {
           return BigInt::from_biguint(Plus,  BigUint::from_uint(n as uint));
        }
        if n < 0 {
            return BigInt::from_biguint(
                Minus, BigUint::from_uint(uint::max_value - (n as uint) + 1)
            );
        }
        return Zero::zero();
    }
}

pub impl BigInt {
    /// Creates and initializes an BigInt.
    static pub pure fn new(sign: Sign, v: ~[BigDigit]) -> BigInt {
        BigInt::from_biguint(sign, BigUint::new(v))
    }

    /// Creates and initializes an BigInt.
    static pub pure fn from_biguint(sign: Sign, data: BigUint) -> BigInt {
        if sign == Zero || data.is_zero() {
            return BigInt { sign: Zero, data: Zero::zero() };
        }
        return BigInt { sign: sign, data: data };
    }

    /// Creates and initializes an BigInt.
    static pub pure fn from_uint(n: uint) -> BigInt {
        if n == 0 { return Zero::zero(); }
        return BigInt::from_biguint(Plus, BigUint::from_uint(n));
    }

    /// Creates and initializes an BigInt.
    static pub pure fn from_slice(sign: Sign, slice: &[BigDigit]) -> BigInt {
        BigInt::from_biguint(sign, BigUint::from_slice(slice))
    }

    /// Creates and initializes an BigInt.
    static pub pure fn from_str_radix(s: &str, radix: uint)
        -> Option<BigInt> {
        BigInt::parse_bytes(str::to_bytes(s), radix)
    }

    /// Creates and initializes an BigInt.
    static pub pure fn parse_bytes(buf: &[u8], radix: uint)
        -> Option<BigInt> {
        if buf.is_empty() { return None; }
        let mut sign  = Plus;
        let mut start = 0;
        if buf[0] == ('-' as u8) {
            sign  = Minus;
            start = 1;
        }
        return BigUint::parse_bytes(vec::slice(buf, start, buf.len()), radix)
            .map(|bu| BigInt::from_biguint(sign, *bu));
    }

    pure fn abs(&self) -> BigInt {
        BigInt::from_biguint(Plus, copy self.data)
    }

    pure fn cmp(&self, other: &BigInt) -> int {
        let ss = self.sign, os = other.sign;
        if ss < os { return -1; }
        if ss > os { return  1; }

        fail_unless!(ss == os);
        match ss {
            Zero  => 0,
            Plus  => self.data.cmp(&other.data),
            Minus => self.data.cmp(&other.data).neg(),
        }
    }

    pure fn divmod(&self, other: &BigInt) -> (BigInt, BigInt) {
        // m.sign == other.sign
        let (d_ui, m_ui) = self.data.divmod(&other.data);
        let d = BigInt::from_biguint(Plus, d_ui),
            m = BigInt::from_biguint(Plus, m_ui);
        match (self.sign, other.sign) {
            (_,    Zero)   => fail!(),
            (Plus, Plus)  | (Zero, Plus)  => (d, m),
            (Plus, Minus) | (Zero, Minus) => if m.is_zero() {
                (-d, Zero::zero())
            } else {
                (-d - One::one(), m + *other)
            },
            (Minus, Plus) => if m.is_zero() {
                (-d, Zero::zero())
            } else {
                (-d - One::one(), other - m)
            },
            (Minus, Minus) => (d, -m)
        }
    }

    pure fn quot(&self, other: &BigInt) -> BigInt {
        let (q, _) = self.quotrem(other);
        return q;
    }
    pure fn rem(&self, other: &BigInt) -> BigInt {
        let (_, r) = self.quotrem(other);
        return r;
    }

    pure fn quotrem(&self, other: &BigInt) -> (BigInt, BigInt) {
        // r.sign == self.sign
        let (q_ui, r_ui) = self.data.quotrem(&other.data);
        let q = BigInt::from_biguint(Plus, q_ui);
        let r = BigInt::from_biguint(Plus, r_ui);
        match (self.sign, other.sign) {
            (_,    Zero)   => fail!(),
            (Plus, Plus)  | (Zero, Plus)  => ( q,  r),
            (Plus, Minus) | (Zero, Minus) => (-q,  r),
            (Minus, Plus)                 => (-q, -r),
            (Minus, Minus)                => ( q, -r)
        }
    }

    pure fn is_zero(&self) -> bool { self.sign == Zero }
    pure fn is_not_zero(&self) -> bool { self.sign != Zero }
    pure fn is_positive(&self) -> bool { self.sign == Plus }
    pure fn is_negative(&self) -> bool { self.sign == Minus }
    pure fn is_nonpositive(&self) -> bool { self.sign != Plus }
    pure fn is_nonnegative(&self) -> bool { self.sign != Minus }

    pure fn to_uint(&self) -> uint {
        match self.sign {
            Plus  => self.data.to_uint(),
            Zero  => 0,
            Minus => 0
        }
    }

    pure fn to_str_radix(&self, radix: uint) -> ~str {
        match self.sign {
            Plus  => self.data.to_str_radix(radix),
            Zero  => ~"0",
            Minus => ~"-" + self.data.to_str_radix(radix)
        }
    }
}

#[cfg(test)]
mod biguint_tests {

    use core::*;
    use core::num::{IntConvertible, Zero, One};
    use super::{BigInt, BigUint, BigDigit};

    #[test]
    fn test_from_slice() {
        fn check(slice: &[BigDigit], data: &[BigDigit]) {
            fail_unless!(data == BigUint::from_slice(slice).data);
        }
        check(~[1], ~[1]);
        check(~[0, 0, 0], ~[]);
        check(~[1, 2, 0, 0], ~[1, 2]);
        check(~[0, 0, 1, 2], ~[0, 0, 1, 2]);
        check(~[0, 0, 1, 2, 0, 0], ~[0, 0, 1, 2]);
        check(~[-1], ~[-1]);
    }

    #[test]
    fn test_cmp() {
        let data = [ &[], &[1], &[2], &[-1], &[0, 1], &[2, 1], &[1, 1, 1]  ]
            .map(|v| BigUint::from_slice(*v));
        for data.eachi |i, ni| {
            for vec::slice(data, i, data.len()).eachi |j0, nj| {
                let j = j0 + i;
                if i == j {
                    fail_unless!(ni.cmp(nj) == 0);
                    fail_unless!(nj.cmp(ni) == 0);
                    fail_unless!(ni == nj);
                    fail_unless!(!(ni != nj));
                    fail_unless!(ni <= nj);
                    fail_unless!(ni >= nj);
                    fail_unless!(!(ni < nj));
                    fail_unless!(!(ni > nj));
                } else {
                    fail_unless!(ni.cmp(nj) < 0);
                    fail_unless!(nj.cmp(ni) > 0);

                    fail_unless!(!(ni == nj));
                    fail_unless!(ni != nj);

                    fail_unless!(ni <= nj);
                    fail_unless!(!(ni >= nj));
                    fail_unless!(ni < nj);
                    fail_unless!(!(ni > nj));

                    fail_unless!(!(nj <= ni));
                    fail_unless!(nj >= ni);
                    fail_unless!(!(nj < ni));
                    fail_unless!(nj > ni);
                }
            }
        }
    }

    #[test]
    fn test_shl() {
        fn check(v: ~[BigDigit], shift: uint, ans: ~[BigDigit]) {
            fail_unless!(BigUint::new(v) << shift == BigUint::new(ans));
        }

        check(~[], 3, ~[]);
        check(~[1, 1, 1], 3, ~[1 << 3, 1 << 3, 1 << 3]);
        check(~[1 << (BigDigit::bits - 2)], 2, ~[0, 1]);
        check(~[1 << (BigDigit::bits - 2)], 3, ~[0, 2]);
        check(~[1 << (BigDigit::bits - 2)], 3 + BigDigit::bits, ~[0, 0, 2]);

        test_shl_bits();

        #[cfg(target_arch = "x86_64")]
        fn test_shl_bits() {
            check(~[0x7654_3210, 0xfedc_ba98,
                    0x7654_3210, 0xfedc_ba98], 4,
                  ~[0x6543_2100, 0xedcb_a987,
                    0x6543_210f, 0xedcb_a987, 0xf]);
            check(~[0x2222_1111, 0x4444_3333,
                    0x6666_5555, 0x8888_7777], 16,
                  ~[0x1111_0000, 0x3333_2222,
                    0x5555_4444, 0x7777_6666, 0x8888]);
        }

        #[cfg(target_arch = "arm")]
        #[cfg(target_arch = "x86")]
        #[cfg(target_arch = "mips")]
        fn test_shl_bits() {
            check(~[0x3210, 0x7654, 0xba98, 0xfedc,
                    0x3210, 0x7654, 0xba98, 0xfedc], 4,
                  ~[0x2100, 0x6543, 0xa987, 0xedcb,
                    0x210f, 0x6543, 0xa987, 0xedcb, 0xf]);
            check(~[0x1111, 0x2222, 0x3333, 0x4444,
                    0x5555, 0x6666, 0x7777, 0x8888], 16,
                  ~[0x0000, 0x1111, 0x2222, 0x3333,
                    0x4444, 0x5555, 0x6666, 0x7777, 0x8888]);
        }

    }

    #[test]
    #[ignore(cfg(target_arch = "x86"))]
    #[ignore(cfg(target_arch = "arm"))]
    #[ignore(cfg(target_arch = "mips"))]
    fn test_shr() {
        fn check(v: ~[BigDigit], shift: uint, ans: ~[BigDigit]) {
            fail_unless!(BigUint::new(v) >> shift == BigUint::new(ans));
        }

        check(~[], 3, ~[]);
        check(~[1, 1, 1], 3,
              ~[1 << (BigDigit::bits - 3), 1 << (BigDigit::bits - 3)]);
        check(~[1 << 2], 2, ~[1]);
        check(~[1, 2], 3, ~[1 << (BigDigit::bits - 2)]);
        check(~[1, 1, 2], 3 + BigDigit::bits, ~[1 << (BigDigit::bits - 2)]);
        test_shr_bits();

        #[cfg(target_arch = "x86_64")]
        fn test_shr_bits() {
            check(~[0x6543_2100, 0xedcb_a987,
                    0x6543_210f, 0xedcb_a987, 0xf], 4,
                  ~[0x7654_3210, 0xfedc_ba98,
                    0x7654_3210, 0xfedc_ba98]);
            check(~[0x1111_0000, 0x3333_2222,
                    0x5555_4444, 0x7777_6666, 0x8888], 16,
                  ~[0x2222_1111, 0x4444_3333,
                    0x6666_5555, 0x8888_7777]);
        }

        #[cfg(target_arch = "arm")]
        #[cfg(target_arch = "x86")]
        #[cfg(target_arch = "mips")]
        fn test_shr_bits() {
            check(~[0x2100, 0x6543, 0xa987, 0xedcb,
                    0x210f, 0x6543, 0xa987, 0xedcb, 0xf], 4,
                  ~[0x3210, 0x7654, 0xba98, 0xfedc,
                    0x3210, 0x7654, 0xba98, 0xfedc]);
            check(~[0x0000, 0x1111, 0x2222, 0x3333,
                    0x4444, 0x5555, 0x6666, 0x7777, 0x8888], 16,
                  ~[0x1111, 0x2222, 0x3333, 0x4444,
                    0x5555, 0x6666, 0x7777, 0x8888]);
        }
    }

    #[test]
    fn test_convert_int() {
        fn check(v: ~[BigDigit], i: int) {
            let b = BigUint::new(v);
            fail_unless!(b == IntConvertible::from_int(i));
            fail_unless!(b.to_int() == i);
        }

        check(~[], 0);
        check(~[1], 1);
        check(~[-1], (uint::max_value >> BigDigit::bits) as int);
        check(~[ 0,  1], ((uint::max_value >> BigDigit::bits) + 1) as int);
        check(~[-1, -1 >> 1], int::max_value);

        fail_unless!(BigUint::new(~[0, -1]).to_int() == int::max_value);
        fail_unless!(BigUint::new(~[0, 0, 1]).to_int() == int::max_value);
        fail_unless!(BigUint::new(~[0, 0, -1]).to_int() == int::max_value);
    }

    #[test]
    fn test_convert_uint() {
        fn check(v: ~[BigDigit], u: uint) {
            let b = BigUint::new(v);
            fail_unless!(b == BigUint::from_uint(u));
            fail_unless!(b.to_uint() == u);
        }

        check(~[], 0);
        check(~[ 1], 1);
        check(~[-1], uint::max_value >> BigDigit::bits);
        check(~[ 0,  1], (uint::max_value >> BigDigit::bits) + 1);
        check(~[ 0, -1], uint::max_value << BigDigit::bits);
        check(~[-1, -1], uint::max_value);

        fail_unless!(BigUint::new(~[0, 0, 1]).to_uint()  == uint::max_value);
        fail_unless!(BigUint::new(~[0, 0, -1]).to_uint() == uint::max_value);
    }

    const sum_triples: &'static [(&'static [BigDigit],
                                 &'static [BigDigit],
                                 &'static [BigDigit])] = &[
        (&[],          &[],       &[]),
        (&[],          &[ 1],     &[ 1]),
        (&[ 1],        &[ 1],     &[ 2]),
        (&[ 1],        &[ 1,  1], &[ 2,  1]),
        (&[ 1],        &[-1],     &[ 0,  1]),
        (&[ 1],        &[-1, -1], &[ 0,  0, 1]),
        (&[-1, -1],    &[-1, -1], &[-2, -1, 1]),
        (&[ 1,  1, 1], &[-1, -1], &[ 0,  1, 2]),
        (&[ 2,  2, 1], &[-1, -2], &[ 1,  1, 2])
    ];

    #[test]
    fn test_add() {
        for sum_triples.each |elm| {
            let (aVec, bVec, cVec) = *elm;
            let a = BigUint::from_slice(aVec);
            let b = BigUint::from_slice(bVec);
            let c = BigUint::from_slice(cVec);

            fail_unless!(a + b == c);
            fail_unless!(b + a == c);
        }
    }

    #[test]
    fn test_sub() {
        for sum_triples.each |elm| {
            let (aVec, bVec, cVec) = *elm;
            let a = BigUint::from_slice(aVec);
            let b = BigUint::from_slice(bVec);
            let c = BigUint::from_slice(cVec);

            fail_unless!(c - a == b);
            fail_unless!(c - b == a);
        }
    }

    const mul_triples: &'static [(&'static [BigDigit],
                                 &'static [BigDigit],
                                 &'static [BigDigit])] = &[
        (&[],               &[],               &[]),
        (&[],               &[ 1],             &[]),
        (&[ 2],             &[],               &[]),
        (&[ 1],             &[ 1],             &[1]),
        (&[ 2],             &[ 3],             &[ 6]),
        (&[ 1],             &[ 1,  1,  1],     &[1, 1,  1]),
        (&[ 1,  2,  3],     &[ 3],             &[ 3,  6,  9]),
        (&[ 1,  1,  1],     &[-1],             &[-1, -1, -1]),
        (&[ 1,  2,  3],     &[-1],             &[-1, -2, -2, 2]),
        (&[ 1,  2,  3,  4], &[-1],             &[-1, -2, -2, -2, 3]),
        (&[-1],             &[-1],             &[ 1, -2]),
        (&[-1, -1],         &[-1],             &[ 1, -1, -2]),
        (&[-1, -1, -1],     &[-1],             &[ 1, -1, -1, -2]),
        (&[-1, -1, -1, -1], &[-1],             &[ 1, -1, -1, -1, -2]),
        (&[-1/2 + 1],       &[ 2],             &[ 0,  1]),
        (&[0, -1/2 + 1],    &[ 2],             &[ 0,  0,  1]),
        (&[ 1,  2],         &[ 1,  2,  3],     &[1, 4,  7,  6]),
        (&[-1, -1],         &[-1, -1, -1],     &[1, 0, -1, -2, -1]),
        (&[-1, -1, -1],     &[-1, -1, -1, -1], &[1, 0,  0, -1, -2, -1, -1]),
        (&[ 0,  0,  1],     &[ 1,  2,  3],     &[0, 0,  1,  2,  3]),
        (&[ 0,  0,  1],     &[ 0,  0,  0,  1], &[0, 0,  0,  0,  0,  1])
    ];

    const divmod_quadruples: &'static [(&'static [BigDigit],
                                       &'static [BigDigit],
                                       &'static [BigDigit],
                                       &'static [BigDigit])]
        = &[
            (&[ 1],        &[ 2], &[],               &[1]),
            (&[ 1,  1],    &[ 2], &[-1/2+1],         &[1]),
            (&[ 1,  1, 1], &[ 2], &[-1/2+1, -1/2+1], &[1]),
            (&[ 0,  1],    &[-1], &[1],              &[1]),
            (&[-1, -1],    &[-2], &[2, 1],           &[3])
        ];

    #[test]
    fn test_mul() {
        for mul_triples.each |elm| {
            let (aVec, bVec, cVec) = *elm;
            let a = BigUint::from_slice(aVec);
            let b = BigUint::from_slice(bVec);
            let c = BigUint::from_slice(cVec);

            fail_unless!(a * b == c);
            fail_unless!(b * a == c);
        }

        for divmod_quadruples.each |elm| {
            let (aVec, bVec, cVec, dVec) = *elm;
            let a = BigUint::from_slice(aVec);
            let b = BigUint::from_slice(bVec);
            let c = BigUint::from_slice(cVec);
            let d = BigUint::from_slice(dVec);

            fail_unless!(a == b * c + d);
            fail_unless!(a == c * b + d);
        }
    }

    #[test]
    fn test_divmod() {
        for mul_triples.each |elm| {
            let (aVec, bVec, cVec) = *elm;
            let a = BigUint::from_slice(aVec);
            let b = BigUint::from_slice(bVec);
            let c = BigUint::from_slice(cVec);

            if a.is_not_zero() {
                fail_unless!(c.divmod(&a) == (b, Zero::zero()));
            }
            if b.is_not_zero() {
                fail_unless!(c.divmod(&b) == (a, Zero::zero()));
            }
        }

        for divmod_quadruples.each |elm| {
            let (aVec, bVec, cVec, dVec) = *elm;
            let a = BigUint::from_slice(aVec);
            let b = BigUint::from_slice(bVec);
            let c = BigUint::from_slice(cVec);
            let d = BigUint::from_slice(dVec);

            if b.is_not_zero() { fail_unless!(a.divmod(&b) == (c, d)); }
        }
    }

    fn to_str_pairs() -> ~[ (BigUint, ~[(uint, ~str)]) ] {
        let bits = BigDigit::bits;
        ~[( Zero::zero(), ~[
            (2, ~"0"), (3, ~"0")
        ]), ( BigUint::from_slice([ 0xff ]), ~[
            (2,  ~"11111111"),
            (3,  ~"100110"),
            (4,  ~"3333"),
            (5,  ~"2010"),
            (6,  ~"1103"),
            (7,  ~"513"),
            (8,  ~"377"),
            (9,  ~"313"),
            (10, ~"255"),
            (11, ~"212"),
            (12, ~"193"),
            (13, ~"168"),
            (14, ~"143"),
            (15, ~"120"),
            (16, ~"ff")
        ]), ( BigUint::from_slice([ 0xfff ]), ~[
            (2,  ~"111111111111"),
            (4,  ~"333333"),
            (16, ~"fff")
        ]), ( BigUint::from_slice([ 1, 2 ]), ~[
            (2,
             ~"10" +
             str::from_chars(vec::from_elem(bits - 1, '0')) + "1"),
            (4,
             ~"2" +
             str::from_chars(vec::from_elem(bits / 2 - 1, '0')) + "1"),
            (10, match bits {
                32 => ~"8589934593", 16 => ~"131073", _ => fail!()
            }),
            (16,
             ~"2" +
             str::from_chars(vec::from_elem(bits / 4 - 1, '0')) + "1")
        ]), ( BigUint::from_slice([ 1, 2, 3 ]), ~[
            (2,
             ~"11" +
             str::from_chars(vec::from_elem(bits - 2, '0')) + "10" +
             str::from_chars(vec::from_elem(bits - 1, '0')) + "1"),
            (4,
             ~"3" +
             str::from_chars(vec::from_elem(bits / 2 - 1, '0')) + "2" +
             str::from_chars(vec::from_elem(bits / 2 - 1, '0')) + "1"),
            (10, match bits {
                32 => ~"55340232229718589441",
                16 => ~"12885032961",
                _ => fail!()
            }),
            (16, ~"3" +
             str::from_chars(vec::from_elem(bits / 4 - 1, '0')) + "2" +
             str::from_chars(vec::from_elem(bits / 4 - 1, '0')) + "1")
        ]) ]
    }

    #[test]
    fn test_to_str_radix() {
        for to_str_pairs().each |num_pair| {
            let &(n, rs) = num_pair;
            for rs.each |str_pair| {
                let &(radix, str) = str_pair;
                fail_unless!(n.to_str_radix(radix) == str);
            }
        }
    }

    #[test]
    fn test_from_str_radix() {
        for to_str_pairs().each |num_pair| {
            let &(n, rs) = num_pair;
            for rs.each |str_pair| {
                let &(radix, str) = str_pair;
                fail_unless!(Some(n) == BigUint::from_str_radix(str, radix));
            }
        }

        fail_unless!(BigUint::from_str_radix(~"Z", 10) == None);
        fail_unless!(BigUint::from_str_radix(~"_", 2) == None);
        fail_unless!(BigUint::from_str_radix(~"-1", 10) == None);
    }

    #[test]
    fn test_factor() {
        fn factor(n: uint) -> BigUint {
            let mut f= One::one::<BigUint>();
            for uint::range(2, n + 1) |i| {
                f *= BigUint::from_uint(i);
            }
            return f;
        }

        fn check(n: uint, s: &str) {
            let n = factor(n);
            let ans = match BigUint::from_str_radix(s, 10) {
                Some(x) => x, None => fail!()
            };
            fail_unless!(n == ans);
        }

        check(3, "6");
        check(10, "3628800");
        check(20, "2432902008176640000");
        check(30, "265252859812191058636308480000000");
    }
}

#[cfg(test)]
mod bigint_tests {
    use super::{BigInt, BigUint, BigDigit, Sign, Minus, Zero, Plus};

    use core::*;
    use core::num::{IntConvertible, Zero, One};

    #[test]
    fn test_from_biguint() {
        fn check(inp_s: Sign, inp_n: uint, ans_s: Sign, ans_n: uint) {
            let inp = BigInt::from_biguint(inp_s, BigUint::from_uint(inp_n));
            let ans = BigInt { sign: ans_s, data: BigUint::from_uint(ans_n)};
            fail_unless!(inp == ans);
        }
        check(Plus, 1, Plus, 1);
        check(Plus, 0, Zero, 0);
        check(Minus, 1, Minus, 1);
        check(Zero, 1, Zero, 0);
    }

    #[test]
    fn test_cmp() {
        let vs = [ &[2], &[1, 1], &[2, 1], &[1, 1, 1] ];
        let mut nums = vec::reversed(vs)
            .map(|s| BigInt::from_slice(Minus, *s));
        nums.push(Zero::zero());
        nums.push_all_move(vs.map(|s| BigInt::from_slice(Plus, *s)));

        for nums.eachi |i, ni| {
            for vec::slice(nums, i, nums.len()).eachi |j0, nj| {
                let j = i + j0;
                if i == j {
                    fail_unless!(ni.cmp(nj) == 0);
                    fail_unless!(nj.cmp(ni) == 0);
                    fail_unless!(ni == nj);
                    fail_unless!(!(ni != nj));
                    fail_unless!(ni <= nj);
                    fail_unless!(ni >= nj);
                    fail_unless!(!(ni < nj));
                    fail_unless!(!(ni > nj));
                } else {
                    fail_unless!(ni.cmp(nj) < 0);
                    fail_unless!(nj.cmp(ni) > 0);

                    fail_unless!(!(ni == nj));
                    fail_unless!(ni != nj);

                    fail_unless!(ni <= nj);
                    fail_unless!(!(ni >= nj));
                    fail_unless!(ni < nj);
                    fail_unless!(!(ni > nj));

                    fail_unless!(!(nj <= ni));
                    fail_unless!(nj >= ni);
                    fail_unless!(!(nj < ni));
                    fail_unless!(nj > ni);
                }
            }
        }
    }

    #[test]
    fn test_convert_int() {
        fn check(b: BigInt, i: int) {
            fail_unless!(b == IntConvertible::from_int(i));
            fail_unless!(b.to_int() == i);
        }

        check(Zero::zero(), 0);
        check(One::one(), 1);
        check(BigInt::from_biguint(
            Plus, BigUint::from_uint(int::max_value as uint)
        ), int::max_value);

        fail_unless!(BigInt::from_biguint(
            Plus, BigUint::from_uint(int::max_value as uint + 1)
        ).to_int() == int::max_value);
        fail_unless!(BigInt::from_biguint(
            Plus, BigUint::new(~[1, 2, 3])
        ).to_int() == int::max_value);

        check(BigInt::from_biguint(
            Minus, BigUint::from_uint(-int::min_value as uint)
        ), int::min_value);
        fail_unless!(BigInt::from_biguint(
            Minus, BigUint::from_uint(-int::min_value as uint + 1)
        ).to_int() == int::min_value);
        fail_unless!(BigInt::from_biguint(
            Minus, BigUint::new(~[1, 2, 3])
        ).to_int() == int::min_value);
    }

    #[test]
    fn test_convert_uint() {
        fn check(b: BigInt, u: uint) {
            fail_unless!(b == BigInt::from_uint(u));
            fail_unless!(b.to_uint() == u);
        }

        check(Zero::zero(), 0);
        check(One::one(), 1);

        check(
            BigInt::from_biguint(Plus, BigUint::from_uint(uint::max_value)),
            uint::max_value);
        fail_unless!(BigInt::from_biguint(
            Plus, BigUint::new(~[1, 2, 3])
        ).to_uint() == uint::max_value);

        fail_unless!(BigInt::from_biguint(
            Minus, BigUint::from_uint(uint::max_value)
        ).to_uint() == 0);
        fail_unless!(BigInt::from_biguint(
            Minus, BigUint::new(~[1, 2, 3])
        ).to_uint() == 0);
    }

    const sum_triples: &'static [(&'static [BigDigit],
                                 &'static [BigDigit],
                                 &'static [BigDigit])] = &[
        (&[],          &[],       &[]),
        (&[],          &[ 1],     &[ 1]),
        (&[ 1],        &[ 1],     &[ 2]),
        (&[ 1],        &[ 1,  1], &[ 2,  1]),
        (&[ 1],        &[-1],     &[ 0,  1]),
        (&[ 1],        &[-1, -1], &[ 0,  0, 1]),
        (&[-1, -1],    &[-1, -1], &[-2, -1, 1]),
        (&[ 1,  1, 1], &[-1, -1], &[ 0,  1, 2]),
        (&[ 2,  2, 1], &[-1, -2], &[ 1,  1, 2])
    ];

    #[test]
    fn test_add() {
        for sum_triples.each |elm| {
            let (aVec, bVec, cVec) = *elm;
            let a = BigInt::from_slice(Plus, aVec);
            let b = BigInt::from_slice(Plus, bVec);
            let c = BigInt::from_slice(Plus, cVec);

            fail_unless!(a + b == c);
            fail_unless!(b + a == c);
            fail_unless!(c + (-a) == b);
            fail_unless!(c + (-b) == a);
            fail_unless!(a + (-c) == (-b));
            fail_unless!(b + (-c) == (-a));
            fail_unless!((-a) + (-b) == (-c));
            fail_unless!(a + (-a) == Zero::zero());
        }
    }

    #[test]
    fn test_sub() {
        for sum_triples.each |elm| {
            let (aVec, bVec, cVec) = *elm;
            let a = BigInt::from_slice(Plus, aVec);
            let b = BigInt::from_slice(Plus, bVec);
            let c = BigInt::from_slice(Plus, cVec);

            fail_unless!(c - a == b);
            fail_unless!(c - b == a);
            fail_unless!((-b) - a == (-c));
            fail_unless!((-a) - b == (-c));
            fail_unless!(b - (-a) == c);
            fail_unless!(a - (-b) == c);
            fail_unless!((-c) - (-a) == (-b));
            fail_unless!(a - a == Zero::zero());
        }
    }

    const mul_triples: &'static [(&'static [BigDigit],
                                 &'static [BigDigit],
                                 &'static [BigDigit])] = &[
        (&[],               &[],               &[]),
        (&[],               &[ 1],             &[]),
        (&[ 2],             &[],               &[]),
        (&[ 1],             &[ 1],             &[1]),
        (&[ 2],             &[ 3],             &[ 6]),
        (&[ 1],             &[ 1,  1,  1],     &[1, 1,  1]),
        (&[ 1,  2,  3],     &[ 3],             &[ 3,  6,  9]),
        (&[ 1,  1,  1],     &[-1],             &[-1, -1, -1]),
        (&[ 1,  2,  3],     &[-1],             &[-1, -2, -2, 2]),
        (&[ 1,  2,  3,  4], &[-1],             &[-1, -2, -2, -2, 3]),
        (&[-1],             &[-1],             &[ 1, -2]),
        (&[-1, -1],         &[-1],             &[ 1, -1, -2]),
        (&[-1, -1, -1],     &[-1],             &[ 1, -1, -1, -2]),
        (&[-1, -1, -1, -1], &[-1],             &[ 1, -1, -1, -1, -2]),
        (&[-1/2 + 1],       &[ 2],             &[ 0,  1]),
        (&[0, -1/2 + 1],    &[ 2],             &[ 0,  0,  1]),
        (&[ 1,  2],         &[ 1,  2,  3],     &[1, 4,  7,  6]),
        (&[-1, -1],         &[-1, -1, -1],     &[1, 0, -1, -2, -1]),
        (&[-1, -1, -1],     &[-1, -1, -1, -1], &[1, 0,  0, -1, -2, -1, -1]),
        (&[ 0,  0,  1],     &[ 1,  2,  3],     &[0, 0,  1,  2,  3]),
        (&[ 0,  0,  1],     &[ 0,  0,  0,  1], &[0, 0,  0,  0,  0,  1])
    ];

    const divmod_quadruples: &'static [(&'static [BigDigit],
                                       &'static [BigDigit],
                                       &'static [BigDigit],
                                       &'static [BigDigit])]
        = &[
            (&[ 1],        &[ 2], &[],               &[1]),
            (&[ 1,  1],    &[ 2], &[-1/2+1],         &[1]),
            (&[ 1,  1, 1], &[ 2], &[-1/2+1, -1/2+1], &[1]),
            (&[ 0,  1],    &[-1], &[1],              &[1]),
            (&[-1, -1],    &[-2], &[2, 1],           &[3])
        ];

    #[test]
    fn test_mul() {
        for mul_triples.each |elm| {
            let (aVec, bVec, cVec) = *elm;
            let a = BigInt::from_slice(Plus, aVec);
            let b = BigInt::from_slice(Plus, bVec);
            let c = BigInt::from_slice(Plus, cVec);

            fail_unless!(a * b == c);
            fail_unless!(b * a == c);

            fail_unless!((-a) * b == -c);
            fail_unless!((-b) * a == -c);
        }

        for divmod_quadruples.each |elm| {
            let (aVec, bVec, cVec, dVec) = *elm;
            let a = BigInt::from_slice(Plus, aVec);
            let b = BigInt::from_slice(Plus, bVec);
            let c = BigInt::from_slice(Plus, cVec);
            let d = BigInt::from_slice(Plus, dVec);

            fail_unless!(a == b * c + d);
            fail_unless!(a == c * b + d);
        }
    }

    #[test]
    fn test_divmod() {
        fn check_sub(a: &BigInt, b: &BigInt, ans_d: &BigInt, ans_m: &BigInt) {
            let (d, m) = a.divmod(b);
            if m.is_not_zero() {
                fail_unless!(m.sign == b.sign);
            }
            fail_unless!(m.abs() <= b.abs());
            fail_unless!(*a == b * d + m);
            fail_unless!(d == *ans_d);
            fail_unless!(m == *ans_m);
        }

        fn check(a: &BigInt, b: &BigInt, d: &BigInt, m: &BigInt) {
            if m.is_zero() {
                check_sub(a, b, d, m);
                check_sub(a, &b.neg(), &d.neg(), m);
                check_sub(&a.neg(), b, &d.neg(), m);
                check_sub(&a.neg(), &b.neg(), d, m);
            } else {
                check_sub(a, b, d, m);
                check_sub(a, &b.neg(), &(d.neg() - One::one()), &(m - *b));
                check_sub(&a.neg(), b, &(d.neg() - One::one()), &(b - *m));
                check_sub(&a.neg(), &b.neg(), d, &m.neg());
            }
        }

        for mul_triples.each |elm| {
            let (aVec, bVec, cVec) = *elm;
            let a = BigInt::from_slice(Plus, aVec);
            let b = BigInt::from_slice(Plus, bVec);
            let c = BigInt::from_slice(Plus, cVec);

            if a.is_not_zero() { check(&c, &a, &b, &Zero::zero()); }
            if b.is_not_zero() { check(&c, &b, &a, &Zero::zero()); }
        }

        for divmod_quadruples.each |elm| {
            let (aVec, bVec, cVec, dVec) = *elm;
            let a = BigInt::from_slice(Plus, aVec);
            let b = BigInt::from_slice(Plus, bVec);
            let c = BigInt::from_slice(Plus, cVec);
            let d = BigInt::from_slice(Plus, dVec);

            if b.is_not_zero() {
                check(&a, &b, &c, &d);
            }
        }
    }


    #[test]
    fn test_quotrem() {
        fn check_sub(a: &BigInt, b: &BigInt, ans_q: &BigInt, ans_r: &BigInt) {
            let (q, r) = a.quotrem(b);
            if r.is_not_zero() {
                fail_unless!(r.sign == a.sign);
            }
            fail_unless!(r.abs() <= b.abs());
            fail_unless!(*a == b * q + r);
            fail_unless!(q == *ans_q);
            fail_unless!(r == *ans_r);
        }

        fn check(a: &BigInt, b: &BigInt, q: &BigInt, r: &BigInt) {
            check_sub(a, b, q, r);
            check_sub(a, &b.neg(), &q.neg(), r);
            check_sub(&a.neg(), b, &q.neg(), &r.neg());
            check_sub(&a.neg(), &b.neg(), q, &r.neg());
        }
        for mul_triples.each |elm| {
            let (aVec, bVec, cVec) = *elm;
            let a = BigInt::from_slice(Plus, aVec);
            let b = BigInt::from_slice(Plus, bVec);
            let c = BigInt::from_slice(Plus, cVec);

            if a.is_not_zero() { check(&c, &a, &b, &Zero::zero()); }
            if b.is_not_zero() { check(&c, &b, &a, &Zero::zero()); }
        }

        for divmod_quadruples.each |elm| {
            let (aVec, bVec, cVec, dVec) = *elm;
            let a = BigInt::from_slice(Plus, aVec);
            let b = BigInt::from_slice(Plus, bVec);
            let c = BigInt::from_slice(Plus, cVec);
            let d = BigInt::from_slice(Plus, dVec);

            if b.is_not_zero() {
                check(&a, &b, &c, &d);
            }
        }
    }

    #[test]
    fn test_to_str_radix() {
        fn check(n: int, ans: &str) {
            fail_unless!(ans == IntConvertible::from_int::<BigInt>(
                n).to_str_radix(10));
        }
        check(10, "10");
        check(1, "1");
        check(0, "0");
        check(-1, "-1");
        check(-10, "-10");
    }


    #[test]
    fn test_from_str_radix() {
        fn check(s: &str, ans: Option<int>) {
            let ans = ans.map(|&n| IntConvertible::from_int(n));
            fail_unless!(BigInt::from_str_radix(s, 10) == ans);
        }
        check("10", Some(10));
        check("1", Some(1));
        check("0", Some(0));
        check("-1", Some(-1));
        check("-10", Some(-10));
        check("Z", None);
        check("_", None);
    }

    #[test]
    fn test_neg() {
        fail_unless!(-BigInt::new(Plus,  ~[1, 1, 1]) ==
            BigInt::new(Minus, ~[1, 1, 1]));
        fail_unless!(-BigInt::new(Minus, ~[1, 1, 1]) ==
            BigInt::new(Plus,  ~[1, 1, 1]));
        fail_unless!(-Zero::zero::<BigInt>() == Zero::zero::<BigInt>());
    }
}

