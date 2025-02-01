use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct MatchablePath(pub Vec<String>);

impl From<&Path> for MatchablePath {
    fn from(path: &Path) -> Self {
        let vec = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .fold(vec![], |mut acc, cur| {
                acc.push(cur);
                acc
            });

        MatchablePath(vec)
    }
}

impl From<Vec<&str>> for MatchablePath {
    fn from(value: Vec<&str>) -> Self {
        MatchablePath(value.into_iter().map(|item| item.to_string()).collect())
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
        println!("matchable_path: {:?}", one);
        assert_eq!(one, two);
    }
}
