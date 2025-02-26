use std::error::Error;
use std::fs;
use std::path::Path;

/// Quick and dirty Cargo.toml version bumping 
/// 
/// grabs the Cargo.toml of the active directory, 
/// changes the line starting with "version" by bumping the patch version by 1, 
/// overwrites the Cargo.toml with the updated version, 
/// any unexpected parsing leads to non-zero exit
fn main() -> Result<(), Box<dyn Error>> {
    let toml_path = Path::new("./Cargo.toml");
    let toml_content = fs::read_to_string(toml_path)?;
    println!("ORIGINAL:\n{}", toml_content);

    let toml_content = toml_content
        .lines()
        .into_iter()
        .map(|line| {
            if line.starts_with("version") {
                bump_version(line, SemVersionPart::Patch)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<String>>()
        .join("\n");

    println!("UPDATED:\n{}", toml_content);
    fs::write(toml_path, toml_content)?;
    Ok(())
}

enum SemVersionPart {
    Major,
    Minor,
    Patch,
}

fn bump_version(original_line: &str, part: SemVersionPart) -> String {
    let (mut major, mut minor, mut patch) = extract_sem_version(original_line);
    let original = format_version_string(major, minor, patch);
    match part {
        SemVersionPart::Major => major += 1,
        SemVersionPart::Minor => minor += 1,
        SemVersionPart::Patch => patch += 1,
    }
    let updated = format_version_string(major, minor, patch);
    println!("bumping version from {} -> {}", original, updated);
    format!("version = \"{}\"", updated)
}

fn extract_sem_version(value: &str) -> (u8, u8, u8) {
    let start = value.find('"').expect("Missing opening quote") + 1;
    let end = value.rfind('"').expect("Missing closing quote");
    let version_str = &value[start..end];
    let mut parts = version_str.split(".");
    (
        parts
            .next()
            .expect("No major version found")
            .parse::<u8>()
            .expect("Cannot parse major version string"),
        parts
            .next()
            .expect("No minor version found")
            .parse::<u8>()
            .expect("Cannot parse minor version string"),
        parts
            .next()
            .expect("No patch version found")
            .parse::<u8>()
            .expect("Cannot parse patch version string"),
    )
}

fn format_version_string(major: u8, minor: u8, patch: u8) -> String {
    format!("{}.{}.{}", major, minor, patch)
}

#[cfg(test)]
mod test {
    use crate::{SemVersionPart, bump_version};

    #[test]
    fn should_bump_patch_version() {
        assert_eq!(
            "version = \"1.1.2\"",
            bump_version("version = \"1.1.1\"", SemVersionPart::Patch)
        );
        assert_eq!(
            "version = \"0.1.1\"",
            bump_version("version = \"0.1.0\"", SemVersionPart::Patch)
        );
        assert_eq!(
            "version = \"0.1.10\"",
            bump_version("version = \"0.1.9\"", SemVersionPart::Patch)
        );
        assert_eq!(
            "version = \"0.1.11\"",
            bump_version("version = \"0.1.10\"", SemVersionPart::Patch)
        );
    }
}
