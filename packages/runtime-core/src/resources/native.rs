//! Opaque Runtime IO state. No OS path/handle is returned to a plugin.
use crate::protocol::ErrorCode;
use crate::{file_workflows::resource_source::HeldResource, resources::*};
use std::{collections::HashMap, path::PathBuf};
use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

pub(crate) struct Entry {
    pub slot: String,
    pub info: ResourceInfo,
    pub source: Option<Arc<HeldResource>>,
}
pub(crate) struct Owner {
    pub public: ResourceOwner,
    pub epoch: u32,
    pub entries: Vec<Entry>,
}
pub(crate) struct Run {
    pub entries: Vec<Entry>,
    pub cursors: HashMap<String, (String, usize)>,
    pub retired: HashMap<String, (String, ResourceRight)>,
}
pub(crate) fn entry_slots<'a>(entries: impl Iterator<Item = &'a Entry>) -> usize {
    let mut shared = HashMap::new();
    let mut slots = 0;
    for entry in entries {
        slots += 1; // ResourceRef
        if let Some(source) = &entry.source {
            let (group, handles, own) = source.handles();
            shared.insert(group, handles);
            slots += own;
        }
    }
    slots + shared.values().sum::<usize>()
}
impl Run {
    pub fn slots(&self) -> usize {
        entry_slots(self.entries.iter()) + self.cursors.len()
    }
}
pub(crate) struct Active {
    pub reserved: usize,
    pub owner: Option<String>,
    pub run: Option<String>,
    pub slot: String,
    pub right: ResourceRight,
    pub cancellation: Arc<Cancellation>,
    pub operation: Option<ResourceOperation>,
}
#[derive(Default)]
pub(crate) struct State {
    pub owners: HashMap<String, Owner>,
    pub runs: HashMap<String, Run>,
    pub active: HashMap<String, Active>,
    pub deferred: HashMap<String, (Result<serde_json::Value, ErrorCode>, Option<JobState>)>,
}
impl State {
    pub fn slots(&self) -> usize {
        self.owners
            .values()
            .map(|o| entry_slots(o.entries.iter()))
            .sum::<usize>()
            + self.runs.values().map(Run::slots).sum::<usize>()
            + self.active.values().map(|a| a.reserved).sum::<usize>()
    }
    pub fn observed(&self) -> u64 {
        self.owners
            .values()
            .flat_map(|o| &o.entries)
            .chain(self.runs.values().flat_map(|r| &r.entries))
            .filter_map(|e| e.source.as_ref())
            .map(|s| s.observation())
            .collect::<HashMap<_, _>>()
            .values()
            .sum()
    }
    pub fn cancel_run(&self, run: &str) -> Result<(), ErrorCode> {
        for call in self
            .active
            .values()
            .filter(|a| a.run.as_deref() == Some(run))
        {
            call.cancellation.cancel()?;
        }
        Ok(())
    }
}

/// Move-only IO work prepared by Core after authentication. Paths never leave T0.
pub struct ResourceIo {
    pub id: String,
    pub cancellation: Arc<Cancellation>,
    pub(crate) action: IoAction,
}
pub(crate) enum IoAction {
    Released,
    Select {
        path: PathBuf,
        kind: ResourceKind,
        profile: PathBuf,
    },
    Validate(Arc<HeldResource>),
    Read(Arc<HeldResource>, u32, u32),
    List(Arc<HeldResource>, usize, usize),
}
pub struct ResourceIoResult(pub(crate) Result<IoValue, ErrorCode>);
impl ResourceIoResult {
    pub fn interrupted() -> Self {
        Self(Err(ErrorCode::ExecutionInterrupted))
    }
}
pub(crate) enum IoValue {
    Selected(Arc<HeldResource>),
    Checked,
    Bytes(Vec<u8>),
    Children(Vec<HeldResource>, Option<usize>),
}
impl ResourceIo {
    #[cfg(test)]
    pub fn perform(self) -> ResourceIoResult {
        self.perform_authorized(|| Ok(()))
    }
    pub fn perform_authorized(
        self,
        authority: impl Fn() -> Result<(), ErrorCode>,
    ) -> ResourceIoResult {
        ResourceIoResult((|| {
            let _thread = self.cancellation.enter()?;
            let check = || {
                self.cancellation.check()?;
                authority()
            };
            check()?;
            match self.action {
                IoAction::Released => Ok(IoValue::Checked),
                IoAction::Select {
                    path,
                    kind,
                    profile,
                } => HeldResource::select(&path, kind, &profile, &check)
                    .map(|s| IoValue::Selected(Arc::new(s))),
                IoAction::Validate(source) => {
                    source.validate(&check)?;
                    Ok(IoValue::Checked)
                }
                IoAction::Read(source, offset, max) => {
                    source.read(offset, max, &check).map(IoValue::Bytes)
                }
                IoAction::List(source, index, limit) => source
                    .page(index, limit, &check)
                    .map(|(v, n)| IoValue::Children(v, n)),
            }
        })())
    }
}

