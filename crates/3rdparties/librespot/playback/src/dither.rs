use rand::rngs::SmallRng;
use rand_distr::{Distribution, Normal, Triangular, Uniform};
use std::fmt;

use crate::NUM_CHANNELS;

// Dithering lowers digital-to-analog conversion ("requantization") error,
// linearizing output, lowering distortion and replacing it with a constant,
// fixed noise level, which is more pleasant to the ear than the distortion.
//
// Guidance:
//
//  * On S24, S24_3 and S24, the default is to use triangular dithering.
//    Depending on personal preference you may use Gaussian dithering instead;
//    it's not as good objectively, but it may be preferred subjectively if
//    you are looking for a more "analog" sound akin to tape hiss.
//
//  * Advanced users who know that they have a DAC without noise shaping have
//    a third option: high-passed dithering, which is like triangular dithering
//    except that it moves dithering noise up in frequency where it is less
//    audible. Note: 99% of DACs are of delta-sigma design with noise shaping,
//    so unless you have a multibit / R2R DAC, or otherwise know what you are
//    doing, this is not for you.
//
//  * Don't dither or shape noise on S32 or F32. On F32 it's not supported
//    anyway (there are no integer conversions and so no rounding errors) and
//    on S32 the noise level is so far down that it is simply inaudible even
//    after volume normalisation and control.
//
pub trait Ditherer {
    fn new() -> Self
    where
        Self: Sized;
    fn name(&self) -> &'static str;
    fn noise(&mut self) -> f64;
}

impl fmt::Display for dyn Ditherer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

fn create_rng() -> SmallRng {
    rand::make_rng()
}

pub struct TriangularDitherer {
    cached_rng: SmallRng,
    distribution: Triangular<f64>,
}

impl Ditherer for TriangularDitherer {
    fn new() -> Self {
        Self {
            cached_rng: create_rng(),
            // 2 LSB peak-to-peak needed to linearize the response:
            distribution: Triangular::new(-1.0, 1.0, 0.0).unwrap(),
        }
    }

    fn name(&self) -> &'static str {
        Self::NAME
    }

    #[inline]
    fn noise(&mut self) -> f64 {
        self.distribution.sample(&mut self.cached_rng)
    }
}

impl TriangularDitherer {
    pub const NAME: &'static str = "tpdf";
}

pub struct GaussianDitherer {
    cached_rng: SmallRng,
    distribution: Normal<f64>,
}

impl Ditherer for GaussianDitherer {
    fn new() -> Self {
        Self {
            cached_rng: create_rng(),
            // For Gaussian to achieve equivalent decorrelation to triangular dithering, it needs
            // 3-4 dB higher amplitude than TPDF's optimal 0.408 LSB. If optimizing:
            // - minimum correlation: σ ≈ 0.58
            // - perceptual equivalence: σ ≈ 0.65
            // - worst-case performance: σ ≈ 0.70
            //
            // σ = 0.6 LSB is a reasonable compromise that balances mathematical theory with
            // empirical performance across various signal types.
            distribution: Normal::new(0.0, 0.6).unwrap(),
        }
    }

    fn name(&self) -> &'static str {
        Self::NAME
    }

    #[inline]
    fn noise(&mut self) -> f64 {
        self.distribution.sample(&mut self.cached_rng)
    }
}

impl GaussianDitherer {
    pub const NAME: &'static str = "gpdf";
}

pub struct HighPassDitherer {
    active_channel: usize,
    previous_noises: [f64; NUM_CHANNELS as usize],
    cached_rng: SmallRng,
    distribution: Uniform<f64>,
}

impl Ditherer for HighPassDitherer {
    fn new() -> Self {
        Self {
            active_channel: 0,
            previous_noises: [0.0; NUM_CHANNELS as usize],
            cached_rng: create_rng(),
            // 1 LSB +/- 1 LSB (previous) = 2 LSB
            distribution: Uniform::new_inclusive(-0.5, 0.5)
                .expect("Failed to create uniform distribution"),
        }
    }

