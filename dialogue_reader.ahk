#Requires AutoHotkey v2.0
#SingleInstance Force

; ============================================================================
;  Dialogue Reader — AHK control script
;
;  - Launches main.py as a child process on startup.
;  - Sends UDP commands to the Python process on hotkey press.
;  - Kills the Python process when this script exits.
;
;  Hotkey bindings live in dialogue_reader.ini next to this file. The .ini
;  is auto-created with sensible defaults on first run; edit it and reload
;  this script (right-click tray -> Reload) to change them.
;
;  AHK key syntax (cheat sheet):
;    F1..F12       function keys
;    +             Shift modifier   e.g. +F6 = Shift+F6
;    ^             Ctrl modifier    e.g. ^F6 = Ctrl+F6
;    !             Alt modifier     e.g. !F6 = Alt+F6
;    #             Win modifier     e.g. #F6 = Win+F6
;    a, b, 1, 2... letter/digit keys
;  Full reference: https://www.autohotkey.com/docs/v2/KeyList.htm
; ============================================================================

; --- file paths ---
ScriptDir := A_ScriptDir
MainPy    := ScriptDir "\main.py"
IniFile   := ScriptDir "\dialogue_reader.ini"
PythonExe := "py.exe"

; --- defaults (used when ini is missing or a key is absent) ---
DefaultBindings := Map(
    "PickRegion",        "F6",
    "PickSpeakerRegion", "^F6",
    "ClearRegions",      "+F6",
    "SpeedDown",         "F7",
    "SpeedUp",           "F8",
    "TogglePause",       "F9",
    "CycleVoice",        "F10",
    "CycleVoicePrev",    "^F10",
)
DefaultHost        := "127.0.0.1"
DefaultPort        := "7849"
DefaultHideConsole := "true"

; --- write the ini with defaults if it doesn't exist ---
EnsureIni() {
    global IniFile, DefaultBindings, DefaultHost, DefaultPort, DefaultHideConsole
    if FileExist(IniFile)
        return
    for action, key in DefaultBindings
        IniWrite(key, IniFile, "Hotkeys", action)
    IniWrite(DefaultHost,        IniFile, "Network",  "Host")
    IniWrite(DefaultPort,        IniFile, "Network",  "Port")
    IniWrite(DefaultHideConsole, IniFile, "Launcher", "HideConsole")
}
EnsureIni()

; --- read settings ---
ReadSetting(section, key, fallback) {
    global IniFile
    val := IniRead(IniFile, section, key, fallback)
    return val
}

PickKey            := ReadSetting("Hotkeys", "PickRegion",        DefaultBindings["PickRegion"])
PickSpeakerKey     := ReadSetting("Hotkeys", "PickSpeakerRegion", DefaultBindings["PickSpeakerRegion"])
ClearKey           := ReadSetting("Hotkeys", "ClearRegions",      DefaultBindings["ClearRegions"])
SpeedDownKey       := ReadSetting("Hotkeys", "SpeedDown",         DefaultBindings["SpeedDown"])
SpeedUpKey         := ReadSetting("Hotkeys", "SpeedUp",           DefaultBindings["SpeedUp"])
PauseKey           := ReadSetting("Hotkeys", "TogglePause",       DefaultBindings["TogglePause"])
CycleVoiceKey      := ReadSetting("Hotkeys", "CycleVoice",        DefaultBindings["CycleVoice"])
CycleVoicePrevKey  := ReadSetting("Hotkeys", "CycleVoicePrev",    DefaultBindings["CycleVoicePrev"])

UdpHost     := ReadSetting("Network",  "Host",        DefaultHost)
UdpPort     := Integer(ReadSetting("Network",  "Port",  DefaultPort))
HideConsole := StrLower(Trim(ReadSetting("Launcher", "HideConsole", DefaultHideConsole))) = "true"

; --- Winsock init ---
DllCall("ws2_32\WSAStartup", "UShort", 0x0202, "Ptr", Buffer(408))
sock := DllCall("ws2_32\socket", "Int", 2, "Int", 2, "Int", 17, "Ptr")  ; AF_INET, SOCK_DGRAM, IPPROTO_UDP
if (sock = -1 || sock = 0) {
    MsgBox "Winsock init failed. UDP commands won't work."
    ExitApp
}

; sockaddr_in for the destination
addr := Buffer(16, 0)
NumPut("UShort", 2, addr, 0)                                      ; AF_INET
NumPut("UShort", DllCall("ws2_32\htons", "UShort", UdpPort, "UShort"), addr, 2)
NumPut("UInt",   DllCall("ws2_32\inet_addr", "AStr", UdpHost, "UInt"), addr, 4)

SendUdp(cmd) {
    global sock, addr
    ; UTF-8 max is 4 bytes per char, plus null terminator.
    buf := Buffer(StrLen(cmd) * 4 + 1, 0)
    bytes := StrPut(cmd, buf, "UTF-8") - 1   ; subtract the null terminator
    DllCall("ws2_32\sendto",
        "Ptr",  sock,
        "Ptr",  buf,
        "Int",  bytes,
        "Int",  0,
        "Ptr",  addr,
        "Int",  16)
}

