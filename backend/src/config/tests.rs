#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{parse_extensions, parse_image_sources, parse_media_sources_file, Settings};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn extensions_are_normalized() {
        let parsed = parse_extensions("jpg, .PNG, webp").unwrap();
        assert_eq!(
            parsed.into_iter().collect::<Vec<_>>(),
            vec![".jpg", ".png", ".webp"]
        );
    }

    #[test]
    fn image_sources_accept_delimited_strings_and_json() {
        assert_eq!(
            parse_image_sources("local:///images; minio://bucket/prefix; s3://archive/photos")
                .unwrap(),
            vec![
                "local:///images",
                "minio://bucket/prefix",
                "s3://archive/photos"
            ]
        );
        assert_eq!(
            parse_image_sources(r#"["/images", "video:///clips/demo.mp4"]"#).unwrap(),
            vec!["/images", "video:///clips/demo.mp4"]
        );
    }

    #[test]
    fn media_sources_file_accepts_ignore_style_comments_and_expands_paths() {
        let home = std::env::var("HOME").unwrap_or_default();
        let mut input = "# Default user media folders.\n/srv/audio\n".to_string();
        let mut expected = vec!["/srv/audio".to_string()];
        if !home.is_empty() {
            input.push_str("~/Pictures\n$HOME/Videos\n");
            expected.push(format!("{home}/Pictures"));
            expected.push(format!("{home}/Videos"));
        }
        assert_eq!(parse_media_sources_file(&input).unwrap(), expected);
    }

    #[test]
    fn empty_extensions_are_rejected() {
        assert!(parse_extensions(" , ").is_err());
    }

    #[test]
    fn default_extensions_include_gif() {
        assert!(Settings::default().image_extensions.contains(".gif"));
    }

    #[test]
    fn default_audio_extensions_include_mp3() {
        assert!(Settings::default().audio_extensions.contains(".mp3"));
    }

    #[test]
    fn default_pdf_extensions_include_pdf() {
        assert!(Settings::default().pdf_extensions.contains(".pdf"));
    }

    #[test]
    fn default_bind_addr_is_localhost() {
        assert_eq!(Settings::default().bind_addr, "127.0.0.1:8000");
    }

    #[test]
    fn frontend_serving_is_enabled_by_default() {
        assert!(Settings::default().frontend_serving_enabled);
    }

    #[test]
    fn frontend_serving_can_be_disabled_from_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set([
            ("FRONTEND_SERVING_ENABLED", Some("false")),
            ("IMAGE_SOURCES", Some("/images")),
            ("MEDIA_SOURCES_FILE", None),
            ("MEDIA_SOURCES_SEED_FILE", None),
        ]);

        let settings = Settings::from_env().unwrap();

        assert!(!settings.frontend_serving_enabled);
    }

    #[test]
    fn startup_indexing_is_disabled_by_default() {
        assert!(!Settings::default().startup_indexing_enabled);
    }

    #[test]
    fn startup_indexing_can_be_enabled_from_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set([
            ("STARTUP_INDEXING_ENABLED", Some("true")),
            ("IMAGE_SOURCES", Some("/images")),
            ("MEDIA_SOURCES_FILE", None),
            ("MEDIA_SOURCES_SEED_FILE", None),
        ]);

        let settings = Settings::from_env().unwrap();

        assert!(settings.startup_indexing_enabled);
    }

    #[test]
    fn default_qdrant_http_settings_are_bounded() {
        let settings = Settings::default();

        assert_eq!(settings.qdrant_request_timeout_ms, 30_000);
        assert_eq!(settings.qdrant_connect_timeout_ms, 2_000);
        assert_eq!(settings.qdrant_retry_attempts, 2);
        assert_eq!(settings.qdrant_retry_backoff_ms, 100);
    }

    #[test]
    fn qdrant_http_settings_are_loaded_from_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set([
            ("QDRANT_REQUEST_TIMEOUT_MS", Some("45000")),
            ("QDRANT_CONNECT_TIMEOUT_MS", Some("3000")),
            ("QDRANT_RETRY_ATTEMPTS", Some("4")),
            ("QDRANT_RETRY_BACKOFF_MS", Some("250")),
            ("IMAGE_SOURCES", Some("/images")),
            ("MEDIA_SOURCES_FILE", None),
            ("MEDIA_SOURCES_SEED_FILE", None),
        ]);

        let settings = Settings::from_env().unwrap();

        assert_eq!(settings.qdrant_request_timeout_ms, 45_000);
        assert_eq!(settings.qdrant_connect_timeout_ms, 3_000);
        assert_eq!(settings.qdrant_retry_attempts, 4);
        assert_eq!(settings.qdrant_retry_backoff_ms, 250);
    }

    #[test]
    fn invalid_qdrant_http_settings_are_rejected() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set([
            ("QDRANT_REQUEST_TIMEOUT_MS", Some("999")),
            ("QDRANT_CONNECT_TIMEOUT_MS", None),
            ("QDRANT_RETRY_ATTEMPTS", None),
            ("QDRANT_RETRY_BACKOFF_MS", None),
            ("IMAGE_SOURCES", Some("/images")),
            ("MEDIA_SOURCES_FILE", None),
            ("MEDIA_SOURCES_SEED_FILE", None),
        ]);

        let error = Settings::from_env().unwrap_err();

        assert!(error.contains("QDRANT_REQUEST_TIMEOUT_MS"));
        assert!(error.contains("between 1000 and 600000"));
    }

    #[test]
    fn qdrant_connect_timeout_must_not_exceed_request_timeout() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set([
            ("QDRANT_REQUEST_TIMEOUT_MS", Some("1000")),
            ("QDRANT_CONNECT_TIMEOUT_MS", Some("2000")),
            ("QDRANT_RETRY_ATTEMPTS", None),
            ("QDRANT_RETRY_BACKOFF_MS", None),
            ("IMAGE_SOURCES", Some("/images")),
            ("MEDIA_SOURCES_FILE", None),
            ("MEDIA_SOURCES_SEED_FILE", None),
        ]);

        let error = Settings::from_env().unwrap_err();

        assert!(error.contains("QDRANT_CONNECT_TIMEOUT_MS"));
        assert!(error.contains("QDRANT_REQUEST_TIMEOUT_MS"));
    }

    #[test]
    fn image_sources_env_wins_over_media_source_files() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = TestTempDir::new();
        let target = temp.path().join("target-sources.txt");
        let seed = temp.path().join("seed-sources.txt");
        fs::write(&target, "/target\n").unwrap();
        fs::write(&seed, "/seed\n").unwrap();
        let _env = EnvGuard::set([
            ("IMAGE_SOURCES", Some("/env-a,/env-b")),
            ("MEDIA_SOURCES_FILE", Some(target.to_str().unwrap())),
            ("MEDIA_SOURCES_SEED_FILE", Some(seed.to_str().unwrap())),
        ]);

        let settings = Settings::from_env().unwrap();

        assert_eq!(settings.image_sources, vec!["/env-a", "/env-b"]);
        assert_eq!(settings.media_sources_file, target);
        assert_eq!(settings.media_sources_seed_file, Some(seed));
    }

    #[test]
    fn missing_media_source_target_loads_seed_file() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = TestTempDir::new();
        let target = temp.path().join("target-sources.txt");
        let seed = temp.path().join("seed-sources.txt");
        fs::write(&seed, "/seed\n").unwrap();
        let _env = EnvGuard::set([
            ("IMAGE_SOURCES", None),
            ("MEDIA_SOURCES_FILE", Some(target.to_str().unwrap())),
            ("MEDIA_SOURCES_SEED_FILE", Some(seed.to_str().unwrap())),
        ]);

        let settings = Settings::from_env().unwrap();

        assert_eq!(settings.image_sources, vec!["/seed"]);
        assert_eq!(settings.media_sources_file, target);
        assert_eq!(settings.media_sources_seed_file, Some(seed));
    }

    #[test]
    fn existing_media_source_target_wins_over_seed_file() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = TestTempDir::new();
        let target = temp.path().join("target-sources.txt");
        let seed = temp.path().join("seed-sources.txt");
        fs::write(&target, "/target\n").unwrap();
        fs::write(&seed, "/seed\n").unwrap();
        let _env = EnvGuard::set([
            ("IMAGE_SOURCES", None),
            ("MEDIA_SOURCES_FILE", Some(target.to_str().unwrap())),
            ("MEDIA_SOURCES_SEED_FILE", Some(seed.to_str().unwrap())),
        ]);

        let settings = Settings::from_env().unwrap();

        assert_eq!(settings.image_sources, vec!["/target"]);
    }

    #[test]
    fn explicit_missing_media_source_target_without_seed_errors() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = TestTempDir::new();
        let target = temp.path().join("missing-sources.txt");
        let _env = EnvGuard::set([
            ("IMAGE_SOURCES", None),
            ("MEDIA_SOURCES_FILE", Some(target.to_str().unwrap())),
            ("MEDIA_SOURCES_SEED_FILE", None),
        ]);

        let error = Settings::from_env().unwrap_err();

        assert!(error.contains("MEDIA_SOURCES_FILE"));
        assert!(error.contains("file does not exist"));
    }

    #[test]
    fn implicit_missing_media_source_target_without_seed_falls_back_to_default_source_dir() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = TestTempDir::new();
        let source_dir = temp.path().join("sources");
        let _dir = CurrentDirGuard::set(temp.path());
        let _env = EnvGuard::set([
            ("IMAGE_SOURCES", None),
            ("MEDIA_SOURCES_FILE", None),
            ("MEDIA_SOURCES_SEED_FILE", None),
            ("SOURCE_IMAGE_DIR", Some(source_dir.to_str().unwrap())),
        ]);

        let settings = Settings::from_env().unwrap();

        assert!(settings.image_sources.is_empty());
        assert_eq!(
            settings.source_specs(),
            vec![source_dir.to_string_lossy().to_string()]
        );
    }

    struct EnvGuard {
        previous: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn set<const N: usize>(values: [(&'static str, Option<&str>); N]) -> Self {
            let previous = values
                .iter()
                .map(|(name, _)| (*name, std::env::var(name).ok()))
                .collect::<Vec<_>>();
            for (name, value) in values {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
            Self { previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.previous {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "image-sim-config-test-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    struct CurrentDirGuard {
        previous: PathBuf,
    }

    impl CurrentDirGuard {
        fn set(path: &Path) -> Self {
            let previous = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { previous }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.previous);
        }
    }
}
