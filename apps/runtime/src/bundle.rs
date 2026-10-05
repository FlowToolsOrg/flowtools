use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: String,
    pub sha256: String,
}

pub fn verify(root: &Path, artifact: &Artifact) -> Result<PathBuf, &'static str> {
    let relative = Path::new(&artifact.path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || artifact.sha256.len() != 64
        || !artifact.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("BUNDLE_INVALID");
    }
    let path = root.join(relative);
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor).map_err(|_| "BUNDLE_MISSING")?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("BUNDLE_INVALID");
            }
        }
        if metadata.file_type().is_symlink() {
            return Err("BUNDLE_INVALID");
        }
    }
    if !path.is_file() {
        return Err("BUNDLE_INVALID");
    }
    let bytes = std::fs::read(&path).map_err(|_| "BUNDLE_MISSING")?;
    if format!("{:x}", Sha256::digest(bytes)) != artifact.sha256 {
        return Err("BUNDLE_INTEGRITY_FAILED");
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_traversal_missing_and_changed_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "flowtools-validation-bundle-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("fixed"), b"original").unwrap();
        let good = Artifact {
            path: "fixed".into(),
            sha256: format!("{:x}", Sha256::digest(b"original")),
        };
        assert!(verify(&root, &good).is_ok());
        assert_eq!(
            verify(
                &root,
                &Artifact {
                    path: "../fixed".into(),
                    sha256: good.sha256.clone()
                }
            ),
            Err("BUNDLE_INVALID")
        );
        assert_eq!(
            verify(
                &root,
                &Artifact {
                    path: "missing".into(),
                    sha256: good.sha256.clone()
                }
            ),
            Err("BUNDLE_MISSING")
        );
        std::fs::write(root.join("fixed"), b"tampered").unwrap();
        assert_eq!(verify(&root, &good), Err("BUNDLE_INTEGRITY_FAILED"));
    }
}