; --- launch python child ---
PyPid := 0
LaunchPython() {
    global PyPid, PythonExe, MainPy, ScriptDir, HideConsole
    ; Pass our own PID via --parent-pid. Python starts a watchdog thread
    ; that exits as soon as this AHK process dies — even if AHK is killed
    ; via TerminateProcess (which doesn't trigger OnExit Cleanup), the
    ; watchdog notices within ~1 second and Python exits. This is the
    ; safety net for any kill path that bypasses Cleanup (e.g.
    ; MIG_Launcher's ProcessClose).
    ahkPid := DllCall("kernel32\GetCurrentProcessId", "UInt")
    cmd := PythonExe ' "' MainPy '" --debug --parent-pid ' ahkPid
    ; "Hide" hides the py.exe console window. The Python process still
    ; runs normally; print() output just isn't visible. Flip HideConsole
    ; in dialogue_reader.ini to false if you want to see live output.
    if HideConsole
        Run(cmd, ScriptDir, "Hide", &PyPid)
    else
        Run(cmd, ScriptDir, , &PyPid)
    if !PyPid {
        MsgBox "Failed to launch Python: " cmd
        ExitApp
    }
}
LaunchPython()

; --- shutdown handling: kill the python process tree when this script exits ---
;
; Three-step cleanup, because killing Python from AHK is harder than it
; should be:
;
;   1. taskkill /F /T /PID PyPid — kills py.exe AND its tree, IF py.exe is
;      still alive. Sometimes py.exe (the launcher) exits after spawning
;      python.exe, so this finds nothing.
;   2. WMI scan for any python.exe / pythonw.exe whose command line still
;      references our main.py, and kill them by PID. Catches the orphan
;      python.exe that survived step 1.
;   3. Close the UDP socket and shut Winsock down.
;
; Note: this only fires for graceful exits (tray Exit, ExitApp, X button).
; If AHK is force-killed (taskkill /F on AutoHotkey64.exe), neither step
; runs, but the orphan cleanup at the start of main.py catches the leftover
; on the next launch.
OnExit(Cleanup)
Cleanup(*) {
    global PyPid, sock, MainPy
    if PyPid {
        try RunWait("taskkill.exe /F /T /PID " PyPid, , "Hide")
    }
    ; Belt-and-suspenders: also kill any python.exe still pointing at our
    ; main.py, in case py.exe had already exited and step 1 found nothing.
    try {
        wmi := ComObjGet("winmgmts:")
        procs := wmi.ExecQuery("SELECT ProcessId,CommandLine FROM Win32_Process"
            . " WHERE Name='python.exe' OR Name='pythonw.exe'")
        for proc in procs {
            cmdLine := proc.CommandLine
            if !cmdLine
                continue
            if InStr(cmdLine, MainPy)
                try RunWait("taskkill.exe /F /PID " proc.ProcessId, , "Hide")
        }
    }
    try DllCall("ws2_32\closesocket", "Ptr", sock)
    try DllCall("ws2_32\WSACleanup")
}

; --- bind hotkeys (each wrapped in try so a bad ini entry doesn't kill the script) ---
TryBind(keyStr, cmdName) {
    try {
        Hotkey(keyStr, (*) => SendUdp(cmdName))
    } catch as e {
        MsgBox "Couldn't bind hotkey '" keyStr "' for " cmdName ":`n" e.Message
            . "`n`nEdit dialogue_reader.ini and reload."
    }
}

TryBind(PickKey,            "PICK_REGION")
TryBind(PickSpeakerKey,     "PICK_SPEAKER")
TryBind(ClearKey,           "CLEAR_REGIONS")
TryBind(SpeedDownKey,       "SPEED_DOWN")
TryBind(SpeedUpKey,         "SPEED_UP")
TryBind(PauseKey,           "TOGGLE_PAUSE")
TryBind(CycleVoiceKey,      "CYCLE_VOICE")
TryBind(CycleVoicePrevKey,  "CYCLE_VOICE_PREV")

; --- tray menu (labels reflect the actual configured bindings) ---
A_TrayMenu.Delete()
A_TrayMenu.Add("Pick Dialogue Region (" PickKey ")",        (*) => SendUdp("PICK_REGION"))
A_TrayMenu.Add("Pick Speaker Region (" PickSpeakerKey ")",  (*) => SendUdp("PICK_SPEAKER"))
A_TrayMenu.Add("Clear Regions (" ClearKey ")",              (*) => SendUdp("CLEAR_REGIONS"))
A_TrayMenu.Add()
A_TrayMenu.Add("Speed Down (" SpeedDownKey ")",             (*) => SendUdp("SPEED_DOWN"))
A_TrayMenu.Add("Speed Up (" SpeedUpKey ")",                 (*) => SendUdp("SPEED_UP"))
A_TrayMenu.Add()
A_TrayMenu.Add("Cycle Voice forward (" CycleVoiceKey ")",   (*) => SendUdp("CYCLE_VOICE"))
A_TrayMenu.Add("Cycle Voice back (" CycleVoicePrevKey ")",  (*) => SendUdp("CYCLE_VOICE_PREV"))
A_TrayMenu.Add("Toggle Pause (" PauseKey ")",               (*) => SendUdp("TOGGLE_PAUSE"))
A_TrayMenu.Add()
A_TrayMenu.Add("Open Config (.ini)",                        (*) => Run(IniFile))
A_TrayMenu.Add("Reload Script",                             (*) => Reload())
A_TrayMenu.Add()
A_TrayMenu.Add("Exit",                                      (*) => ExitApp())
A_TrayMenu.Default := "Pick Dialogue Region (" PickKey ")"
TraySetIcon("imageres.dll", 174)  ; speech-bubble icon
A_IconTip := "Dialogue Reader (" PickKey "=dialogue, " PickSpeakerKey "=speaker, " PauseKey "=pause)"
