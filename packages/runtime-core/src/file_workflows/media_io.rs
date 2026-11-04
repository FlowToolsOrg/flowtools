//! Trusted local selection for fixed native media jobs. Definitions remain DPAPI
//! private; plugins receive only an opaque plan ID. No caller argv/executable.
use super::*;
use crate::{data::DataStore, private_jobs::PrivateJobs};
use std::collections::HashSet;
const KEY: &str = "user-media-plans-v1";
const LIMIT: usize = 65536;
pub const MAX_INPUT_BYTES: u64 = 1048576;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, specta::Type, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum UserMediaAction {
    Inspect,
    Audio,
    Webm,
}
impl UserMediaAction {
    pub fn plugin(self) -> &'static str {
        if self == Self::Inspect {
            "plugin-media-inspector"
        } else {
            "plugin-media-converter"
        }
    }
    pub fn operation(self) -> &'static str {
        match self {
            Self::Inspect => "inspect-v1",
            Self::Audio => "extract-audio-wav-v1",
            Self::Webm => "transcode-webm-v1",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateMediaPlan {
    pub request_key: String,
    pub input_path: String,
    pub output_path: Option<String>,
    pub action: UserMediaAction,
}
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaPlanPreview {
    pub id: String,
    pub digest: String,
    pub action: UserMediaAction,
    pub input_name: String,
    pub input_bytes: u32,
    pub input_sha256: String,
    pub output_name: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartMediaPlan {
    pub id: String,
    pub digest: String,
    pub target: crate::protocol::GrantTarget,
}
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserMediaProgress {
    pub run_id: String,
    pub stage: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Definition {
    input_directory: String,
    input_root: RootPin,
    input_name: String,
    input_pin: FilePin,
    output_directory: Option<String>,
    output_root: Option<RootPin>,
    output_name: Option<String>,
    action: UserMediaAction,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Entry {
    id: String,
    digest: String,
    input_id: String,
    run_id: Option<String>,
    publication: Option<Publication>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Publication {
    sha256: String,
    bytes: u32,
    stage: String,
    complete: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    format_version: u16,
    profile_digest: String,
    generation: Option<String>,
    entries: Vec<Entry>,
    #[serde(skip)]
    predecessor: Option<String>,
}
fn valid_id(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok_and(|u| u.to_string() == s)
}
fn valid_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn scope(store: &DataStore) -> Result<(String, Option<String>), ErrorCode> {
    let profile = std::fs::canonicalize(store.database_path().ok_or(ErrorCode::StorageFailed)?)
        .map_err(|_| ErrorCode::StorageFailed)?;
    let generation = store::bounded_text(&store.connection, "recovery-generation", 128)?;
    if generation.as_deref().is_some_and(|g| g.is_empty()) {
        return Err(ErrorCode::StoreCorrupt);
    }
    Ok((
        digest(&serde_json::json!(profile
            .to_str()
            .ok_or(ErrorCode::StorageFailed)?)),
        generation,
    ))
}
fn load(store: &DataStore, restore: bool) -> Result<State, ErrorCode> {
    let raw = store::bounded_text(&store.connection, KEY, LIMIT)?;
    let Some(raw) = raw else {
        let (profile_digest, generation) = if store.database_path().is_some() {
            scope(store)?
        } else {
            (digest(&serde_json::json!("memory-media")), None)
        };
        return Ok(State {
            format_version: 1,
            profile_digest,
            generation,
            entries: Vec::new(),
            predecessor: None,
        });
    };
    let mut s: State = serde_json::from_value(
        crate::packages::strict_json(raw.as_bytes()).map_err(|_| ErrorCode::StoreCorrupt)?,
    )
    .map_err(|_| ErrorCode::StoreCorrupt)?;
    s.predecessor = Some(raw);
    let mut ids = HashSet::new();
    let mut inputs = HashSet::new();
    if s.format_version != 1
        || !valid_digest(&s.profile_digest)
        || s.entries.len() > 64
        || s.generation
            .as_deref()
            .is_some_and(|g| g.is_empty() || g.len() > 128)
        || s.entries.iter().any(|e| {
            !valid_id(&e.id)
                || !valid_id(&e.input_id)
                || !valid_digest(&e.digest)
                || !ids.insert(&e.id)
                || !inputs.insert(&e.input_id)
                || e.run_id.as_deref().is_some_and(|r| !valid_id(r))
                || e.publication.as_ref().is_some_and(|p| {
                    !valid_digest(&p.sha256)
                        || !valid_id(&p.stage)
                        || p.bytes == 0
                        || u64::from(p.bytes) > MAX_INPUT_BYTES
                })
        })
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    if !restore && scope(store)? != (s.profile_digest.clone(), s.generation.clone()) {
        return Err(ErrorCode::RecoveryPending);
    }
    Ok(s)
}
fn save(store: &mut DataStore, s: &mut State) -> Result<(), ErrorCode> {
    let raw = serde_json::to_string(s).map_err(|_| ErrorCode::StorageFailed)?;
    if raw.len() > LIMIT {
        return Err(ErrorCode::BudgetExceeded);
    }
    if scope(store)? != (s.profile_digest.clone(), s.generation.clone()) {
        return Err(ErrorCode::RecoveryPending);
    }
    let tx = store
        .connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| ErrorCode::StorageFailed)?;
    if store::bounded_text(&tx, KEY, LIMIT)? != s.predecessor
        || store::bounded_text(&tx, "recovery-generation", 128)? != s.generation
    {
        return Err(ErrorCode::RevisionConflict);
    }
    let before = tx.total_changes();
    let rows=tx.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",rusqlite::params![KEY,raw]).map_err(|_|ErrorCode::StorageFailed)?;
    if rows != 1
        || tx.total_changes() != before + 1
        || store::bounded_text(&tx, KEY, LIMIT)?.as_deref() != Some(&raw)
        || store::bounded_text(&tx, "recovery-generation", 128)? != s.generation
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    tx.commit().map_err(|_| ErrorCode::StorageFailed)?;
    s.predecessor = Some(raw);
    Ok(())
}
fn private(store: &DataStore) -> Result<PrivateJobs, ErrorCode> {
    PrivateJobs::open(store.profile().ok_or(ErrorCode::StorageFailed)?)
}
fn definition(store: &DataStore, e: &Entry) -> Result<Definition, ErrorCode> {
    let value = private(store)?.read(&e.input_id, "input")?;
    if digest(&value) != e.digest {
        return Err(ErrorCode::StoreCorrupt);
    }
    let d: Definition = serde_json::from_value(value).map_err(|_| ErrorCode::StoreCorrupt)?;
    if d.input_pin.size == 0
        || d.input_pin.size > MAX_INPUT_BYTES
        || (d.action == UserMediaAction::Inspect)
            != (d.output_name.is_none() && d.output_directory.is_none() && d.output_root.is_none())
    {
        return Err(ErrorCode::StoreCorrupt);
    }
    Ok(d)
}
fn preview(e: &Entry, d: &Definition) -> MediaPlanPreview {
    MediaPlanPreview {
        id: e.id.clone(),
        digest: e.digest.clone(),
        action: d.action,
        input_name: d.input_name.clone(),
        input_bytes: d.input_pin.size as u32,
        input_sha256: d.input_pin.sha256.clone(),
        output_name: d.output_name.clone(),
    }
}
pub fn get(store: &DataStore, id: &str) -> Result<MediaPlanPreview, ErrorCode> {
    let s = load(store, false)?;
    let e = s
        .entries
        .iter()
        .find(|e| e.id == id)
        .ok_or(ErrorCode::InvalidRequest)?;
    Ok(preview(e, &definition(store, e)?))
}
pub(crate) fn bound_run(store: &DataStore, id: &str) -> Result<Option<String>, ErrorCode> {
    let s = load(store, false)?;
    Ok(s.entries
        .iter()
        .find(|e| e.id == id)
        .ok_or(ErrorCode::InvalidRequest)?
        .run_id
        .clone())
}
pub(crate) fn retained_input_ids(store: &DataStore) -> Result<HashSet<String>, ErrorCode> {
    Ok(load(store, true)?
        .entries
        .into_iter()
        .map(|e| e.input_id)
        .collect())
}
pub(crate) fn bind(store: &mut DataStore, id: &str, run: &str) -> Result<(), ErrorCode> {
    let mut s = load(store, false)?;
    let e = s
        .entries
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or(ErrorCode::InvalidRequest)?;
    if !valid_id(run) || e.run_id.as_deref().is_some_and(|old| old != run) {
        return Err(ErrorCode::RevisionConflict);
    }
    e.run_id = Some(run.into());
    save(store, &mut s)
}
#[cfg(windows)]
pub fn create(
    store: &mut DataStore,
    create: CreateMediaPlan,
) -> Result<MediaPlanPreview, ErrorCode> {
    if !valid_id(&create.request_key)
        || create.input_path.len() > 32768
        || create.output_path.as_ref().is_some_and(|p| p.len() > 32768)
    {
        return Err(ErrorCode::InputInvalid);
    }
    let mut s = load(store, false)?;
    let input = Path::new(&create.input_path);
    let parent = input.parent().ok_or(ErrorCode::InputInvalid)?;
    let name = input
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(ErrorCode::InputInvalid)?;
    let root = windows::Directory::open(parent, store.profile())?;
    let source = root.source(name)?.ok_or(ErrorCode::ScopeDenied)?;
    if source.pin.size == 0 || source.pin.size > MAX_INPUT_BYTES {
        return Err(ErrorCode::BudgetExceeded);
    }
    let output = if create.action == UserMediaAction::Inspect {
        if create.output_path.is_some() {
            return Err(ErrorCode::InputInvalid);
        }
        None
    } else {
        let path = Path::new(
            create
                .output_path
                .as_deref()
                .ok_or(ErrorCode::InputInvalid)?,
        );
        let dir = path.parent().ok_or(ErrorCode::InputInvalid)?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(ErrorCode::InputInvalid)?;
        let extension = if create.action == UserMediaAction::Audio {
            "wav"
        } else {
            "webm"
        };
        if !name
            .to_ascii_lowercase()
            .ends_with(&format!(".{extension}"))
        {
            return Err(ErrorCode::InputInvalid);
        }
        windows::component(name)?;
        let root = windows::Directory::open(dir, store.profile())?;
        if !root.absent(name)? {
            return Err(ErrorCode::RevisionConflict);
        }
        Some((
            dir.to_str().ok_or(ErrorCode::ScopeDenied)?.to_owned(),
            root,
            name.to_owned(),
        ))
    };
    let d = Definition {
        input_directory: parent.to_str().ok_or(ErrorCode::ScopeDenied)?.into(),
        input_root: root.pin.clone(),
        input_name: name.into(),
        input_pin: source.pin.clone(),
        output_directory: output.as_ref().map(|o| o.0.clone()),
        output_root: output.as_ref().map(|o| o.1.pin.clone()),
        output_name: output.map(|o| o.2),
        action: create.action,
    };
    let value = serde_json::to_value(&d).map_err(|_| ErrorCode::StorageFailed)?;
    let checksum = digest(&value);
    if let Some(e) = s.entries.iter().find(|e| e.id == create.request_key) {
        if e.digest != checksum {
            return Err(ErrorCode::IdempotencyConflict);
        }
        return Ok(preview(e, &d));
    }
    if s.entries.len() >= 64 {
        return Err(ErrorCode::BudgetExceeded);
    }
    let e = Entry {
        id: create.request_key,
        digest: checksum,
        input_id: uuid::Uuid::new_v4().to_string(),
        run_id: None,
        publication: None,
    };
    private(store)?.write(&e.input_id, "input", &value)?;
    let result = preview(&e, &d);
    s.entries.push(e);
    save(store, &mut s)?;
    Ok(result)
}
#[cfg(windows)]
pub(crate) fn capture(store: &DataStore, id: &str, run: &str) -> Result<Vec<u8>, ErrorCode> {
    let s = load(store, false)?;
    let e = s
        .entries
        .iter()
        .find(|e| e.id == id && e.run_id.as_deref() == Some(run))
        .ok_or(ErrorCode::ScopeDenied)?;
    let d = definition(store, e)?;
    let root = windows::Directory::open(Path::new(&d.input_directory), store.profile())?;
    if root.pin != d.input_root {
        return Err(ErrorCode::RevisionConflict);
    }
    let source = root
        .source(&d.input_name)?
        .ok_or(ErrorCode::RevisionConflict)?;
    if source.pin != d.input_pin {
        return Err(ErrorCode::RevisionConflict);
    }
    source.bytes(MAX_INPUT_BYTES)
}
#[cfg(windows)]
pub(crate) fn publish(
    store: &mut DataStore,
    id: &str,
    run: &str,
    bytes: &[u8],
    mut authorize: impl FnMut(&DataStore) -> Result<(), ErrorCode>,
) -> Result<(), ErrorCode> {
    use sha2::{Digest, Sha256};
    if bytes.is_empty() || bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(ErrorCode::BudgetExceeded);
    }
    let mut s = load(store, false)?;
    let n = s
        .entries
        .iter()
        .position(|e| e.id == id && e.run_id.as_deref() == Some(run))
        .ok_or(ErrorCode::ScopeDenied)?;
    let d = definition(store, &s.entries[n])?;
    let dir = d
        .output_directory
        .as_deref()
        .ok_or(ErrorCode::ScopeDenied)?;
    let name = d.output_name.as_deref().ok_or(ErrorCode::ScopeDenied)?;
    let root = windows::Directory::open(Path::new(dir), store.profile())?;
    if Some(&root.pin) != d.output_root.as_ref() {
        return Err(ErrorCode::RevisionConflict);
    }
    let checksum = format!("{:x}", Sha256::digest(bytes));
    if let Some(publication) = &s.entries[n].publication {
        if publication.sha256 != checksum || publication.bytes as usize != bytes.len() {
            return Err(ErrorCode::RevisionConflict);
        }
        // A repeated current-authorized collection can acknowledge the exact
        // completed bytes. Pending/unknown publication never automatically replays.
        if publication.complete
            && root
                .source(name)?
                .is_some_and(|f| f.pin.sha256 == checksum && f.pin.size == bytes.len() as u64)
        {
            return Ok(());
        }
        return Err(ErrorCode::AcceptanceUnknown);
    }
    if !root.absent(name)? {
        return Err(ErrorCode::RevisionConflict);
    }
    let stage = uuid::Uuid::new_v4().to_string();
    s.entries[n].publication = Some(Publication {
        sha256: checksum,
        bytes: bytes.len() as u32,
        stage: stage.clone(),
        complete: false,
    });
    save(store, &mut s)?;
    root.publish(&format!(".flowtools-{stage}.tmp"), name, bytes, || {
        if store::bounded_text(&store.connection, KEY, LIMIT)? != s.predecessor
            || scope(store)? != (s.profile_digest.clone(), s.generation.clone())
        {
            return Err(ErrorCode::RevisionConflict);
        }
        authorize(store)
    })?;
    s.entries[n].publication.as_mut().unwrap().complete = true;
    save(store, &mut s)
}

pub fn action(store: &DataStore, id: &str) -> Result<UserMediaAction, ErrorCode> {
    Ok(get(store, id)?.action)
}
pub fn input_handle(id: &str) -> String {
    format!("{id}-input")
}
pub fn output_handle(id: &str) -> String {
    format!("{id}-output")
}

#[cfg(all(test, windows))]
mod tests;
