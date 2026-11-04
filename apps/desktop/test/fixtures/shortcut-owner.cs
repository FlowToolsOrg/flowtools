using System;
using System.Runtime.InteropServices;
using System.Windows.Forms;

// Test-only user-driven fixture. It never controls another application's UI.
// https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-registerhotkey
sealed class ShortcutOwner : Form {
  [DllImport("user32.dll", SetLastError = true)]
  static extern bool RegisterHotKey(IntPtr window, int id, uint modifiers, uint key);
  [DllImport("user32.dll", SetLastError = true)]
  static extern bool UnregisterHotKey(IntPtr window, int id);
  readonly Label status = new Label { AutoSize = false, Dock = DockStyle.Fill };
  bool owned;
  int received;

  ShortcutOwner() {
    Text = "FlowTools shortcut owner - test data only";
    Width = 650;
    Height = 260;
    var release = new Button { Text = "Release test shortcut", Dock = DockStyle.Bottom, Height = 40 };
    release.Click += delegate {
      if (owned && UnregisterHotKey(Handle, 1)) owned = false;
      UpdateStatus();
    };
    Controls.Add(status);
    Controls.Add(release);
    Shown += delegate {
      // Ctrl + Shift + Space; no repeat and no persistent system setting.
      owned = RegisterHotKey(Handle, 1, 0x4000 | 0x0002 | 0x0004, 0x20);
      UpdateStatus();
    };
    FormClosed += delegate { if (owned) UnregisterHotKey(Handle, 1); };
  }

  void UpdateStatus() {
    status.Text = "Test-only Ctrl+Shift+Space owner\r\nRegistered: " + owned
      + "\r\nReceived: " + received
      + "\r\nContains no user data. Release or close to unregister.";
  }

  protected override void WndProc(ref Message message) {
    if (message.Msg == 0x0312 && message.WParam.ToInt32() == 1) {
      received++;
      UpdateStatus();
    }
    base.WndProc(ref message);
  }

  [STAThread]
  static void Main() { Application.Run(new ShortcutOwner()); }
}
