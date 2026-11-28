#!/usr/bin/env bash
# Builds VIctoryTry.exe, a double-click launcher for try.ps1, on any OS with
# Zig (https://ziglang.org, or `pip install ziglang`), which cross-compiles C
# for Windows. The exe is a release download only: never commit it.
#
#   scripts/try/build-exe.sh [out-dir]        (default: target/try)
#
# The exe holds the try.ps1 it was built with. It writes it to
# %LOCALAPPDATA%\VIctoryTry\try.ps1, runs it with Windows PowerShell in its
# console and waits for Enter before the window closes. The C source is
# below, in full.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="${1:-$root/target/try}"
mkdir -p "$out"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if command -v zig >/dev/null 2>&1; then
    zig=(zig)
elif python3 -c 'import ziglang' >/dev/null 2>&1; then
    zig=(python3 -m ziglang)
elif [ -x /tmp/venv/bin/python ] && /tmp/venv/bin/python -c 'import ziglang' >/dev/null 2>&1; then
    zig=(/tmp/venv/bin/python -m ziglang)
else
    echo "needs Zig: install it, or: pip install ziglang" >&2
    exit 1
fi

version="$(sed -n "s/^\$TryVersion = '\([0-9.]*\)'.*/\1/p" "$here/try.ps1" | tr -d '\r')"
[ -n "$version" ] || { echo "no \$TryVersion in try.ps1" >&2; exit 1; }
IFS=. read -r v1 v2 <<<"$version"
v2="${v2:-0}"

# try.ps1 as a C byte array (generated here, never stored in the repo).
{
    echo 'static const unsigned char TRY_PS1[] = {'
    od -An -v -tu1 "$here/try.ps1" | sed 's/  */ /g; s/^ //; s/ $//; s/ /,/g; s/$/,/' | grep -v '^,$'
    echo '};'
} >"$work/try_ps1.h"

cat >"$work/launcher.c" <<'EOF'
/* VIctory Try launcher: see scripts/try/build-exe.sh. */
#define WIN32_LEAN_AND_MEAN
#define UNICODE
#define _UNICODE
#include <windows.h>
#include <stdio.h>
#include <wchar.h>

#include "try_ps1.h"

static void wait_enter(void) {
    wprintf(L"\nPress Enter to close this window.");
    fflush(stdout);
    HANDLE in = GetStdHandle(STD_INPUT_HANDLE);
    FlushConsoleInputBuffer(in);
    wchar_t buf[8];
    DWORD n = 0;
    ReadConsoleW(in, buf, 8, &n, NULL);
}

static int fail(const wchar_t *what) {
    wprintf(L"VIctory Try: %ls (error %lu)\n", what, GetLastError());
    wait_enter();
    return 1;
}

int main(void) {
    SetConsoleTitleW(L"VIctory Try");
    /* Ctrl+C stops PowerShell (the child), not this window's last message. */
    SetConsoleCtrlHandler(NULL, TRUE);

    /* Started from inside a game run (the game path named this exe): stop
       at once instead of running VIctory Try inside itself. */
    wchar_t running[8];
    DWORD got = GetEnvironmentVariableW(L"VICTORY_TRY_RUNNING", running, 8);
    if (got > 0 && got < 8 && wcscmp(running, L"1") == 0) {
        wprintf(L"VIctory Try was started from inside a game run: not running it again.\n"
                L"In the first VIctory Try window press G and give the game's own exe.\n");
        return 3;
    }

    wchar_t home[MAX_PATH], script[MAX_PATH], ps[MAX_PATH];
    DWORD len = ExpandEnvironmentStringsW(L"%LOCALAPPDATA%\\VIctoryTry", home,
                                          MAX_PATH);
    if (len == 0 || len > MAX_PATH || wcschr(home, L'%'))
        return fail(L"LOCALAPPDATA is not set");
    CreateDirectoryW(home, NULL);
    swprintf(script, MAX_PATH, L"%ls\\try.ps1", home);

    HANDLE f = CreateFileW(script, GENERIC_WRITE, 0, NULL, CREATE_ALWAYS,
                           FILE_ATTRIBUTE_NORMAL, NULL);
    if (f == INVALID_HANDLE_VALUE)
        return fail(L"cannot write try.ps1");
    DWORD wrote = 0;
    BOOL ok = WriteFile(f, TRY_PS1, (DWORD)sizeof TRY_PS1, &wrote, NULL);
    CloseHandle(f);
    if (!ok || wrote != sizeof TRY_PS1)
        return fail(L"cannot write try.ps1");

    len = ExpandEnvironmentStringsW(
        L"%SystemRoot%\\System32\\WindowsPowerShell\\v1.0\\powershell.exe", ps,
        MAX_PATH);
    if (len == 0 || len > MAX_PATH ||
        GetFileAttributesW(ps) == INVALID_FILE_ATTRIBUTES)
        wcscpy(ps, L"powershell.exe");

    static wchar_t cmd[2048];
    swprintf(cmd, 2048,
             L"\"%ls\" -NoProfile -ExecutionPolicy Bypass -File \"%ls\"", ps,
             script);

    STARTUPINFOW si = {sizeof si};
    PROCESS_INFORMATION pi;
    if (!CreateProcessW(NULL, cmd, NULL, NULL, FALSE, 0, NULL, home, &si, &pi))
        return fail(L"cannot start Windows PowerShell");
    WaitForSingleObject(pi.hProcess, INFINITE);
    DWORD code = 0;
    GetExitCodeProcess(pi.hProcess, &code);
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);

    wprintf(L"\nVIctory Try closed. Start it again with VIctoryTry.exe.\n");
    wait_enter();
    return (int)code;
}
EOF

cat >"$work/VIctoryTry.manifest" <<'EOF'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="VIctory.Try" version="1.0.0.0"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security><requestedPrivileges>
      <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
    </requestedPrivileges></security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
EOF

cat >"$work/VIctoryTry.rc" <<EOF
#include <windows.h>
1 RT_MANIFEST "VIctoryTry.manifest"
VS_VERSION_INFO VERSIONINFO
FILEVERSION $v1,$v2,0,0
PRODUCTVERSION $v1,$v2,0,0
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "VIctory contributors"
      VALUE "FileDescription", "VIctory Try"
      VALUE "FileVersion", "$version"
      VALUE "InternalName", "VIctoryTry"
      VALUE "LegalCopyright", "GPL-3.0-or-later"
      VALUE "OriginalFilename", "VIctoryTry.exe"
      VALUE "ProductName", "VIctory Try"
      VALUE "ProductVersion", "$version"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
EOF

(cd "$work" && "${zig[@]}" build-exe launcher.c VIctoryTry.rc -lc \
    -target x86_64-windows-gnu -O ReleaseSmall -fstrip \
    --name VIctoryTry -femit-bin="$out/VIctoryTry.exe")
rm -f "$out"/*.pdb "$out"/*.lib
echo "built $out/VIctoryTry.exe (VIctory Try $version, $(wc -c <"$out/VIctoryTry.exe") bytes)"
