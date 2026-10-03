fn optional_bounded_u32_var(name: &str, min: u32, max: u32) -> Result<Option<u32>, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<u32>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(Some(parsed))
            }
        }
        None => Ok(None),
    }
}

fn optional_bounded_usize_var(name: &str, min: usize, max: usize) -> Result<Option<usize>, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<usize>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(Some(parsed))
            }
        }
        None => Ok(None),
    }
}

fn bounded_usize_var(name: &str, default: usize, min: usize, max: usize) -> Result<usize, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<usize>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(parsed)
            }
        }
        None => Ok(default),
    }
}

fn bounded_f32_var(name: &str, default: f32, min: f32, max: f32) -> Result<f32, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<f32>()
                .map_err(|_| format!("{name} must be a number"))?;
            if !parsed.is_finite() || parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(parsed)
            }
        }
        None => Ok(default),
    }
}

fn audio_transcription_provider_var(default: String) -> Result<String, String> {
    let value = optional_string_var("AUDIO_TRANSCRIPTION_PROVIDER").unwrap_or(default);
    let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "candle-whisper" => Ok("candle-whisper".to_string()),
        _ => Err(
            "AUDIO_TRANSCRIPTION_PROVIDER must be `candle-whisper`; whisper.cpp is deprecated for managed model setup"
                .to_string(),
        ),
    }
}

fn audio_transcription_model_var(default: String) -> String {
    optional_string_var("AUDIO_TRANSCRIPTION_MODEL_ID")
        .or_else(|| optional_string_var("AUDIO_TRANSCRIPTION_MODEL"))
        .map(|value| canonical_audio_transcription_model_id(&value))
        .unwrap_or(default)
}

fn canonical_audio_transcription_model_id(value: &str) -> String {
    match value.trim() {
        "large-v3-turbo" | "whisper-large-v3-turbo" => {
            "openai/whisper-large-v3-turbo".to_string()
        }
        other => other.to_string(),
    }
}

fn audio_transcription_device_var(default: String) -> Result<String, String> {
    let value = optional_string_var("AUDIO_TRANSCRIPTION_DEVICE").unwrap_or(default);
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "auto" | "cpu" | "cuda" => Ok(normalized),
        _ => Err("AUDIO_TRANSCRIPTION_DEVICE must be one of `auto`, `cpu`, or `cuda`".to_string()),
    }
}

fn audio_transcription_compute_type_var(default: String) -> Result<String, String> {
    let value = optional_string_var("AUDIO_TRANSCRIPTION_COMPUTE_TYPE").unwrap_or(default);
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "automatic" | "auto" => Ok("automatic".to_string()),
        "fp16" | "float16" => Ok("fp16".to_string()),
        "fp32" | "float32" => Ok("fp32".to_string()),
        _ => Err(
            "AUDIO_TRANSCRIPTION_COMPUTE_TYPE must be one of `automatic`, `fp16`, or `fp32`"
                .to_string(),
        ),
    }
}
