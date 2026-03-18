//! Exports [ResampledFFmpegVideo].

use std::num::NonZeroUsize;

use super::{Clip, VideoFrameStreamBuilder};
use crate::ffmpeg_tools::FFmpegResult;
use crate::ffmpeg_tools::ffmpeg_video::{FFmpegVideo, FFmpegVideoFrame};
use crate::fps::{self, Fps, Resampler};
use crate::frame::{Dimensions, RescaleMethod};

/// An extended [FFmpegVideo] that supports FPS resampling, custom playback
/// speeds, looping, and clipping.
///
/// If any method returns an error, the state of the object becomes undefined.
#[derive(Debug)]
pub struct ResampledFFmpegVideo {
    ffmpeg_video: FFmpegVideo,
    fps_resampler: Resampler,
    target_fps: Fps,
    playback_speed: Fps,
    resampled_playhead: usize,
    resampled_clip: Clip,
    resampled_duration: NonZeroUsize,
    resampled_paused: bool,
    will_loop: bool,
    last_frame_played: bool,

    #[cfg(debug_assertions)]
    debug_check_blocks: usize,
}

impl ResampledFFmpegVideo {
    /// Create a new [ResampledFFmpegVideo].
    ///
    /// `builder`'s [rescale](VideoFrameStreamBuilder::rescale) and
    /// [fetch_timeout](VideoFrameStreamBuilder::fetch_timeout) are ignored.
    pub fn new(ffmpeg_video: FFmpegVideo, builder: VideoFrameStreamBuilder) -> Self {
        let VideoFrameStreamBuilder {
            target_fps,
            paused,
            clip,
            playhead,
            will_loop,
            playback_speed,
            rescale: _,
            fetch_timeout: _,
        } = builder;

        let src_fps = ffmpeg_video.src_fps();
        let src_duration = ffmpeg_video.duration_non_zero();

        let mut ret = Self {
            ffmpeg_video,
            fps_resampler: Resampler::no_op(),       // fixed below
            target_fps: src_fps,                     // fixed below
            playback_speed: fps::consts::FPS_1,      // fixed below
            resampled_playhead: 0,                   // fixed below
            resampled_clip: (0..=usize::MAX).into(), // fixed below
            resampled_duration: src_duration,        // fixed below
            resampled_paused: paused,
            will_loop,
            last_frame_played: false,

            #[cfg(debug_assertions)]
            debug_check_blocks: 1,
        };

        let target_fps = target_fps.unwrap_or(src_fps);
        ret.reconfigure_resampler_and_affected_fields(target_fps, playback_speed);
        ret.set_clip(clip);
        ret.seek_playhead(playhead);

        ret.unblock_debug_checks();
        ret.debug_assert_state_is_valid();

        ret
    }

    /// Like [FFmpegVideo::write_next] except it handles all the extra things
    /// that the resampled video can handle.
    pub fn write_next(
        &mut self,
        recycled_frame: Option<FFmpegVideoFrame>,
    ) -> FFmpegResult<FFmpegVideoFrame> {
        self.debug_assert_state_is_valid();

        let ret = self.step_frame_wrapper(|slf| {
            // If we're already on the right frame, this will be a no-op.
            let target_src_playhead = slf.fps_resampler.resample(slf.resampled_playhead);
            slf.ffmpeg_video.seek_playhead(target_src_playhead)?;

            // The src video should be paused if:
            // - We (the resampled video) are paused (also the case when the
            //   video is over and we're not looping).
            // - The next frame after this will be the same as the one we're
            //   about to generate.
            let pause_src = slf.resampled_paused
                || target_src_playhead == slf.fps_resampler.resample(slf.resampled_playhead + 1);
            slf.ffmpeg_video.set_paused(pause_src);

            slf.ffmpeg_video.write_next(recycled_frame)
        })?;

        self.debug_assert_state_is_valid();
        Ok(ret)
    }

    /// Like [Self::write_next] except it only simulates writing the next frame.
    pub fn step(&mut self) {
        self.debug_assert_state_is_valid();
        self.step_frame_wrapper(|_| {});
        self.debug_assert_state_is_valid();
    }

    // Non-trivial setters:

