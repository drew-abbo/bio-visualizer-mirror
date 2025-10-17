use std::env;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;
use std::process;

use ffmpeg_next::format::{self, Pixel};
use ffmpeg_next::media::Type;
use ffmpeg_next::software::scaling::{context::Context, flag::Flags};
use ffmpeg_next::util::frame::video::Video;

fn main() -> Result<(), ffmpeg_next::Error> {
    // We always need to do this to set up FFmpeg.
    ffmpeg_next::init().unwrap();

    // Open file and parse to figure out what kind of container it is (e.g. MP4,
    // MKV). Once we have this we'll have an easier time figuring out what we
    // can actually use this file for.
    // Called `ictx` which stands for "Input Context". I'd just call it
    // `input_context`.
    let mut ictx = format::input(&get_src_file_path_from_args())?;

    // `input` is a `Stream` and `ictx` is an `Input`.
    let input = ictx
        .streams()
        .best(Type::Video)
        .ok_or(ffmpeg_next::Error::StreamNotFound)?;
    let video_stream_index = input.index();

    // Input parameters (parameters on the stream) define info about the stream
    // (e.g. resolution, pixel format). It tells the decoder what it's looking
    // at.
    // Contexts are parameters and buffers for the decoder. It's a space for the
    // decoder to work.
    // The decoder is the actual "engine" for doing the decoding work. You load
    // packets from the input context `ictx` into it and then you can ask it to
    // pull frames out.
    let context_decoder =
        ffmpeg_next::codec::context::Context::from_parameters(input.parameters())?;
    let mut decoder = context_decoder.decoder().video()?;

    // Used to change frames from one format to another. You can rescale and
    // change the pixel format.
    let mut scaler = Context::get(
        // Src. format:
        decoder.format(),
        decoder.width(),
        decoder.height(),
        // Dest. format:
        Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        // How to do rescaling (only used if src != dest for dimensions):
        Flags::BILINEAR,
    )?;

    // Helper type for saving frames to `.ppm` files.
    let mut frame_saver = FrameSaver::with_fresh_out_dir();

    // These are frame buffers. One for the frame that gets decoded and one to
    // write the "scaled" (changed) frame to.
    let mut decoded = Video::empty();
    let mut rgb_frame = Video::empty();

    // This is a closure that decodes all frames it can from all of the packets
    // we've given the decoder. We call this after we give the decoder packets
    // to pull frames from. Frames are converted to RGB and saved to files.
    let mut receive_and_process_decoded_frames =
        |decoder: &mut ffmpeg_next::decoder::Video| -> Result<(), ffmpeg_next::Error> {
            while decoder.receive_frame(&mut decoded).is_ok() {
                scaler.run(&decoded, &mut rgb_frame)?;

                frame_saver.save_frame_to_file(&rgb_frame);
            }

            Ok(())
        };

    // Iterate over all packets in the file.
    for (stream, packet) in ictx.packets() {
        // If the packet is for a stream we don't care about, skip it.
        if stream.index() == video_stream_index {
            // For each packet, give the packet to the decoder and write all
            // frames to the output.
            decoder.send_packet(&packet)?;
            receive_and_process_decoded_frames(&mut decoder)?;
        }
    }
    // Tell the decoder we have no more packets to give it and that the video
    // is over. It will prepare all of the rest of the frames it may have
    // buffered internally so no frames get left behind.
    decoder.send_eof()?;
    receive_and_process_decoded_frames(&mut decoder)?;

    // `send_packet` and `receive_frame` (also potentially `send_eof`) are the
    // expensive calls here. Everything else is relatively trivial (except
    // parsing the container at the start, but that only happens once).

    // Unless the file is being stored on an HDD instead of an SSD, the
    // bottleneck here is actually the video decoding process (CPU bottleneck).

    // You can configure each decoder to use additional threads, but it spawns
    // and manages them itself and the count can't change (I think). This is not
    // ideal for our use case, since we'll have a dynamically changing number of
    // decoders (1 per source) and we don't want to end up spawning a million
    // threads we don't have control over.
    // Instead, I think we just have a thread pool and then for each frame we
    // just distribute the decoding work across the threads. This should be
    // relatively simple as long as all of the `ffmpeg_next` types we'll share
    // are `Send + Sync` (I think some might not be unfortunately).

    Ok(())
}

const OUT_DIR: &str = "out";

#[derive(Debug, Default)]
struct FrameSaver {
    frame_idx: usize,
}

impl FrameSaver {
    pub fn with_fresh_out_dir() -> Self {
        if Path::new(OUT_DIR).exists() {
            fs::remove_dir_all(OUT_DIR).expect(&format!("Failed to remove `{OUT_DIR}` directory."));
            fs::create_dir(OUT_DIR).expect(&format!("Failed to create `{OUT_DIR}` directory."));
        }

        Self::default()
    }

    pub fn save_frame_to_file(&mut self, frame: &Video) {
        println!("Saving frame {}...", self.frame_idx);

        let out_file_path = format!("{}/frame_{}.ppm", OUT_DIR, self.frame_idx);
        File::create(&out_file_path)
            .and_then(|mut out_file| {
                out_file.write_all(
                    format!("P6\n{} {}\n255\n", frame.width(), frame.height()).as_bytes(),
                )?;
                out_file.write_all(frame.data(0))
            })
            .expect(&format!("Failed to save frame to `{out_file_path}`."));

        self.frame_idx += 1;
    }
}

fn get_src_file_path_from_args() -> String {
    let mut args = env::args();
    let arg0 = args.next().unwrap();
    args.next()
        .and_then(|src_file_path| args.next().is_none().then(|| src_file_path))
        .unwrap_or_else(|| {
            println!("Usage: {arg0} <src>");
            process::exit(1);
        })
}
