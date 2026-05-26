#[cfg(test)]
mod tests {
    use std::fs;

    use image::{ImageBuffer, Rgb};

    use super::{build_image_sources, minio_uri, object_relative_path, s3_uri, ImageSource};
    use crate::config::Settings;

    #[test]
    fn build_sources_defaults_to_source_image_dir() {
        let settings = Settings::default();
        let sources = build_image_sources(&settings);
        assert_eq!(sources.len(), 1);
        assert!(matches!(sources[0], ImageSource::Local(_)));
    }

    #[tokio::test]
    async fn local_folder_source_yields_metadata_and_loads_images() {
        let dir = tempfile_dir();
        let image_path = dir.join("sample.jpg");
        ImageBuffer::from_pixel(64, 48, Rgb([1_u8, 2_u8, 3_u8]))
            .save(&image_path)
            .unwrap();
        let settings = Settings {
            source_image_dir: dir.clone(),
            ..Settings::default()
        };
        let source = build_image_sources(&settings).remove(0);
        let items = source.iter_images().await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source_type, "local");
        assert_eq!(items[0].relative_path, "sample.jpg");
        let media = items[0].load_media(&settings).await.unwrap();
        assert_eq!((media.width, media.height), (64, 48));
    }

    #[tokio::test]
    async fn local_folder_source_yields_video_files() {
        let dir = tempfile_dir();
        fs::write(dir.join("clip.mp4"), b"not a real video").unwrap();
        let settings = Settings {
            source_image_dir: dir.clone(),
            ..Settings::default()
        };
        let source = build_image_sources(&settings).remove(0);
        let items = source.iter_images().await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_video());
        assert_eq!(items[0].relative_path, "clip.mp4");
    }

    #[tokio::test]
    async fn local_folder_source_yields_audio_files() {
        let dir = tempfile_dir();
        fs::write(dir.join("song.mp3"), b"not real audio").unwrap();
        let settings = Settings {
            source_image_dir: dir.clone(),
            ..Settings::default()
        };
        let source = build_image_sources(&settings).remove(0);
        let items = source.iter_images().await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_audio());
        assert_eq!(items[0].relative_path, "song.mp3");
    }

    #[tokio::test]
    async fn local_folder_source_yields_pdf_files() {
        let dir = tempfile_dir();
        fs::write(dir.join("paper.PDF"), b"%PDF-1.4\n").unwrap();
        let settings = Settings {
            source_image_dir: dir.clone(),
            ..Settings::default()
        };
        let source = build_image_sources(&settings).remove(0);
        let items = source.iter_images().await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_pdf());
        assert_eq!(items[0].relative_path, "paper.PDF");
    }

    #[tokio::test]
    async fn unsupported_sources_are_reported_without_panicking() {
        let settings = Settings {
            image_sources: vec![
                "minio://images/catalog".to_string(),
                "video:///demo.mp4".to_string(),
            ],
            ..Settings::default()
        };
        let sources = build_image_sources(&settings);
        assert!(matches!(sources[0], ImageSource::ObjectStore(_)));
        assert!(sources[0].iter_images().await.is_err());
        assert!(matches!(sources[1], ImageSource::Unavailable(_)));
    }

    #[test]
    fn object_store_uri_normalization_preserves_bucket_and_prefix() {
        let minio = url::Url::parse("minio://images/catalog/").unwrap();
        let s3 = url::Url::parse("s3://archive/family/2024").unwrap();
        assert_eq!(minio_uri(&minio), "minio://images/catalog");
        assert_eq!(s3_uri(&s3), "s3://archive/family/2024");
    }

    #[test]
    fn object_relative_paths_trim_configured_prefix() {
        assert_eq!(
            object_relative_path("family/2024/photo.jpg", "family/2024"),
            "photo.jpg"
        );
        assert_eq!(
            object_relative_path("other/photo.jpg", "family/2024"),
            "other/photo.jpg"
        );
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("image-sim-rust-source-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