    /// Updates the resampled playhead to point to the index of the next frame
    /// that will be written. The actual new playhead (after being clamped to be
    /// within the clip) is returned.
    ///
    /// The underlying [FFmpegVideo] is not seeked. That doesn't happen until
    /// the next call to [Self::write_next].
    pub fn seek_playhead(&mut self, new_playhead: usize) -> usize {
        self.debug_assert_state_is_valid();

        let new_playhead = new_playhead.clamp(self.resampled_clip.start, self.resampled_clip.end);
        if new_playhead == self.resampled_playhead {
            return new_playhead;
        }

        // We have to be paused if we've played the last frame and are still at
        // the end of the video.
        if self.last_frame_played && new_playhead == self.resampled_clip.end {
            self.resampled_paused = true;
        }

        self.resampled_playhead = new_playhead;

        self.debug_assert_state_is_valid();
        new_playhead
    }

    /// Set the range of this stream that can be played.
    pub fn set_clip(&mut self, mut clip: Clip) -> Clip {
        self.debug_assert_state_is_valid();

        clip.fix_in_place(self.resampled_duration);
        if clip == self.resampled_clip {
            return clip;
        }

        self.block_debug_checks();
        self.resampled_clip = clip;
        self.seek_playhead(self.resampled_playhead);
        self.unblock_debug_checks();

        self.debug_assert_state_is_valid();
        clip
    }

    /// Change the [Self::target_fps] from here.
    pub fn set_target_fps(&mut self, target_fps: Fps) {
        self.debug_assert_state_is_valid();

        if target_fps != self.target_fps {
            self.reconfigure_resampler_and_affected_fields(target_fps, self.playback_speed);
        }

        self.debug_assert_state_is_valid();
    }

    /// Change the [Self::playback_speed] from here.
    pub fn set_playback_speed(&mut self, playback_speed: Fps) {
        self.debug_assert_state_is_valid();

        if playback_speed != self.playback_speed {
            self.reconfigure_resampler_and_affected_fields(self.target_fps, playback_speed);
        }

        self.debug_assert_state_is_valid();
    }

    /// Tries to set whether or not the stream is paused. The new paused state
    /// is returned.
    pub const fn set_paused(&mut self, paused: bool) -> bool {
        self.debug_assert_state_is_valid();

        // We have to be paused if we've played the last frame and are still at
        // the end of the video.
        if self.last_frame_played && self.resampled_playhead == self.resampled_clip.end {
            debug_assert!(self.resampled_paused);
            return true;
        }

        self.resampled_paused = paused;

        self.debug_assert_state_is_valid();
        paused
    }

    // Trivial getters/setters:

    /// The resampled index of the next frame that will be written.
    ///
    /// The returned value will never be more than *or equal to*
    /// [Self::resampled_duration].
    pub const fn playhead(&self) -> usize {
        self.resampled_playhead
    }

    /// The range of this stream that can be played.
    pub const fn clip(&self) -> Clip {
        self.resampled_clip
    }

    /// The number of frames this video has once resampled.
    ///
    /// This value will never be 0. Also see
    /// [Self::resampled_duration_non_zero].
    pub const fn resampled_duration(&self) -> usize {
        self.resampled_duration.get()
    }

    /// The number of frames this video has once resampled.
    pub const fn resampled_duration_non_zero(&self) -> NonZeroUsize {
        self.resampled_duration
    }

    /// The number of frames this video had before it was resampled.
    ///
    /// This value will never be 0. Also see [Self::src_duration_non_zero].
    pub const fn src_duration(&self) -> usize {
        self.src_duration_non_zero().get()
    }

    /// The number of frames this video had before it was resampled.
    pub const fn src_duration_non_zero(&self) -> NonZeroUsize {
        self.ffmpeg_video.duration_non_zero()
    }

    /// The resampled [Fps] playback speed of this video.
    pub const fn target_fps(&self) -> Fps {
        self.target_fps
    }

    /// The multipler that changes the source playback speed of this stream.
    pub const fn playback_speed(&self) -> Fps {
        self.playback_speed
    }

    /// Whether or not the stream is paused.
    pub const fn paused(&self) -> bool {
        self.resampled_paused
    }

    /// Whether or not the stream will loop.
    pub const fn will_loop(&self) -> bool {
        self.will_loop
    }

    /// Set whether or not the stream will loop.
    pub fn set_will_loop(&mut self, will_loop: bool) {
        self.debug_assert_state_is_valid();
        self.will_loop = will_loop;
        self.debug_assert_state_is_valid();
    }

    /// The intended (native) [Fps] playback speed of this video.
    pub const fn src_fps(&self) -> Fps {
        self.ffmpeg_video.src_fps()
    }

    /// The intended (native) dimensions of the frames in this video.
    pub const fn src_dimensions(&self) -> Dimensions {
        self.ffmpeg_video.src_dimensions()
    }

    /// The dimensions of the frames that will be produced.
    pub const fn dest_dimensions(&self) -> Dimensions {
        self.ffmpeg_video.dest_dimensions()
    }

