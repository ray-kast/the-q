use num_traits::{ConstOne, NumOps};

#[inline]
pub fn reinhard<T: ConstOne + NumOps + Copy>(x: T) -> T { x / (x + T::ONE) }

/// Equal to `-reinhard(-x)`
#[inline]
pub fn reinhard_inv<T: ConstOne + NumOps + Copy>(x: T) -> T { x / (T::ONE - x) }

#[cfg(test)]
mod test {
    use proptest::prelude::*;

    pub const STOP12: f64 = 4096.0;
    pub const EPSILON12: f64 = STOP12.recip();

    pub fn assert_eq(expected: f64, actual: f64) {
        assert!(
            (expected - actual).abs() < EPSILON12,
            "expected={expected}\nactual={actual}"
        );
    }

    proptest! {
        #[test]
        fn reinhard(x in -STOP12..=STOP12) {
            let roundtrip = super::reinhard(super::reinhard_inv(x));
            assert_eq(x, roundtrip);
        }
    }
}

pub use sigmoid::{normalized as sigmoid, normalized_inv as sigmoid_inv, Params as SigmoidParams};

mod sigmoid {
    use num_traits::{ConstOne, ConstZero, NumOps};

    use super::{reinhard, reinhard_inv};

    pub trait Params: Copy {
        type Scalar: From<bool> + PartialOrd + ConstZero + ConstOne + NumOps + Copy;

        /* const */
        fn gain_negative(self) -> Self::Scalar;
        /* const */
        fn gain_positive(self) -> Self::Scalar;

        /* const */
        fn inflection(self) -> Self::Scalar;

        /* const */
        fn domain_max(self) -> Self::Scalar;
    }

    #[inline]
    fn gain<P: Params>(x: P::Scalar, params: P) -> P::Scalar {
        let pos = x >= P::Scalar::ZERO;
        P::Scalar::from(pos) * params.gain_positive()
            + P::Scalar::from(!pos) * params.gain_negative()
    }

    #[inline]
    fn sigmoid<P: Params>(x: P::Scalar, params: P) -> P::Scalar {
        let gain = gain(x, params);
        reinhard(gain * x) / gain
    }

    #[inline]
    fn sigmoid_inv<P: Params>(x: P::Scalar, params: P) -> P::Scalar {
        let gain = gain(x, params);
        reinhard_inv(gain * x) / gain
    }

    #[inline]
    fn normalized_min<P: Params>(params: P) -> P::Scalar {
        sigmoid(P::Scalar::ZERO - params.inflection(), params)
    }

    #[inline]
    fn normalized_max<P: Params>(params: P) -> P::Scalar {
        sigmoid(params.domain_max() - params.inflection(), params)
    }

    #[inline]
    pub fn normalized<P: Params>(x: P::Scalar, params: P) -> P::Scalar {
        let min = normalized_min(params);
        (sigmoid(x - params.inflection(), params) - min) / (normalized_max(params) - min)
    }

    #[inline]
    pub fn normalized_inv<P: Params>(x: P::Scalar, params: P) -> P::Scalar {
        let min = normalized_min(params);
        sigmoid_inv(x * (normalized_max(params) - min) + min, params) + params.inflection()
    }

    #[cfg(test)]
    mod test {
        use proptest::prelude::*;

        use super::super::test::{assert_eq, EPSILON12, STOP12};

        #[derive(Debug, Clone, Copy)]
        struct Params {
            gain_negative: f64,
            gain_positive: f64,
            inflection: f64,
            domain_max: f64,
        }

        fn params() -> impl Strategy<Value = Params> {
            (
                -STOP12..=-EPSILON12,
                EPSILON12..=STOP12,
                -STOP12..=STOP12,
                EPSILON12..=STOP12,
            )
                .prop_map(|(gain_negative, gain_positive, inflection, domain_max)| {
                    Params {
                        gain_negative,
                        gain_positive,
                        inflection,
                        domain_max,
                    }
                })
        }

        impl super::Params for Params {
            type Scalar = f64;

            #[inline]
            fn gain_negative(self) -> Self::Scalar { self.gain_negative }

            #[inline]
            fn gain_positive(self) -> Self::Scalar { self.gain_positive }

            #[inline]
            fn inflection(self) -> Self::Scalar { self.inflection }

            #[inline]
            fn domain_max(self) -> Self::Scalar { self.domain_max }
        }

        fn sigmoid_params() -> impl Strategy<Value = (Params, f64)> {
            params().prop_flat_map(|p| {
                (p.gain_negative.recip()..=p.gain_positive.recip()).prop_map(move |x| (p, x))
            })
        }

        fn biased_params() -> impl Strategy<Value = (Params, f64)> { sigmoid_params() }

        fn normalized_params() -> impl Strategy<Value = (Params, f64)> { (params(), 0.0..=1.0) }

        proptest! {
            #[test]
            fn sigmoid((params, x) in sigmoid_params()) {
                let roundtrip = super::sigmoid(super::sigmoid_inv(x, params), params);
                assert_eq(x, roundtrip);
            }

            #[test]
            fn normalized((params, x) in normalized_params()) {
                let roundtrip = super::normalized(super::normalized_inv(x, params), params);
                assert_eq(x, roundtrip);
            }

        }
    }
}
