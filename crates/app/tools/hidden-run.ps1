# Runs a program on a hidden Windows desktop and waits for it. Its windows never show on the user's screen and
# never take the foreground. The desktop is never switched to. On a timeout the whole process tree is killed.
# It logs the foreground window before and after the run, and warns when it changed.
# Usage: pwsh -File hidden-run.ps1 -Exe <path> -Arguments "<command line arguments>" [-TimeoutSec 60]
# Example: -Exe target\release\examples\snapshot.exe -Arguments "skia overview out.png 1"
# (snapshot sets SLINT_SCALE_FACTOR itself from its scale argument, default 1).
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$Arguments = "",
    [int]$TimeoutSec = 60
)

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class HiddenRun {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct STARTUPINFO {
        public int cb; public string lpReserved; public string lpDesktop; public string lpTitle;
        public int dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
        public short wShowWindow, cbReserved2; public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError;
    }
    [StructLayout(LayoutKind.Sequential)]
    struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public int dwProcessId, dwThreadId; }

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr CreateDesktop(string name, IntPtr device, IntPtr devmode, int flags, uint access, IntPtr sa);
    [DllImport("user32.dll", SetLastError = true)]
    static extern bool CloseDesktop(IntPtr desk);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    // lpCommandLine must be writable: CreateProcessW may change it in place.
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool CreateProcess(string app, StringBuilder cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags,
        IntPtr env, string dir, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
    [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr h, uint ms);
    [DllImport("kernel32.dll")] static extern bool GetExitCodeProcess(IntPtr h, out uint code);
    [DllImport("kernel32.dll")] static extern bool TerminateProcess(IntPtr h, uint code);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);

    public static string Foreground() {
        IntPtr h = GetForegroundWindow(); uint pid; GetWindowThreadProcessId(h, out pid);
        return "hwnd 0x" + h.ToString("X") + " pid " + pid;
    }

    public static int Run(string exe, string args, int timeoutSec) {
        string name = "dr-hidden-" + Guid.NewGuid().ToString("N").Substring(0, 8);
        IntPtr desk = CreateDesktop(name, IntPtr.Zero, IntPtr.Zero, 0, 0x10000000u, IntPtr.Zero);
        if (desk == IntPtr.Zero) throw new Exception("CreateDesktop failed: " + Marshal.GetLastWin32Error());
        var pi = new PROCESS_INFORMATION();
        try {
            var si = new STARTUPINFO(); si.cb = Marshal.SizeOf(si); si.lpDesktop = name;
            var cmd = new StringBuilder("\"" + exe + "\" " + args, 32768);
            if (!CreateProcess(null, cmd, IntPtr.Zero, IntPtr.Zero, false, 0, IntPtr.Zero, null, ref si, out pi))
                throw new Exception("CreateProcess failed: " + Marshal.GetLastWin32Error());
            uint code = 259;
            if (WaitForSingleObject(pi.hProcess, (uint)(timeoutSec * 1000)) != 0) {
                // Kill the whole tree, not only the root process.
                try {
                    var kill = System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(
                        "taskkill.exe", "/PID " + pi.dwProcessId + " /T /F") { UseShellExecute = false, CreateNoWindow = true });
                    kill.WaitForExit(10000);
                } catch (Exception) { }
                TerminateProcess(pi.hProcess, 1); code = 1460;
            } else {
                GetExitCodeProcess(pi.hProcess, out code);
            }
            return (int)code;
        } finally {
            if (pi.hThread != IntPtr.Zero) CloseHandle(pi.hThread);
            if (pi.hProcess != IntPtr.Zero) CloseHandle(pi.hProcess);
            CloseDesktop(desk);
        }
    }
}
'@

$before = [HiddenRun]::Foreground()
$code = [HiddenRun]::Run((Resolve-Path $Exe).Path, $Arguments, $TimeoutSec)
$after = [HiddenRun]::Foreground()
"foreground before: $before"
"foreground after:  $after"
if ($before -ne $after) { Write-Warning "the foreground window changed during the run (the user may have switched windows)" }
"exit code $code"
exit $code
