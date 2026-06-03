use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use image::RgbImage;
use uuid::Uuid;
use video_analysis_core::{DetectionResult, FramePosition, Scene, ScenePipeline, VideoSource};
use video_analysis_detectors::ContentDetector;
use video_analysis_ffmpeg::FfmpegVideoSource;
use video_analysis_split::{build_split_plan, SplitOptions};

use crate::config::Settings;
use crate::workers::media::image_io::image_id_for_uri;
use crate::workers::media::media::{DecodedMedia, MediaFrame, MediaKind};

#[derive(Clone, Debug)]
pub struct DecodedVideoScene {
    pub scene_index: usize,
    pub start: FramePosition,
    pub end: FramePosition,
    pub clip_url: Option<String>,
    pub media: DecodedMedia,
}

#[derive(Clone, Debug)]
pub struct SourceVideoScene {
    pub scene_index: usize,
    pub start: FramePosition,
    pub end: FramePosition,
    pub full_video_url: Option<String>,
    pub clip_url: Option<String>,
    pub media: DecodedMedia,
}

pub fn is_video_extension(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        ".mp4" | ".mov" | ".m4v" | ".webm" | ".mkv" | ".avi"
    )
}

pub fn is_video_content_type(content_type: &str) -> bool {
    content_type.to_ascii_lowercase().starts_with("video/")
}

pub fn video_upload_path(upload_dir: &Path, filename: Option<&str>) -> PathBuf {
    let extension = filename
        .and_then(|name| Path::new(name).extension())
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .map(|extension| format!(".{}", extension.to_ascii_lowercase()))
        .filter(|extension| is_video_extension(extension))
        .unwrap_or_else(|| ".mp4".to_string());
    upload_dir.join(format!("query-{}{extension}", Uuid::new_v4()))
}

pub fn decode_video_scenes(
    path: &Path,
    settings: &Settings,
) -> Result<Vec<DecodedVideoScene>, String> {
    let detection = detect_scenes(path)?;
    let scenes = if detection.scenes.is_empty() {
        whole_video_scene(path)?
    } else {
        detection.scenes
    };

    let scene_media = sample_scene_media(path, &scenes, settings)?;
    Ok(scene_media
        .into_iter()
        .map(|(scene_index, scene, media)| DecodedVideoScene {
            scene_index,
            start: scene.start,
            end: scene.end,
            clip_url: None,
            media,
        })
        .collect())
}

pub fn decode_source_video_scenes(
    path: &Path,
    id_base: &str,
    settings: &Settings,
) -> Result<Vec<SourceVideoScene>, String> {
    decode_source_video_scenes_cancellable(path, id_base, settings, || false)
}

pub fn decode_source_video_scenes_cancellable(
    path: &Path,
    id_base: &str,
    settings: &Settings,
    mut is_cancelled: impl FnMut() -> bool,
) -> Result<Vec<SourceVideoScene>, String> {
    check_cancelled(&mut is_cancelled)?;
    let detection = detect_scenes(path)?;
    check_cancelled(&mut is_cancelled)?;
    let scenes = if detection.scenes.is_empty() {
        whole_video_scene_cancellable(path, &mut is_cancelled)?
    } else {
        detection.scenes
    };

    let video_id = image_id_for_uri(id_base);
    check_cancelled(&mut is_cancelled)?;
    let full_video_url = expose_source_video(path, &video_id, settings)?;
    check_cancelled(&mut is_cancelled)?;
    let clip_urls =
        split_source_scenes_cancellable(path, &scenes, &video_id, settings, &mut is_cancelled)?;
    check_cancelled(&mut is_cancelled)?;
    let scene_media = sample_scene_media_cancellable(path, &scenes, settings, &mut is_cancelled)?;
    Ok(scene_media
        .into_iter()
        .zip(clip_urls)
        .map(|((scene_index, scene, media), clip_url)| SourceVideoScene {
            scene_index,
            start: scene.start,
            end: scene.end,
            full_video_url: full_video_url.clone(),
            clip_url,
            media,
        })
        .collect())
}

