use core::cmp::Ordering;

/// Signed fixed-point value stored as a little-endian magnitude.
///
/// Each limb contributes 32 fractional bits, so `Fixed<1>` is Q32, `Fixed<2>`
/// is Q64, and larger limb counts extend the fractional precision likewise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fixed<const N: usize> {
    negative: bool,
    limbs: [u64; N],
}

impl<const N: usize> Fixed<N> {
    pub fn zero() -> Self {
        assert!(N > 0, "Fixed requires at least one limb");
        Self {
            negative: false,
            limbs: [0; N],
        }
    }

    pub fn from_i64(value: i64) -> Self {
        let mut result = Self::zero();
        let magnitude = value.unsigned_abs() as u128;
        result.set_shifted_magnitude(magnitude, Self::fractional_bits());
        result.negative = value < 0 && !result.is_zero();
        result
    }

    pub fn from_f64(value: f64) -> Self {
        assert!(value.is_finite(), "Fixed requires a finite value");
        let mut result = Self::zero();
        let fractional_bits = Self::fractional_bits();
        assert!(
            fractional_bits < 128,
            "from_f64 supports at most 127 fractional bits"
        );
        let scale = 2f64.powi(fractional_bits as i32);
        let scaled = (value.abs() * scale).round();
        assert!(scaled <= u128::MAX as f64, "value does not fit Fixed");
        result.set_shifted_magnitude(scaled as u128, 0);
        result.negative = value.is_sign_negative() && !result.is_zero();
        result
    }

    pub fn to_f64(self) -> f64 {
        let mut value = 0.0;
        for &limb in self.limbs.iter().rev() {
            value = value * 18_446_744_073_709_551_616.0 + limb as f64;
        }
        let value = value / 2f64.powi(Self::fractional_bits() as i32);
        if self.negative {
            -value
        } else {
            value
        }
    }

    pub const fn fractional_bits() -> u32 {
        (N as u32) * 32
    }

    pub fn add(self, rhs: Self) -> Self {
        if self.negative == rhs.negative {
            let mut result = Self {
                negative: self.negative,
                limbs: add_magnitudes(&self.limbs, &rhs.limbs),
            };
            result.normalize_sign();
            result
        } else if compare_magnitude(&self.limbs, &rhs.limbs) != Ordering::Less {
            let mut result = Self {
                negative: self.negative,
                limbs: sub_magnitudes(&self.limbs, &rhs.limbs),
            };
            result.normalize_sign();
            result
        } else {
            let mut result = Self {
                negative: rhs.negative,
                limbs: sub_magnitudes(&rhs.limbs, &self.limbs),
            };
            result.normalize_sign();
            result
        }
    }

    pub fn sub(self, rhs: Self) -> Self {
        self.add(-rhs)
    }

    pub fn mul(self, rhs: Self) -> Self {
        let mut result = Self {
            negative: self.negative ^ rhs.negative,
            limbs: multiply_magnitudes(&self.limbs, &rhs.limbs),
        };
        result.normalize_sign();
        result
    }

    pub fn square(self) -> Self {
        self.mul(self)
    }

    pub fn compare(self, rhs: Self) -> Ordering {
        match (self.negative, rhs.negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => compare_magnitude(&self.limbs, &rhs.limbs),
            (true, true) => compare_magnitude(&rhs.limbs, &self.limbs),
        }
    }

    fn is_zero(&self) -> bool {
        self.limbs.iter().all(|&limb| limb == 0)
    }

    fn normalize_sign(&mut self) {
        if self.is_zero() {
            self.negative = false;
        }
    }

    fn set_shifted_magnitude(&mut self, magnitude: u128, shift: u32) {
        let word_shift = (shift / 64) as usize;
        let bit_shift = shift % 64;
        if word_shift >= N {
            assert_eq!(magnitude, 0, "value does not fit Fixed");
            return;
        }
        self.limbs[word_shift] = (magnitude as u64) << bit_shift;
        if bit_shift != 0 && word_shift + 1 < N {
            self.limbs[word_shift + 1] |= (magnitude as u64) >> (64 - bit_shift);
            self.limbs[word_shift + 1] |= ((magnitude >> 64) as u64) << bit_shift;
        } else if word_shift + 1 < N {
            self.limbs[word_shift + 1] = (magnitude >> 64) as u64;
        }
        assert!(
            word_shift + 2 >= N || (magnitude >> 64 == 0),
            "value does not fit Fixed"
        );
    }
}

