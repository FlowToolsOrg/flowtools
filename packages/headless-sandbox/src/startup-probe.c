/* Native startup diagnostic: no CRT, Rust, QuickJS or Winsock. Fixture only. */
#include <windows.h>
void main(void) {
    const char result[] = "{\"value\":42}";
    DWORD written = 0;
    WriteFile(GetStdHandle(STD_OUTPUT_HANDLE), result, sizeof(result) - 1, &written, NULL);
    ExitProcess(written == sizeof(result) - 1 ? 0 : 1);
}
