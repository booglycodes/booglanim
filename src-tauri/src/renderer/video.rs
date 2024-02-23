use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;
use image::DynamicImage;
use serde_json::Value;
use video_rs::{
    ffmpeg::{
        ffi::{av_image_copy, av_image_fill_arrays, AVPixelFormat},
        format::Pixel,
        util::frame::Video as Frame,
        Error as FfmpegError, Rescale,
    },
    Encoder, EncoderSettings, Locator, Time,
};

use winit::dpi::PhysicalSize;

use crate::interface::FrameDescription;

use super::renderers::ImageRenderer;

async fn export(
    image_renderer: &ImageRenderer,
    frames: &Vec<Vec<Value>>,
    images: &HashMap<u32, DynamicImage>,
    fps: usize,
    mut on_frame_complete: impl FnMut(usize) -> (),
    path: String,
) -> Result<()> {
    let mut output = ffmpeg::format::output(&path)?;
    let mut options = ffmpeg::Dictionary::new();
    // Set H264 encoder to the medium preset.
    options.set("preset", "medium");
    // Tune for low latency
    options.set("tune", "zerolatency");
    let codec =
        ffmpeg::encoder::find(ffmpeg::codec::Id::H264).context("could not find h264 codec")?;
    output.add_stream(codec)?;

    for (frame_index, frame) in frames.iter().enumerate().into_iter() {}

    Ok(())
}

pub async fn export_video(
    image_renderer: &ImageRenderer,
    frames: &Vec<FrameDescription>,
    images: &HashMap<u32, DynamicImage>,
    fps: usize,
    mut on_frame_complete: impl FnMut(usize) -> (),
    path: String,
) {
    let duration = Time::from_nth_of_a_second(fps);
    let mut position = Time::zero();
    let size = image_renderer.size();
    let (w, h) = (size.width as usize, size.height as usize);
    let destination: Locator = PathBuf::from(path).into();
    let settings = EncoderSettings::for_h264_yuv420p(w, h, true);
    let mut encoder = Encoder::new(&destination, settings).unwrap();
    for (frame_index, frame) in frames.iter().enumerate().into_iter() {
        let frame = image_renderer.render(frame, images).await;
        let mut buf = frame.into_raw();
        let mut i = 0;
        buf.retain(|_| {
            i += 1;
            i % 4 != 0
        });

        position = position.aligned_with(&duration).add();
        let mut frame = unsafe { av_img_frame(buf, size) }.unwrap();
        let (time, time_base) = position.clone().into_parts();
        frame.set_pts(time.map(|time| time.rescale(time_base, encoder.time_base())));
        encoder.encode_raw(frame).unwrap();
        on_frame_complete(frame_index);
    }
    encoder.finish().unwrap();
}

unsafe fn av_img_frame(buf: Vec<u8>, size: PhysicalSize<u32>) -> Result<Frame, FfmpegError> {
    let PhysicalSize { width, height } = size;

    // Temporary frame structure to place correctly formatted data and linesize stuff in, which
    // we'll copy later.
    let mut frame_tmp = Frame::empty();
    let frame_tmp_ptr = frame_tmp.as_mut_ptr();

    // This does not copy the data, but it sets the `frame_tmp` data and linesize pointers
    // correctly.
    let bytes_copied = av_image_fill_arrays(
        (*frame_tmp_ptr).data.as_ptr() as *mut *mut u8,
        (*frame_tmp_ptr).linesize.as_ptr() as *mut i32,
        buf.as_ptr(),
        AVPixelFormat::AV_PIX_FMT_RGB24,
        width as i32,
        height as i32,
        1,
    );

    if bytes_copied != buf.len() as i32 {
        return Err(FfmpegError::from(bytes_copied));
    }

    let mut frame = Frame::new(Pixel::RGB24, width, height);
    let frame_ptr = frame.as_mut_ptr();

    // Do the actual copying.
    av_image_copy(
        (*frame_ptr).data.as_ptr() as *mut *mut u8,
        (*frame_ptr).linesize.as_ptr() as *mut i32,
        (*frame_tmp_ptr).data.as_ptr() as *mut *const u8,
        (*frame_tmp_ptr).linesize.as_ptr(),
        AVPixelFormat::AV_PIX_FMT_RGB24,
        width as i32,
        height as i32,
    );

    Ok(frame)
}

fn audio() {}