    fn name(&self) -> &'static str {
        Self::NAME
    }

    #[inline]
    fn noise(&mut self) -> f64 {
        let new_noise = self.distribution.sample(&mut self.cached_rng);
        let high_passed_noise = new_noise - self.previous_noises[self.active_channel];
        self.previous_noises[self.active_channel] = new_noise;
        self.active_channel ^= 1;
        high_passed_noise
    }
}

impl HighPassDitherer {
    pub const NAME: &'static str = "tpdf_hp";
}

pub fn mk_ditherer<D: Ditherer + 'static>() -> Box<dyn Ditherer> {
    Box::new(D::new())
}

pub type DithererBuilder = fn() -> Box<dyn Ditherer>;

pub fn find_ditherer(name: Option<String>) -> Option<DithererBuilder> {
    match name.as_deref() {
        Some(TriangularDitherer::NAME) => Some(mk_ditherer::<TriangularDitherer>),
        Some(GaussianDitherer::NAME) => Some(mk_ditherer::<GaussianDitherer>),
        Some(HighPassDitherer::NAME) => Some(mk_ditherer::<HighPassDitherer>),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn moments<D: Ditherer>(ditherer: &mut D) -> (f64, f64) {
        const SAMPLES: usize = 100_000;
        let (mut sum, mut squares) = (0.0, 0.0);
        for _ in 0..SAMPLES {
            let noise = ditherer.noise();
            assert!(noise.is_finite());
            sum += noise;
            squares += noise * noise;
        }
        (sum / SAMPLES as f64, squares / SAMPLES as f64)
    }

    #[test]
    fn seeded_dither_distributions_preserve_zero_mean_and_variance() {
        let mut triangular = TriangularDitherer::new();
        triangular.cached_rng = SmallRng::seed_from_u64(17);
        let (mean, second_moment) = moments(&mut triangular);
        assert!(mean.abs() < 0.02);
        assert!((second_moment - 1.0 / 6.0).abs() < 0.02);

        let mut gaussian = GaussianDitherer::new();
        gaussian.cached_rng = SmallRng::seed_from_u64(17);
        let (mean, second_moment) = moments(&mut gaussian);
        assert!(mean.abs() < 0.02);
        assert!((second_moment - 0.6_f64.powi(2)).abs() < 0.03);

        let mut high_pass = HighPassDitherer::new();
        high_pass.cached_rng = SmallRng::seed_from_u64(17);
        let (mean, second_moment) = moments(&mut high_pass);
        assert!(mean.abs() < 0.02);
        assert!((second_moment - 1.0 / 6.0).abs() < 0.03);
    }

    #[test]
    fn high_pass_dither_keeps_independent_channel_histories() {
        let mut ditherer = HighPassDitherer::new();
        ditherer.cached_rng = SmallRng::seed_from_u64(29);
        let mut reference_rng = SmallRng::seed_from_u64(29);
        let distribution = Uniform::new_inclusive(-0.5, 0.5).unwrap();
        let samples: Vec<f64> = (0..4)
            .map(|_| distribution.sample(&mut reference_rng))
            .collect();
        assert_eq!(ditherer.noise(), samples[0]);
        assert_eq!(ditherer.noise(), samples[1]);
        assert_eq!(ditherer.noise(), samples[2] - samples[0]);
        assert_eq!(ditherer.noise(), samples[3] - samples[1]);
    }

    #[test]
    fn seeded_dither_replay_is_bit_identical() {
        let mut first = TriangularDitherer::new();
        let mut replay = TriangularDitherer::new();
        first.cached_rng = SmallRng::seed_from_u64(71);
        replay.cached_rng = SmallRng::seed_from_u64(71);
        for _ in 0..4096 {
            assert_eq!(first.noise().to_bits(), replay.noise().to_bits());
        }
    }
}
