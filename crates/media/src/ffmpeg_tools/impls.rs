//! Some basic implementations to help make working with FFmpeg types easier.

use ffmpeg::Rational;
use ffmpeg_next as ffmpeg;

use crate::fps::{Fps, FpsError};

impl TryFrom<Fps> for Rational {
    type Error = FpsError;

    fn try_from(fps: Fps) -> Result<Self, Self::Error> {
        let (num, den) = fps.as_frac();
        match (num.try_into(), den.try_into()) {
            (Ok(num), Ok(den)) => Ok(Rational(num, den)),
            (Err(_), _) => Err(FpsError::NonPositiveNum),
            (_, Err(_)) => Err(FpsError::NonPositiveDen),
        }
    }
}

impl TryFrom<Rational> for Fps {
    type Error = FpsError;

    fn try_from(fps: Rational) -> Result<Self, Self::Error> {
        let (num, den) = (fps.0, fps.1);
        match (num.try_into(), den.try_into()) {
            (Ok(num), Ok(den)) => Fps::from_frac(num, den),
            (Err(_), _) => Err(FpsError::NonPositiveNum),
            (_, Err(_)) => Err(FpsError::NonPositiveDen),
        }
    }
}
