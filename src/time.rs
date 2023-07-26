use std::ops::Add;
use libav::mathematics::rescale::Rescale;
use libav::util::rational::Rational;
use anyhow::Result;

#[derive(Copy, Clone, Debug, Default)]
pub struct Timestamp(u64);
#[derive(Copy, Clone, Debug, Default)]
pub struct Duration(u64);

impl Timestamp {
    pub fn from_libav_ts(ts: i64, time_base: Rational) -> Result<Self> {
        //TODO perform rescale with proper check 
        //https://ffmpeg.org/doxygen/trunk/group__lavu__math.html#ga82d40664213508918093822461cc597e
        todo!()
    }

    pub fn from_timebase(ts: u64, time_base: Rational) -> Result<Self> {
        todo!()
    }

    pub fn as_millis(&self) -> u64 {
        todo!()
    }
}

impl Duration {
    pub fn from_millis(millis: u64) -> Duration {
        todo!()
    }
}

impl Add<Duration> for Timestamp {
    type Output = Self;

    fn add(self, d: Duration) -> Self::Output {
        todo!()
    }
}
