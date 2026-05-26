pub fn parse_extensions(value: &str) -> Result<BTreeSet<String>, String> {
    let extensions = value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            if lower.starts_with('.') {
                lower
            } else {
                format!(".{lower}")
            }
        })
        .collect::<BTreeSet<_>>();
    if extensions.is_empty() {
        Err("At least one image extension is required".to_string())
    } else {
        Ok(extensions)
    }
}

pub fn parse_image_sources(value: &str) -> Result<Vec<String>, String> {
    let stripped = value.trim();
    if stripped.is_empty() {
        return Ok(Vec::new());
    }
    if stripped.starts_with('[') {
        let parsed: Vec<String> = serde_json::from_str(stripped)
            .map_err(|error| format!("IMAGE_SOURCES must be a JSON string array: {error}"))?;
        return Ok(parsed
            .into_iter()
            .map(|part| expand_local_source_spec(part.trim()))
            .filter(|part| !part.is_empty())
            .collect());
    }
    for separator in ['\n', ';', ','] {
        if stripped.contains(separator) {
            return Ok(stripped
                .split(separator)
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(expand_local_source_spec)
                .collect());
        }
    }
    Ok(vec![expand_local_source_spec(stripped)])
}

pub fn parse_media_sources_file(value: &str) -> Result<Vec<String>, String> {
    Ok(value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(expand_local_source_spec)
        .filter(|line| !line.is_empty())
        .collect())
}

fn read_media_sources_files(
    target_path: &Path,
    seed_path: Option<&Path>,
    target_required: bool,
) -> Result<Vec<String>, String> {
    match read_media_sources_file(target_path) {
        Ok(sources) => return Ok(sources),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "Could not read MEDIA_SOURCES_FILE {}: {error}",
                target_path.display()
            ));
        }
    }

    if let Some(seed_path) = seed_path {
        return read_media_sources_file(seed_path).map_err(|error| {
            format!(
                "Could not read MEDIA_SOURCES_SEED_FILE {}: {error}",
                seed_path.display()
            )
        });
    }

    if target_required {
        return Err(format!(
            "Could not read MEDIA_SOURCES_FILE {}: file does not exist",
            target_path.display()
        ));
    }

    Ok(Vec::new())
}

fn read_media_sources_file(path: &Path) -> std::io::Result<Vec<String>> {
    fs::read_to_string(path).and_then(|value| {
        parse_media_sources_file(&value)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })
}

fn expand_local_source_spec(value: &str) -> String {
    if has_uri_scheme(value) {
        return value.to_string();
    }

    let expanded = expand_env_vars(value);
    if let Some(home) = home_dir() {
        if expanded == "~" {
            return home;
        }
        if let Some(rest) = expanded.strip_prefix("~/") {
            return format!("{home}/{rest}");
        }
    }
    expanded
}

fn has_uri_scheme(value: &str) -> bool {
    value
        .find(':')
        .map(|index| {
            value[..index].chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            })
        })
        .unwrap_or(false)
}

fn expand_env_vars(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();

    while let Some(character) = chars.next() {
        if character != '$' {
            output.push(character);
            continue;
        }

        if chars.peek() == Some(&'{') {
            chars.next();
            let mut name = String::new();
            for next in chars.by_ref() {
                if next == '}' {
                    break;
                }
                name.push(next);
            }
            output.push_str(&env::var(name).unwrap_or_default());
            continue;
        }

        let mut name = String::new();
        while let Some(next) = chars.peek() {
            if next.is_ascii_alphanumeric() || *next == '_' {
                name.push(*next);
                chars.next();
            } else {
                break;
            }
        }
        if name.is_empty() {
            output.push('$');
        } else {
            output.push_str(&env::var(name).unwrap_or_default());
        }
    }

    output
}

fn home_dir() -> Option<String> {
    optional_string_var("HOME")
}

fn string_var(name: &str, default: String) -> String {
    optional_string_var(name).unwrap_or(default)
}

fn optional_string_var(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn path_var(name: &str, default: PathBuf) -> PathBuf {
    optional_string_var(name)
        .map(PathBuf::from)
        .unwrap_or(default)
}

fn bool_var(name: &str, default: bool) -> bool {
    optional_string_var(name)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn bounded_u32_var(name: &str, default: u32, min: u32, max: u32) -> Result<u32, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<u32>()
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

fn bounded_u64_var(name: &str, default: u64, min: u64, max: u64) -> Result<u64, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<u64>()
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
