use serde::{de::Error, Deserializer, Serializer};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

/// Describes an OS-path but with a few additional constraints:
///  - just "Normal" Components (1-N folders + 1 file)
///  - cannot start with "..", "~", "/" or any other special character for that matter
///  - at least one item
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct MatchablePath(Vec<String>);

impl MatchablePath {
    /// panics if content is empty
    pub fn new(vec: Vec<String>) -> Self {
        if vec.is_empty() {
            panic!("Trying to construct matchable path from empty vector");
        }
        MatchablePath(vec)
    }

    pub fn get(&self) -> &Vec<String> {
        &self.0
    }

    pub fn resolve(&self, root: &Path) -> PathBuf {
        root.join(
            self.0
                .iter()
                .map(|part| OsStr::new(part))
                .collect::<PathBuf>(),
        )
    }

    pub fn to_serialized_string(&self) -> String {
        self.0.join("/")
    }
}

impl From<&Path> for MatchablePath {
    fn from(path: &Path) -> Self {
        let vec = path
            .components()
            .filter(|comp| match comp {
                Component::Normal(_) => true,
                _ => false,
            })
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .fold(vec![], |mut acc, cur| {
                acc.push(cur);
                acc
            });

        MatchablePath::new(vec)
    }
}

impl From<PathBuf> for MatchablePath {
    fn from(path: PathBuf) -> Self {
        MatchablePath::from(path.as_path())
    }
}

impl From<Vec<&str>> for MatchablePath {
    fn from(value: Vec<&str>) -> Self {
        MatchablePath::from(
            value
                .into_iter()
                .map(|item| item.to_string())
                .collect::<Vec<String>>(),
        )
    }
}

impl From<Vec<String>> for MatchablePath {
    fn from(value: Vec<String>) -> Self {
        MatchablePath(
            value
                .into_iter()
                .map(|item| item.trim().to_string())
                .filter(|item| {
                    let option = item.chars().next();
                    !item.is_empty() && !option.unwrap().is_ascii_punctuation()
                })
                .collect(),
        )
    }
}

impl From<&str> for MatchablePath {
    fn from(value: &str) -> Self {
        MatchablePath::from(Path::new(value))
    }
}

impl Serialize for MatchablePath {
    /// maybe I shouldn't do it that way but who cares...
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_serialized_string())
    }
}

impl<'de> Deserialize<'de> for MatchablePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw: String = String::deserialize(deserializer)?;
        let path = Path::new(&raw);

        // Convert & Validate
        let matchable = MatchablePath::from(path);
        if matchable.0.is_empty() {
            return Err(Error::custom(
                "Invalid path: must contain at least one valid component",
            ));
        }

        Ok(matchable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn check_os_strings() {
        let one = OsString::from("./foo/bar/file.txt");
        let two = OsString::from(".\\foo\\bar\\file.txt");
        assert_ne!(one, two); // would have been too nice to be true
    }

    #[test]
    fn should_match_different_versions() {
        let one = MatchablePath::from(Path::new("./foo/bar/file.txt"));
        let two = MatchablePath::from(Path::new(".\\foo\\bar\\file.txt"));
        assert_eq!(one, two);
        let three = MatchablePath::from(Path::new("foo/bar\\file.txt"));
        assert_eq!(three, two);
    }

    #[test]
    fn should_serialize() {
        let path = MatchablePath::from(Path::new("dir1/dir2/file.txt"));
        let serialized = serde_json::to_string(&path).unwrap();
        assert_eq!(serialized, "\"dir1/dir2/file.txt\"");
    }

    #[test]
    fn should_deserialize() {
        let json = "\"dir1/dir2/file.txt\"";
        let deserialized: MatchablePath = serde_json::from_str(json).unwrap();
        assert_eq!(
            deserialized,
            MatchablePath::from(Path::new("dir1/dir2/file.txt"))
        );
    }

    #[test]
    fn should_deserialize_and_deal_with_path_traversal_attack() {
        let json = "\"../dir1/dir2/file.txt\"";
        let deserialized = serde_json::from_str::<MatchablePath>(json).unwrap();
        assert_eq!(
            MatchablePath::from(Path::new("dir1/dir2/file.txt")),
            deserialized
        );
    }

    #[test]
    fn should_resolve() {
        let path = MatchablePath::from(Path::new("dir1/dir2/file.txt"));
        let other = Path::new("./some/path");

        let resolved = path.resolve(other);

        assert_eq!(Path::new("./some/path/dir1/dir2/file.txt"), resolved)
    }
}
