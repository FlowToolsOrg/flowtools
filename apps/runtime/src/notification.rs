//! Fixed application identity and plain bounded text; no XML, paths or actions.
use flowtools_runtime_core::notifications::NotificationStatus;

pub fn send(title: &str, message: &str) -> NotificationStatus {
    #[cfg(windows)]
    {
        // Match Desktop's installed AppUserModelID. Never impersonate PowerShell
        // or register another application to make a notification appear to work.
        match tauri_winrt_notification::Toast::new("com.hmsuiji.desktop")
            .title(title)
            .text1(message)
            .sound(None)
            .show()
        {
            Ok(()) => NotificationStatus::Requested,
            Err(_) => NotificationStatus::Failed,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (title, message);
        NotificationStatus::Unsupported
    }
}
