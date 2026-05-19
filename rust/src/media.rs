use image::RgbImage;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MediaKind {
    StaticImage,
    AnimatedGif,
    VideoScene,
}

impl MediaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::StaticImage => "static_image",
            Self::AnimatedGif => "animated_gif",
            Self::VideoScene => "video_scene",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MediaFrame {
    pub image: RgbImage,
    pub delay_ms: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedMedia {
    pub kind: MediaKind,
    pub width: u32,
    pub height: u32,
    pub frame_count: Option<u32>,
    pub duration_ms: Option<u32>,
    pub poster: RgbImage,
    pub sampled_frames: Vec<MediaFrame>,
    pub preview_frames: Vec<MediaFrame>,
}
