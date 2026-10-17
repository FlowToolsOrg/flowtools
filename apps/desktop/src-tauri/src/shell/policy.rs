pub(super) fn hide_on_close(label: &str, tray_ready: bool) -> bool {
    label == "main" && tray_ready
}

pub(super) fn autostart_allowed(debug: bool, identifier: &str) -> bool {
    !debug && identifier == "com.hmsuiji.desktop"
}

pub(super) fn start_hidden(
    allowed: bool,
    tray_ready: bool,
    args: impl IntoIterator<Item = impl AsRef<str>>,
) -> bool {
    allowed && tray_ready && args.into_iter().any(|arg| arg.as_ref() == "--background")
}

// An explicit desired value makes retries idempotent even when OS state changes
// outside this app. Re-read after writing; never report an unconfirmed success.
pub(super) fn set_autostart<E>(
    allowed: bool,
    desired: bool,
    write: impl FnOnce(bool) -> Result<(), E>,
    read: impl FnOnce() -> Result<bool, E>,
) -> Option<bool> {
    if !allowed {
        return None;
    }
    write(desired).ok()?;
    let actual = read().ok()?;
    (actual == desired).then_some(actual)
}

#[derive(Clone, Copy)]
pub(super) struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

// Keep enough of the titlebar reachable to drag/close the window. All arithmetic
// is widened because monitors may have negative positions or extreme dimensions.
pub(super) fn restore_position(window: Rect, monitors: &[Rect]) -> Option<(i32, i32)> {
    let x = i64::from(window.x);
    let y = i64::from(window.y);
    let title_width = i64::from(window.width.min(128));
    let reachable = monitors.iter().any(|monitor| {
        x >= i64::from(monitor.x)
            && x + title_width <= i64::from(monitor.x) + i64::from(monitor.width)
            && y >= i64::from(monitor.y)
            && y + 40 <= i64::from(monitor.y) + i64::from(monitor.height)
    });
    if reachable {
        return None;
    }
    let monitor = monitors.first()?;
    let x = i64::from(monitor.x) + i64::from(monitor.width.saturating_sub(window.width)) / 2;
    let y = i64::from(monitor.y) + i64::from(monitor.height.saturating_sub(window.height)) / 2;
    Some((
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_denial_has_no_os_effect_and_failed_writes_never_report_success() {
        use std::cell::Cell;
        let effects = Cell::new(0);
        let write = |_| {
            effects.set(effects.get() + 1);
            Ok::<_, ()>(())
        };
        let read = || {
            effects.set(effects.get() + 1);
            Ok::<_, ()>(true)
        };
        assert_eq!(set_autostart(false, true, write, read), None);
        assert_eq!(effects.get(), 0);
        assert_eq!(
            set_autostart(
                true,
                true,
                |_| Err(()),
                || panic!("read after failed write")
            ),
            None
        );
        assert_eq!(
            set_autostart(true, true, |_| Ok::<_, ()>(()), || Err(())),
            None
        );
        assert_eq!(
            set_autostart(true, true, |_| Ok::<_, ()>(()), || Ok(false)),
            None
        );
    }

    #[test]
    fn explicit_autostart_requests_are_idempotent_and_confirm_the_os_state() {
        use std::cell::Cell;
        let state = Cell::new(false);
        for desired in [true, true, false, false] {
            assert_eq!(
                set_autostart(
                    true,
                    desired,
                    |value| {
                        state.set(value);
                        Ok::<_, ()>(())
                    },
                    || Ok(state.get())
                ),
                Some(desired)
            );
        }
    }

    #[test]
    fn close_only_hides_the_main_window_with_a_working_tray() {
        assert!(hide_on_close("main", true));
        assert!(!hide_on_close("main", false));
        assert!(!hide_on_close("external", true));
    }

    #[test]
    fn autostart_never_changes_debug_or_fixture_profiles() {
        for identifier in [
            "com.hmsuiji.desktop",
            "com.flowtools.manual-validation-20261004-r2",
            "com.flowtools.g2-validation-20261004",
            "untrusted",
        ] {
            assert!(!autostart_allowed(true, identifier));
            assert_eq!(
                autostart_allowed(false, identifier),
                identifier == "com.hmsuiji.desktop"
            );
        }
    }

    #[test]
    fn background_launch_cannot_strand_a_window_without_a_tray() {
        assert!(start_hidden(true, true, ["app", "--background"]));
        assert!(!start_hidden(true, false, ["app", "--background"]));
        assert!(!start_hidden(false, true, ["app", "--background"]));
        assert!(!start_hidden(true, true, ["app", "--background=1"]));
        assert!(!start_hidden(true, true, ["app", "plugin-command"]));
    }

    #[test]
    fn restore_recovers_removed_monitors_and_preserves_reachable_negative_coordinates() {
        let primary = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
        };
        let left = Rect {
            x: -1600,
            y: -100,
            width: 1600,
            height: 900,
        };
        let on_left = Rect {
            x: -1400,
            y: -50,
            width: 800,
            height: 600,
        };
        assert_eq!(restore_position(on_left, &[primary, left]), None);
        assert_eq!(restore_position(on_left, &[primary]), Some((560, 220)));
        let below = Rect {
            x: 100,
            y: 1040,
            width: 800,
            height: 600,
        };
        assert_eq!(restore_position(below, &[primary]), Some((560, 220)));
        let above = Rect {
            x: 100,
            y: -40,
            width: 800,
            height: 600,
        };
        assert_eq!(restore_position(above, &[primary]), Some((560, 220)));
        assert_eq!(restore_position(below, &[]), None);
    }

    #[test]
    fn oversized_and_extreme_positions_keep_the_titlebar_accessible_without_overflow() {
        let primary = Rect {
            x: 0,
            y: 40,
            width: 1280,
            height: 680,
        };
        let huge = Rect {
            x: i32::MAX,
            y: i32::MIN,
            width: u32::MAX,
            height: u32::MAX,
        };
        assert_eq!(restore_position(huge, &[primary]), Some((0, 40)));
        let narrow_sliver = Rect {
            x: 1279,
            y: 50,
            width: 800,
            height: 600,
        };
        assert_eq!(restore_position(narrow_sliver, &[primary]), Some((240, 80)));
    }
}
