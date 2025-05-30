use std::error::Error;
use std::fmt::{Display, Formatter};

pub use dist_info_name::DistInfoName;
pub use extra_name::{DefaultExtras, ExtraName};
pub use group_name::{DefaultGroups, GroupName, PipGroupName, DEV_DEPENDENCIES};
pub use package_name::PackageName;

use uv_small_str::SmallString;

mod dist_info_name;
mod extra_name;
mod group_name;
mod package_name;

/// Validate and normalize an unowned package or extra name.
pub(crate) fn validate_and_normalize_ref(
    name: impl AsRef<str>,
) -> Result<SmallString, InvalidNameError> {
    let name = name.as_ref();
    if is_normalized(name)? {
        Ok(SmallString::from(name))
    } else {
        Ok(SmallString::from(normalize(name)?))
    }
}

/// Normalize an unowned package or extra name.
fn normalize(name: &str) -> Result<String, InvalidNameError> {
    let mut normalized = String::with_capacity(name.len());

    let mut last = None;
    for char in name.bytes() {
        match char {
            b'A'..=b'Z' => {
                normalized.push(char.to_ascii_lowercase() as char);
            }
            b'a'..=b'z' | b'0'..=b'9' => {
                normalized.push(char as char);
            }
            b'-' | b'_' | b'.' => {
                match last {
                    // Names can't start with punctuation.
                    None => return Err(InvalidNameError(name.to_string())),
                    Some(b'-' | b'_' | b'.') => {}
                    Some(_) => normalized.push('-'),
                }
            }
            _ => return Err(InvalidNameError(name.to_string())),
        }
        last = Some(char);
    }

    // Names can't end with punctuation.
    if matches!(last, Some(b'-' | b'_' | b'.')) {
        return Err(InvalidNameError(name.to_string()));
    }

    Ok(normalized)
}

/// Returns `true` if the name is already normalized.
fn is_normalized(name: impl AsRef<str>) -> Result<bool, InvalidNameError> {
    let mut last = None;
    for char in name.as_ref().bytes() {
        match char {
            b'A'..=b'Z' => {
                // Uppercase characters need to be converted to lowercase.
                return Ok(false);
            }
            b'a'..=b'z' | b'0'..=b'9' => {}
            b'_' | b'.' => {
                // `_` and `.` are normalized to `-`.
                return Ok(false);
            }
            b'-' => {
                match last {
                    // Names can't start with punctuation.
                    None => return Err(InvalidNameError(name.as_ref().to_string())),
                    Some(b'-') => {
                        // Runs of `-` are normalized to a single `-`.
                        return Ok(false);
                    }
                    Some(_) => {}
                }
            }
            _ => return Err(InvalidNameError(name.as_ref().to_string())),
        }
        last = Some(char);
    }

    // Names can't end with punctuation.
    if matches!(last, Some(b'-' | b'_' | b'.')) {
        return Err(InvalidNameError(name.as_ref().to_string()));
    }

    Ok(true)
}

/// Invalid [`PackageName`] or [`ExtraName`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidNameError(String);

impl InvalidNameError {
    /// Returns the invalid name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for InvalidNameError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Not a valid package or extra name: \"{}\". Names must start and end with a letter or \
            digit and may only contain -, _, ., and alphanumeric characters.",
            self.0
        )
    }
}

impl Error for InvalidNameError {}

/// Path didn't end with `pyproject.toml`
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidPipGroupPathError(String);

impl InvalidPipGroupPathError {
    /// Returns the invalid path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for InvalidPipGroupPathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "The `--group` path is required to end in 'pyproject.toml' for compatibility with pip; got: {}",
            self.0,
        )
    }
}
impl Error for InvalidPipGroupPathError {}

/// Possible errors from reading a [`PipGroupName`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidPipGroupError {
    Name(InvalidNameError),
    Path(InvalidPipGroupPathError),
}

impl Display for InvalidPipGroupError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            InvalidPipGroupError::Name(e) => e.fmt(f),
            InvalidPipGroupError::Path(e) => e.fmt(f),
        }
    }
}
impl Error for InvalidPipGroupError {}
impl From<InvalidNameError> for InvalidPipGroupError {
    fn from(value: InvalidNameError) -> Self {
        Self::Name(value)
    }
}
impl From<InvalidPipGroupPathError> for InvalidPipGroupError {
    fn from(value: InvalidPipGroupPathError) -> Self {
        Self::Path(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize() {
        let inputs = [
            "friendly-bard",
            "Friendly-Bard",
            "FRIENDLY-BARD",
            "friendly.bard",
            "friendly_bard",
            "friendly--bard",
            "friendly-.bard",
            "FrIeNdLy-._.-bArD",
        ];
        for input in inputs {
            assert_eq!(
                validate_and_normalize_ref(input).unwrap().as_ref(),
                "friendly-bard"
            );
        }
    }

    #[test]
    fn check() {
        let inputs = ["friendly-bard", "friendlybard"];
        for input in inputs {
            assert!(is_normalized(input).unwrap(), "{input:?}");
        }

        let inputs = [
            "friendly.bard",
            "friendly.BARD",
            "friendly_bard",
            "friendly--bard",
            "friendly-.bard",
            "FrIeNdLy-._.-bArD",
        ];
        for input in inputs {
            assert!(!is_normalized(input).unwrap(), "{input:?}");
        }
    }

    #[test]
    fn unchanged() {
        // Unchanged
        let unchanged = ["friendly-bard", "1okay", "okay2"];
        for input in unchanged {
            assert_eq!(validate_and_normalize_ref(input).unwrap().as_ref(), input);
            assert!(is_normalized(input).unwrap());
        }
    }

    #[test]
    fn failures() {
        let failures = [
            " starts-with-space",
            "-starts-with-dash",
            "ends-with-dash-",
            "ends-with-space ",
            "includes!invalid-char",
            "space in middle",
            "alpha-α",
        ];
        for input in failures {
            assert!(validate_and_normalize_ref(input).is_err());
            assert!(is_normalized(input).is_err());
        }
    }
}
