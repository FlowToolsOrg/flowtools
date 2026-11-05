//! Opaque, read-only OS objects sharing the G7 native path/ancestor guards.
use super::windows::{Directory, EntryPin, MetadataSource, ReadSource};
use crate::{protocol::ErrorCode, resources::ResourceKind};
use std::{path::Path, sync::Arc};

type Result<T> = std::result::Result<T, ErrorCode>;
pub(crate) struct HeldResource(Kind);
enum Kind {
    File {
        parent: Directory,
        source: ReadSource,
        name: String,
    },
    Directory(Arc<DirectorySnapshot>),
    Child {
        parent: Arc<DirectorySnapshot>,
        source: MetadataSource,
    },
}
struct DirectorySnapshot {
    root: Directory,
    name: String,
    entries: Vec<EntryPin>,
}
impl HeldResource {
    pub fn select(
        path: &Path,
        kind: ResourceKind,
        protected: &Path,
        check: &impl Fn() -> Result<()>,
    ) -> Result<Self> {
        crate::data::safe_path(path).map_err(|_| ErrorCode::ScopeDenied)?;
        let name = path
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or(ErrorCode::ScopeDenied)?
            .to_owned();
        super::windows::component(&name)?;
        match kind {
            ResourceKind::File => {
                let parent = Directory::open(
                    path.parent().ok_or(ErrorCode::ScopeDenied)?,
                    Some(protected),
                )?;
                let source = parent.read_source(&name, check)?;
                Ok(Self(Kind::File {
                    parent,
                    source,
                    name,
                }))
            }
            ResourceKind::Directory => {
                let root = Directory::open(path, Some(protected))?;
                if root.related_to(protected)? {
                    return Err(ErrorCode::ScopeDenied);
                }
                let entries = root.resource_entries(check)?;
                Ok(Self(Kind::Directory(Arc::new(DirectorySnapshot {
                    root,
                    name,
                    entries,
                }))))
            }
        }
    }
    pub fn name(&self) -> &str {
        match &self.0 {
            Kind::File { name, .. } => name,
            Kind::Directory(root) => &root.name,
            Kind::Child { source, .. } => &source.pin.name,
        }
    }
    pub fn kind(&self) -> ResourceKind {
        match &self.0 {
            Kind::Directory(_) => ResourceKind::Directory,
            Kind::Child { source, .. } if source.pin.directory => ResourceKind::Directory,
            _ => ResourceKind::File,
        }
    }
    pub fn size(&self) -> Option<u32> {
        match &self.0 {
            Kind::File { source, .. } => Some(source.size() as u32),
            Kind::Child { source, .. } if !source.pin.directory => Some(source.pin.size as u32),
            _ => None,
        }
    }
    pub fn observed_bytes(&self) -> u64 {
        match &self.0 {
            Kind::File { source, .. } => source.size(),
            Kind::Directory(root) => root.entries.iter().map(|e| e.size).sum(),
            Kind::Child { .. } => 0,
        }
    }
    pub fn observation(&self) -> (usize, u64) {
        match &self.0 {
            Kind::File { source, .. } => (std::ptr::from_ref(source) as usize, source.size()),
            Kind::Directory(root) | Kind::Child { parent: root, .. } => (
                Arc::as_ptr(root) as usize,
                root.entries.iter().map(|e| e.size).sum(),
            ),
        }
    }
    // Shared directory ancestors stay charged while any child retains them.
    // The tuple is an accounting identity, never an OS handle or authority.
    pub fn handles(&self) -> (usize, usize, usize) {
        match &self.0 {
            Kind::File { parent, source, .. } => (
                std::ptr::from_ref(source) as usize,
                parent.handle_count() + 1,
                0,
            ),
            Kind::Directory(root) => (Arc::as_ptr(root) as usize, root.root.handle_count(), 0),
            Kind::Child { parent, .. } => {
                (Arc::as_ptr(parent) as usize, parent.root.handle_count(), 1)
            }
        }
    }
    pub fn validate(&self, check: &impl Fn() -> Result<()>) -> Result<()> {
        check()?;
        match &self.0 {
            Kind::File { parent, source, .. } => {
                parent.validate()?;
                source.validate(check)
            }
            Kind::Directory(root) => root.validate(check),
            Kind::Child { parent, source } => {
                parent.validate(check)?;
                source.validate()
            }
        }
    }
    pub fn read(&self, offset: u32, max: u32, check: &impl Fn() -> Result<()>) -> Result<Vec<u8>> {
        match &self.0 {
            Kind::File { parent, source, .. } => {
                parent.validate()?;
                let bytes = source.chunk(offset, max, check)?;
                parent.validate()?;
                check()?;
                Ok(bytes)
            }
            _ => Err(ErrorCode::ScopeDenied),
        }
    }
    pub fn page(
        &self,
        index: usize,
        limit: usize,
        check: &impl Fn() -> Result<()>,
    ) -> Result<(Vec<Self>, Option<usize>)> {
        let Kind::Directory(root) = &self.0 else {
            return Err(ErrorCode::ScopeDenied);
        };
        root.validate(check)?;
        if limit == 0 || limit > 64 || index > root.entries.len() {
            return Err(ErrorCode::InputInvalid);
        }
        let end = index.saturating_add(limit).min(root.entries.len());
        let mut children = Vec::new();
        for pin in &root.entries[index..end] {
            check()?;
            let source = root.root.metadata_source(&pin.name)?;
            if source.pin != *pin {
                return Err(ErrorCode::RevisionConflict);
            }
            children.push(Self(Kind::Child {
                parent: root.clone(),
                source,
            }));
        }
        root.validate(check)?;
        Ok((children, (end < root.entries.len()).then_some(end)))
    }
}
impl DirectorySnapshot {
    fn validate(&self, check: &impl Fn() -> Result<()>) -> Result<()> {
        self.root.validate()?;
        if self.root.resource_entries(check)? != self.entries {
            return Err(ErrorCode::RevisionConflict);
        }
        check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    struct Fixture {
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "flowtools-resource-native-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir(&root).unwrap();
            std::fs::create_dir(root.join("profile")).unwrap();
            std::fs::create_dir(root.join("files")).unwrap();
            Self { root }
        }
        fn profile(&self) -> PathBuf {
            self.root.join("profile")
        }
        fn files(&self) -> PathBuf {
            self.root.join("files")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            assert_eq!(self.root.parent(), Some(std::env::temp_dir().as_path()));
            assert!(self
                .root
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("flowtools-resource-native-"));
            std::fs::remove_dir_all(&self.root).unwrap();
        }
    }
    #[test]
    fn actual_readonly_file_checks_chunks_offsets_and_holds_parent_without_delete_access() {
        let fixture = Fixture::new();
        let path = fixture.files().join("canary.txt");
        let bytes = vec![b'x'; 131_101];
        std::fs::write(&path, &bytes).unwrap();
        let held = HeldResource::select(&path, ResourceKind::File, &fixture.profile(), &|| Ok(()))
            .unwrap();
        assert_eq!(
            held.read(65530, 65536, &|| Ok(())).unwrap(),
            bytes[65530..131066]
        );
        assert_eq!(
            held.read(bytes.len() as u32, 1, &|| Ok(())).unwrap(),
            Vec::<u8>::new()
        );
        assert!(matches!(
            held.read(0, 65537, &|| Ok(())),
            Err(ErrorCode::InputInvalid)
        ));
        assert!(std::fs::rename(fixture.files(), fixture.root.join("moved")).is_err());
        assert!(std::fs::write(&path, b"substituted").is_err());
        drop(held);
        std::fs::rename(fixture.files(), fixture.root.join("moved")).unwrap();
        std::fs::write(
            fixture.root.join("moved/canary.txt"),
            b"positive mutation control",
        )
        .unwrap();
    }
    #[test]
    fn directory_pages_are_stable_and_do_not_authorize_child_reads() {
        let fixture = Fixture::new();
        std::fs::write(fixture.files().join("first.txt"), b"NA1_PRIVATE_CANARY").unwrap();
        let held = HeldResource::select(
            &fixture.files(),
            ResourceKind::Directory,
            &fixture.profile(),
            &|| Ok(()),
        )
        .unwrap();
        let (page, next) = held.page(0, 1, &|| Ok(())).unwrap();
        assert_eq!(page.len(), 1);
        assert!(next.is_none());
        assert!(matches!(
            page[0].read(0, 1, &|| Ok(())),
            Err(ErrorCode::ScopeDenied)
        ));
        std::fs::write(fixture.files().join("added.txt"), b"changed").unwrap();
        assert!(matches!(
            held.page(0, 1, &|| Ok(())),
            Err(ErrorCode::RevisionConflict)
        ));
        assert!(HeldResource::select(
            &fixture.root,
            ResourceKind::Directory,
            &fixture.profile(),
            &|| Ok(())
        )
        .is_err());
        assert!(HeldResource::select(
            &fixture.profile(),
            ResourceKind::Directory,
            &fixture.profile(),
            &|| Ok(())
        )
        .is_err());
    }
    #[test]
    fn actual_ntfs_junction_hardlink_and_oversized_files_refuse_without_target_content_delivery() {
        let fixture = Fixture::new();
        let original = fixture.files().join("original.txt");
        std::fs::write(&original, b"outside-canary").unwrap();
        let alias = fixture.files().join("alias.txt");
        std::fs::hard_link(&original, &alias).unwrap();
        assert!(
            HeldResource::select(&alias, ResourceKind::File, &fixture.profile(), &|| Ok(()))
                .is_err()
        );
        std::fs::remove_file(&alias).unwrap();
        let link = fixture.root.join("junction");
        let created = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&link)
            .arg(fixture.files())
            .output()
            .unwrap();
        assert!(created.status.success());
        assert!(HeldResource::select(
            &link.join("original.txt"),
            ResourceKind::File,
            &fixture.profile(),
            &|| Ok(())
        )
        .is_err());
        assert!(
            HeldResource::select(&link, ResourceKind::Directory, &fixture.profile(), &|| Ok(
                ()
            ))
            .is_err()
        );
        std::fs::remove_dir(&link).unwrap();
        let oversized = fixture.files().join("oversized.bin");
        std::fs::File::create(&oversized)
            .unwrap()
            .set_len(crate::resources::MAX_FILE + 1)
            .unwrap();
        assert!(matches!(
            HeldResource::select(&oversized, ResourceKind::File, &fixture.profile(), &|| Ok(
                ()
            )),
            Err(ErrorCode::BudgetExceeded)
        ));
        assert_eq!(std::fs::read(&original).unwrap(), b"outside-canary");
    }
    #[test]
    fn existing_writable_mapping_is_refused_before_resource_admission_and_normal_reads_resume_after_drain(
    ) {
        use std::{
            fs::OpenOptions,
            os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
        };
        use windows_sys::Win32::System::Memory::{
            CreateFileMappingW, FlushViewOfFile, MapViewOfFile, UnmapViewOfFile, FILE_MAP_WRITE,
            PAGE_READWRITE,
        };
        let fixture = Fixture::new();
        let path = fixture.files().join("mapped.bin");
        std::fs::write(&path, b"AAAA").unwrap();
        let writer = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let mapping = unsafe {
            CreateFileMappingW(
                writer.as_raw_handle(),
                std::ptr::null(),
                PAGE_READWRITE,
                0,
                0,
                std::ptr::null(),
            )
        };
        assert!(!mapping.is_null());
        let mapping = unsafe { OwnedHandle::from_raw_handle(mapping) };
        let view = unsafe { MapViewOfFile(mapping.as_raw_handle(), FILE_MAP_WRITE, 0, 0, 4) };
        assert!(!view.Value.is_null());
        drop(writer);
        let refused =
            HeldResource::select(&path, ResourceKind::File, &fixture.profile(), &|| Ok(()));
        assert!(matches!(refused, Err(ErrorCode::RuntimeBusy)));
        unsafe {
            std::ptr::write_volatile(view.Value.cast::<u8>(), b'B');
            assert_ne!(FlushViewOfFile(view.Value, 4), 0);
        }
        unsafe {
            assert_ne!(UnmapViewOfFile(view), 0);
        }
        drop(mapping);
        assert_eq!(std::fs::read(&path).unwrap(), b"BAAA");
        let source =
            HeldResource::select(&path, ResourceKind::File, &fixture.profile(), &|| Ok(()))
                .unwrap();
        assert_eq!(source.read(0, 4, &|| Ok(())).unwrap(), b"BAAA");
    }
    #[test]
    fn selection_refuses_ads_devices_unc_and_cancel_before_read() {
        let fixture = Fixture::new();
        std::fs::write(fixture.files().join("safe.txt"), b"private").unwrap();
        for path in [
            fixture.files().join("safe.txt:stream"),
            fixture.files().join("NUL"),
            PathBuf::from(r"\\localhost\share\file"),
            fixture.profile().join("runtime.sqlite"),
        ] {
            assert!(HeldResource::select(
                &path,
                ResourceKind::File,
                &fixture.profile(),
                &|| Ok(())
            )
            .is_err());
        }
        assert!(matches!(
            HeldResource::select(
                &fixture.files().join("safe.txt"),
                ResourceKind::File,
                &fixture.profile(),
                &|| Err(ErrorCode::Aborted)
            ),
            Err(ErrorCode::Aborted)
        ));
    }
}