    /// Set how frames should be rescaled if needed.
    ///
    /// If `dest_dimensions` is the same as [Self::src_dimensions],
    /// [Self::rescale] will return [None].
    pub fn set_rescale(
        &mut self,
        dest_dimensions: Dimensions,
        rescale_method: RescaleMethod,
    ) -> FFmpegResult<()> {
        self.debug_assert_state_is_valid();

        let rescale_result = self
            .ffmpeg_video
            .set_rescale(dest_dimensions, rescale_method);

        self.debug_assert_state_is_valid();
        rescale_result
    }

    /// The [RescaleMethod] being used if [Self::src_dimensions] don't match
    /// [Self::dest_dimensions].
    pub const fn rescale_method(&self) -> Option<RescaleMethod> {
        self.ffmpeg_video.rescale_method()
    }

    // Helpers:

    fn step_frame_wrapper<F: FnOnce(&mut Self) -> R, R>(&mut self, f: F) -> R {
        // Pause video if we're at the last frame and not looping.
        self.last_frame_played = self.resampled_playhead == self.resampled_clip.end;
        if self.last_frame_played && !self.will_loop {
            self.resampled_paused = true;
        }

        let ret = f(self);

        if !self.resampled_paused {
            self.resampled_playhead += 1;

            // Loop back to the start.
            if self.resampled_playhead >= self.resampled_clip.end {
                debug_assert!(self.will_loop);
                self.resampled_playhead = self.resampled_clip.start;
                self.last_frame_played = false;
            }
        }

        ret
    }

    /// Updates the resampler, the target FPS, the playback speed, the duration,
    /// the clip, and the playhead.
    fn reconfigure_resampler_and_affected_fields(
        &mut self,
        new_target_fps: Fps,
        new_playback_speed: Fps,
    ) {
        self.debug_assert_state_is_valid();

        let old_resampler = self.fps_resampler;

        let new_resampler = Resampler::new(
            self.ffmpeg_video.src_fps() * new_playback_speed,
            new_target_fps,
        );

        let new_duration = NonZeroUsize::new(new_resampler.duration(self.ffmpeg_video.duration()))
            .unwrap_or(NonZeroUsize::MIN);

        let new_clip_start =
            new_resampler.translate_old_dest_idx(old_resampler, self.resampled_clip.start);
        let new_clip_end =
            new_resampler.translate_old_dest_idx(old_resampler, self.resampled_clip.end);
        let new_clip = Clip::new(new_clip_start..=new_clip_end).fix(new_duration);

        let new_playhead = new_resampler
            .translate_old_dest_idx(old_resampler, self.resampled_playhead)
            .clamp(new_clip.start, new_clip.end);

        self.block_debug_checks();
        self.fps_resampler = new_resampler;
        self.resampled_duration = new_duration;
        self.resampled_clip = new_clip;
        self.seek_playhead(new_playhead);
        self.unblock_debug_checks();

        self.debug_assert_state_is_valid();
    }

    /// Does some assertions (only in debug mode) that check that the object's
    /// state is valid.
    #[cfg_attr(not(debug_assertions), inline(always))]
    const fn debug_assert_state_is_valid(&self) {
        #[cfg(debug_assertions)]
        if self.debug_check_blocks > 0 {
            return;
        }

        // The clip should be within the video's bounds.
        debug_assert!(self.resampled_clip.end < self.resampled_duration());
        debug_assert!(self.resampled_clip.start <= self.resampled_clip.end);

        // The playhead should be within the clip.
        debug_assert!(self.resampled_clip.contains(self.resampled_playhead));

        // The last frame shouldn't be outside of the src video's bounds.
        debug_assert!(
            self.fps_resampler.resample(self.resampled_duration() - 1) < self.src_duration()
        );

        // If we've played the last frame and we're still at the last frame we
        // should be paused.
        if self.last_frame_played && self.resampled_playhead == self.resampled_clip.end {
            debug_assert!(self.resampled_paused);
            debug_assert!(self.ffmpeg_video.paused());
        }

        // The inner video should have a valid playhead.
        debug_assert!(self.ffmpeg_video.playhead() < self.src_duration());
    }

    #[cfg_attr(not(debug_assertions), inline(always))]
    fn block_debug_checks(&mut self) {
        #[cfg(debug_assertions)]
        {
            self.debug_check_blocks += 1;
        }
    }

    #[cfg_attr(not(debug_assertions), inline(always))]
    fn unblock_debug_checks(&mut self) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(self.debug_check_blocks != 0);
            self.debug_check_blocks -= 1;
        }
    }
}
