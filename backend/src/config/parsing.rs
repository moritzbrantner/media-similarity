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