fn detect_scenes(path: &Path) -> Result<DetectionResult, String> {
    let mut source = FfmpegVideoSource::open(path).map_err(video_error)?;
    let detector = ContentDetector::new(27.0, 15);
    let mut pipeline = ScenePipeline::builder()
        .detector(detector)
        .start_in_scene(true)
        .build()
        .map_err(video_error)?;
    pipeline.detect(&mut source).map_err(video_error)
}

fn whole_video_scene(path: &Path) -> Result<Vec<Scene>, String> {
    whole_video_scene_cancellable(path, &mut || false)
}

fn whole_video_scene_cancellable(
    path: &Path,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<Vec<Scene>, String> {
    let mut source = FfmpegVideoSource::open(path).map_err(video_error)?;
    let fps = source.frame_rate();
    let mut first = None;
    let mut last = None;
    while let Some(frame) = source.next_frame().map_err(video_error)? {
        check_cancelled(is_cancelled)?;
        first.get_or_insert(frame.position);
        last = Some(frame.position);
    }
    let start = first.unwrap_or_else(|| FramePosition::from_frame_index(0, fps));
    let end = last
        .map(|position| FramePosition::from_frame_index(position.frame_index + 1, fps))
        .unwrap_or_else(|| FramePosition::from_frame_index(1, fps));
    Ok(vec![Scene { start, end }])
}

fn split_source_scenes_cancellable(
    path: &Path,
    scenes: &[Scene],
    video_id: &str,
    settings: &Settings,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<Vec<Option<String>>, String> {
    let output_dir = settings.upload_dir.join("source-scenes").join(video_id);
    let options = SplitOptions {
        output_dir,
        template: "scene-$SCENE_NUMBER.mp4".to_string(),
        video_name: Some(video_id.to_string()),
        ..SplitOptions::default()
    };
    let plan = build_split_plan(path, scenes, &options).map_err(video_error)?;
    std::fs::create_dir_all(&options.output_dir).map_err(|error| error.to_string())?;
    let mut outputs = Vec::new();
    for job in &plan.jobs {
        check_cancelled(is_cancelled)?;
        let mut command = Command::new("ffmpeg");
        command
            .args(job.command_args(&plan.input_video_path))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let status = wait_for_command(&mut command, is_cancelled)?;
        if !status.success() {
            return Err(format!(
                "ffmpeg failed while writing `{}`",
                job.output_path.display()
            ));
        }
        outputs.push(job.output_path.clone());
    }
    Ok(outputs
        .into_iter()
        .map(|output| {
            output
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| format!("/uploads/source-scenes/{video_id}/{name}"))
        })
        .collect())
}

fn expose_source_video(
    path: &Path,
    video_id: &str,
    settings: &Settings,
) -> Result<Option<String>, String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .unwrap_or("mp4");
    let output_dir = settings.upload_dir.join("source-videos");
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    let output_path = output_dir.join(format!("{video_id}.{extension}"));
    if !output_path.exists() {
        match fs::hard_link(path, &output_path) {
            Ok(()) => {}
            Err(_) => {
                fs::copy(path, &output_path).map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(output_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("/uploads/source-videos/{name}")))
}

fn sample_scene_media(
    path: &Path,
    scenes: &[Scene],
    settings: &Settings,
) -> Result<Vec<(usize, Scene, DecodedMedia)>, String> {
    sample_scene_media_cancellable(path, scenes, settings, &mut || false)
}

fn sample_scene_media_cancellable(
    path: &Path,
    scenes: &[Scene],
    settings: &Settings,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<Vec<(usize, Scene, DecodedMedia)>, String> {
    let mut source = FfmpegVideoSource::open(path).map_err(video_error)?;
    let frame_delay_ms = frame_delay_ms(source.frame_rate());
    let stride = settings.video_frame_stride.max(1) as u64;
    let max_frames_per_scene = settings
        .video_max_frames
        .map(|value| value as usize)
        .unwrap_or(settings.gif_sample_frames)
        .max(1);
    let mut scene_frames = vec![Vec::<MediaFrame>::new(); scenes.len()];
    let mut scene_index = 0_usize;

    while let Some(frame) = source.next_frame().map_err(video_error)? {
        check_cancelled(is_cancelled)?;
        while scene_index < scenes.len()
            && frame.position.frame_index >= scenes[scene_index].end.frame_index
        {
            scene_index += 1;
        }
        if scene_index >= scenes.len() {
            break;
        }

        let scene = &scenes[scene_index];
        if frame.position.frame_index < scene.start.frame_index {
            continue;
        }
        if scene_frames[scene_index].len() >= max_frames_per_scene {
            continue;
        }
        let offset = frame
            .position
            .frame_index
            .saturating_sub(scene.start.frame_index);
        if offset != 0 && offset % stride != 0 {
            continue;
        }

        scene_frames[scene_index].push(MediaFrame {
            image: rgb_image_from_frame(&frame)?,
            delay_ms: frame_delay_ms,
        });
    }

    scenes
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, scene)| {
            let frames = scene_frames[index].clone();
            if frames.is_empty() {
                return Err(format!(
                    "Could not sample a representative frame for scene {}",
                    index + 1
                ));
            }
            let poster = frames[0].image.clone();
            let duration_ms = ((scene.end.timestamp.seconds() - scene.start.timestamp.seconds())
                .max(0.0)
                * 1000.0)
                .round() as u32;
            Ok((
                index,
                scene,
                DecodedMedia {
                    kind: MediaKind::VideoScene,
                    width: poster.width(),
                    height: poster.height(),
                    frame_count: Some(frames.len() as u32),
                    duration_ms: Some(duration_ms),
                    poster,
                    sampled_frames: frames.clone(),
                    preview_frames: frames,
                    audio_analysis: None,
                },
            ))
        })
        .collect()
}

fn wait_for_command(
    command: &mut Command,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<std::process::ExitStatus, String> {
    let mut child = command.spawn().map_err(video_error)?;
    loop {
        check_cancelled(is_cancelled).inspect_err(|_| {
            let _ = child.kill();
            let _ = child.wait();
        })?;
        match child.try_wait().map_err(video_error)? {
            Some(status) => return Ok(status),
            None => thread::sleep(Duration::from_millis(50)),
        }
    }
}

fn check_cancelled(is_cancelled: &mut impl FnMut() -> bool) -> Result<(), String> {
    if is_cancelled() {
        Err("job cancelled".to_string())
    } else {
        Ok(())
    }
}

fn rgb_image_from_frame(frame: &video_analysis_core::OwnedVideoFrame) -> Result<RgbImage, String> {
    if frame.pixel_format != video_analysis_core::PixelFormat::Rgb24 {
        return Err("Only RGB24 video frames are supported".to_string());
    }
    if frame.stride != frame.width as usize * 3 {
        return Err("Only tightly packed RGB24 video frames are supported".to_string());
    }
    RgbImage::from_raw(frame.width, frame.height, frame.data.clone())
        .ok_or_else(|| "Video frame buffer did not match frame dimensions".to_string())
}

fn frame_delay_ms(fps: num_rational::Rational64) -> u32 {
    let numerator = *fps.numer() as f64;
    let denominator = *fps.denom() as f64;
    if numerator <= 0.0 || denominator <= 0.0 {
        return 100;
    }
    (1000.0 / (numerator / denominator)).round().max(1.0) as u32
}

fn video_error(error: impl std::fmt::Display) -> String {
    let message = error.to_string();
    if message.contains("No such file or directory") || message.contains("failed to start") {
        format!("{message}. Video upload support requires ffmpeg and ffprobe on PATH")
    } else {
        message
    }
}

pub fn write_video_upload(path: &Path, raw: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, raw).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::decode_source_video_scenes_cancellable;
    use crate::config::Settings;

    #[test]
    fn source_video_decode_stops_before_opening_cancelled_work() {
        let settings = Settings::default();
        let error = decode_source_video_scenes_cancellable(
            std::path::Path::new("/does/not/exist.mp4"),
            "cancelled-video",
            &settings,
            || true,
        )
        .unwrap_err();

        assert_eq!(error, "job cancelled");
    }
}