impl<const N: usize> core::ops::Neg for Fixed<N> {
    type Output = Self;

    fn neg(mut self) -> Self::Output {
        if !self.is_zero() {
            self.negative = !self.negative;
        }
        self
    }
}

fn add_magnitudes<const N: usize>(left: &[u64; N], right: &[u64; N]) -> [u64; N] {
    let mut result = [0; N];
    let mut carry = 0u128;
    for index in 0..N {
        let sum = left[index] as u128 + right[index] as u128 + carry;
        result[index] = sum as u64;
        carry = sum >> 64;
    }
    assert_eq!(carry, 0, "Fixed addition overflow");
    result
}

fn sub_magnitudes<const N: usize>(left: &[u64; N], right: &[u64; N]) -> [u64; N] {
    let mut result = [0; N];
    let mut borrow = 0u64;
    for index in 0..N {
        let (value, first_borrow) = left[index].overflowing_sub(right[index]);
        let (value, second_borrow) = value.overflowing_sub(borrow);
        result[index] = value;
        borrow = u64::from(first_borrow || second_borrow);
    }
    assert_eq!(borrow, 0, "Fixed subtraction underflow");
    result
}

fn compare_magnitude<const N: usize>(left: &[u64; N], right: &[u64; N]) -> Ordering {
    for index in (0..N).rev() {
        match left[index].cmp(&right[index]) {
            Ordering::Equal => continue,
            ordering => return ordering,
        }
    }
    Ordering::Equal
}

fn multiply_magnitudes<const N: usize>(left: &[u64; N], right: &[u64; N]) -> [u64; N] {
    let mut product = vec![0u64; N.saturating_mul(2).saturating_add(1)];
    for i in 0..N {
        let mut carry = 0u128;
        for j in 0..N {
            let index = i + j;
            let value = left[i] as u128 * right[j] as u128 + product[index] as u128 + carry;
            product[index] = value as u64;
            carry = value >> 64;
        }
        let mut index = i + N;
        while carry != 0 {
            let value = product[index] as u128 + carry;
            product[index] = value as u64;
            carry = value >> 64;
            index += 1;
        }
    }

    let fractional_bits = (N as u32) * 32;
    let word_shift = (fractional_bits / 64) as usize;
    let bit_shift = fractional_bits % 64;
    let mut result = [0; N];
    for index in 0..N {
        let source = index + word_shift;
        if source >= product.len() {
            continue;
        }
        result[index] = product[source] >> bit_shift;
        if bit_shift != 0 && source + 1 < product.len() {
            result[index] |= product[source + 1] << (64 - bit_shift);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::Fixed;
    use core::cmp::Ordering;

    #[test]
    fn limb_count_controls_fractional_precision() {
        assert_eq!(Fixed::<1>::fractional_bits(), 32);
        assert_eq!(Fixed::<2>::fractional_bits(), 64);
        assert_eq!(Fixed::<4>::fractional_bits(), 128);
    }

    #[test]
    fn fixed_one_represents_fractional_values() {
        let value = Fixed::<1>::from_f64(1.5);

        assert_eq!(value.to_f64(), 1.5);
    }

    #[test]
    fn supports_signed_addition_and_subtraction() {
        let value = Fixed::<1>::from_f64(1.5).add(Fixed::from_f64(-0.25));

        assert_eq!(value.to_f64(), 1.25);
        assert_eq!(value.sub(Fixed::from_f64(2.0)).to_f64(), -0.75);
    }

    #[test]
    fn multiplication_and_square_preserve_fractional_values() {
        let value = Fixed::<1>::from_f64(-1.5).mul(Fixed::from_f64(2.0));

        assert_eq!(value.to_f64(), -3.0);
        assert_eq!(value.square().to_f64(), 9.0);
    }

    #[test]
    fn two_limbs_carry_across_the_word_boundary() {
        let value = Fixed::<2>::from_i64(1 << 40);

        assert_eq!(value.to_f64(), (1u64 << 40) as f64);
    }

    #[test]
    fn compares_signed_values() {
        assert_eq!(
            Fixed::<1>::from_f64(-1.0).compare(Fixed::from_f64(0.0)),
            Ordering::Less
        );
        assert_eq!(
            Fixed::<1>::from_f64(2.0).compare(Fixed::from_f64(1.0)),
            Ordering::Greater
        );
        assert_eq!(
            Fixed::<1>::from_f64(1.0).compare(Fixed::from_f64(1.0)),
            Ordering::Equal
        );
    }
}