#[derive(Default)]
pub struct Cancellation {
    cancelled: AtomicBool,
    thread: Mutex<Option<OwnedHandle>>,
}
pub struct IoThread(Arc<Cancellation>);
impl Cancellation {
    pub fn check(&self) -> Result<(), ErrorCode> {
        if self.cancelled.load(Ordering::Acquire) {
            Err(ErrorCode::Aborted)
        } else {
            Ok(())
        }
    }
    pub fn cancel(&self) -> Result<(), ErrorCode> {
        use windows_sys::Win32::{
            Foundation::{GetLastError, ERROR_NOT_FOUND},
            System::IO::CancelSynchronousIo,
        };
        self.cancelled.store(true, Ordering::Release);
        let held = self
            .thread
            .lock()
            .map_err(|_| ErrorCode::ExecutionInterrupted)?;
        if let Some(thread) = held.as_ref() {
            if unsafe { CancelSynchronousIo(thread.as_raw_handle()) } == 0
                && unsafe { GetLastError() } != ERROR_NOT_FOUND
            {
                return Err(ErrorCode::ExecutionInterrupted);
            }
        }
        Ok(())
    }
    pub fn enter(self: &Arc<Self>) -> Result<IoThread, ErrorCode> {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThreadId, OpenThread, THREAD_TERMINATE,
        };
        self.check()?;
        let handle = unsafe { OpenThread(THREAD_TERMINATE, 0, GetCurrentThreadId()) };
        if handle.is_null() {
            return Err(ErrorCode::ExecutionInterrupted);
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
        let mut slot = self
            .thread
            .lock()
            .map_err(|_| ErrorCode::ExecutionInterrupted)?;
        if slot.is_some() {
            return Err(ErrorCode::RuntimeBusy);
        }
        *slot = Some(handle);
        drop(slot);
        let guard = IoThread(self.clone());
        self.check()?;
        Ok(guard)
    }
}
impl Drop for IoThread {
    fn drop(&mut self) {
        if let Ok(mut thread) = self.0.thread.lock() {
            thread.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_cancel_is_sticky_and_never_lends_a_reusable_thread_handle() {
        let cancellation = Arc::new(Cancellation::default());
        let entered = cancellation.enter().unwrap();
        // Actual CancelSynchronousIo observes no pending IO, not a mock success.
        cancellation.cancel().unwrap();
        assert_eq!(cancellation.check(), Err(ErrorCode::Aborted));
        drop(entered);
        assert!(cancellation.thread.lock().unwrap().is_none());
        assert!(matches!(cancellation.enter(), Err(ErrorCode::Aborted)));
    }
    #[test]
    fn actual_pending_synchronous_read_cancels_and_positive_control_reads_without_closing_writer() {
        use windows_sys::Win32::{
            Foundation::{GetLastError, ERROR_OPERATION_ABORTED},
            Storage::FileSystem::{ReadFile, WriteFile},
            System::{
                Pipes::CreatePipe,
                Threading::{
                    GetCurrentThreadId, GetThreadIOPendingFlag, OpenThread,
                    THREAD_QUERY_INFORMATION,
                },
            },
        };
        for cancel in [false, true] {
            let mut reader = std::ptr::null_mut();
            let mut writer = std::ptr::null_mut();
            assert_ne!(
                unsafe { CreatePipe(&mut reader, &mut writer, std::ptr::null(), 0) },
                0
            );
            let reader = unsafe { OwnedHandle::from_raw_handle(reader) };
            let writer = unsafe { OwnedHandle::from_raw_handle(writer) };
            let cancellation = Arc::new(Cancellation::default());
            let worker_cancel = cancellation.clone();
            let (send, receive) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                let _guard = worker_cancel.enter().unwrap();
                send.send(unsafe { GetCurrentThreadId() }).unwrap();
                let mut byte = 0u8;
                let mut count = 0;
                let success = unsafe {
                    ReadFile(
                        reader.as_raw_handle(),
                        (&mut byte as *mut u8).cast(),
                        1,
                        &mut count,
                        std::ptr::null_mut(),
                    )
                };
                (
                    success,
                    count,
                    byte,
                    if success == 0 {
                        unsafe { GetLastError() }
                    } else {
                        0
                    },
                )
            });
            let id = receive
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            let thread = unsafe { OpenThread(THREAD_QUERY_INFORMATION, 0, id) };
            assert!(!thread.is_null());
            let thread = unsafe { OwnedHandle::from_raw_handle(thread) };
            let started = std::time::Instant::now();
            loop {
                let mut pending = 0;
                assert_ne!(
                    unsafe { GetThreadIOPendingFlag(thread.as_raw_handle(), &mut pending) },
                    0
                );
                if pending != 0 {
                    break;
                }
                assert!(started.elapsed() < std::time::Duration::from_secs(5));
                std::thread::yield_now();
            }
            if cancel {
                cancellation.cancel().unwrap();
            } else {
                let mut written = 0;
                assert_ne!(
                    unsafe {
                        WriteFile(
                            writer.as_raw_handle(),
                            b"x".as_ptr().cast(),
                            1,
                            &mut written,
                            std::ptr::null_mut(),
                        )
                    },
                    0
                );
                assert_eq!(written, 1);
            }
            let result = worker.join().unwrap();
            if cancel {
                assert_eq!(result.0, 0);
                assert_eq!(result.3, ERROR_OPERATION_ABORTED);
            } else {
                assert_ne!(result.0, 0);
                assert_eq!((result.1, result.2), (1, b'x'));
            }
            assert!(cancellation.thread.lock().unwrap().is_none());
            drop(writer);
        }
    }
}
