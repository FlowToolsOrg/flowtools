//! Fixed maintainer-authorized Windows/T1 prototype bundle. No caller paths,
//! selectors, test keys, review flags or formal release approval.
use super::*;
use crate::{
    packages::{BootstrapPins, PackageError, PublicKey},
    protocol::ErrorCode,
};

const ENVELOPE: &[u8] = include_bytes!("../../../assets/builtin-media/package.dsse.json");
const MANIFEST: &[u8] = include_bytes!("../../../assets/builtin-media/manifest.json");
const ARCHIVE: &[u8] = include_bytes!("../../../assets/builtin-media/package.zip");
const ROOT: &[u8] = include_bytes!("../../../assets/builtin-media/root.dsse.json");

fn current_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("UTC clock")
        .as_secs()
}
pub fn trust() -> Result<PackageTrustStore, PackageError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Pins {
        keys: Vec<PublicKey>,
        threshold: usize,
    }
    let pins: Pins =
        serde_json::from_slice(include_bytes!("../../../assets/builtin-media/pins.json"))
            .map_err(|_| PackageError::TrustInvalid)?;
    PackageTrustStore::new(BootstrapPins {
        keys: pins.keys,
        threshold: pins.threshold,
    })
}
/// T0 startup only. Each repeat uses normal current trust and held-image
/// validation. Unknown or foreign trust/install state is preserved and refused.
pub fn ensure_installed(store: &mut DataStore) -> Result<PackageTrustStore, ErrorCode> {
    let trust = trust().map_err(|_| ErrorCode::DependencyArtifactMismatch)?;
    match trust.snapshot(store) {
        Ok(_) => {}
        Err(PackageError::TrustStateRequired) => {
            trust
                .bootstrap(store, ROOT, current_time())
                .map_err(|_| ErrorCode::DependencyArtifactMismatch)?;
        }
        Err(PackageError::TrustRecoveryRequired) => return Err(ErrorCode::RecoveryPending),
        Err(_) => return Err(ErrorCode::DependencyArtifactMismatch),
    }
    let installed = install::install_inert_tool(
        &trust,
        store,
        ToolArchiveInput {
            envelope: ENVELOPE,
            manifest: MANIFEST,
            archive: ARCHIVE,
        },
        current_time,
    )
    .map_err(install::leases::failed)?;
    install::images::materialize_inert_tool(&trust, store, &installed, current_time)
        .map_err(install::leases::failed)?;
    Ok(trust)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_prototype_uses_actual_quorum_and_exact_archive() {
        assert!(REVIEWED_TOOLS.is_empty());
        let trust = trust().unwrap();
        let mut store = DataStore::memory().unwrap();
        let now = current_time();
        trust.bootstrap(&mut store, ROOT, now).unwrap();
        let archive = admit_tool_archive(
            &trust,
            &mut store,
            ToolArchiveInput {
                envelope: ENVELOPE,
                manifest: MANIFEST,
                archive: ARCHIVE,
            },
            || now,
        )
        .unwrap();
        assert_eq!(
            archive.programs(),
            &[ToolProgram::Ffmpeg, ToolProgram::Ffprobe]
        );
        let mut corrupt = ARCHIVE.to_vec();
        corrupt[100] ^= 1;
        assert!(admit_tool_archive(
            &trust,
            &mut store,
            ToolArchiveInput {
                envelope: ENVELOPE,
                manifest: MANIFEST,
                archive: &corrupt
            },
            || now
        )
        .is_err());
        let mut value: serde_json::Value = serde_json::from_slice(ENVELOPE).unwrap();
        value["signatures"] = serde_json::json!([]);
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(admit_tool_archive(
            &trust,
            &mut store,
            ToolArchiveInput {
                envelope: &bytes,
                manifest: MANIFEST,
                archive: ARCHIVE
            },
            || now
        )
        .is_err());
    }
}
