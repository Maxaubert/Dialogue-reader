; DialogReader installer (Inno Setup 6). Per-user: no admin rights, installs to %LOCALAPPDATA%\Programs\DialogReader.
; Build it with installer\build.ps1, which stages the files in target\installer\stage and runs:
;   ISCC.exe /DStage=<stage folder> installer\DialogReader.iss
; Output: target\installer\DialogReader-Setup-x64-<version>.exe
;
; The version is read from the staged DialogReader.exe (its version block, crates/app/app.rc), which comes from
; [workspace.package] version in the root Cargo.toml: the one version source.
; The user's data in %APPDATA%\DialogReader (settings, games) is never removed by the uninstaller.

#ifndef Stage
  #define Stage AddBackslash(SourcePath) + "..\target\installer\stage"
#endif
#define AppName      "DialogReader"
#define AppExe       "DialogReader.exe"
#define AppId        "{{6B0C2E4D-6F7A-4B4E-9C51-3D1B6E1F9A27}"
#define AppVersion   GetStringFileInfo(AddBackslash(Stage) + AppExe, "ProductVersion")

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=DialogReader
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
DisableDirPage=auto
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\{#AppExe}
SetupIconFile=..\crates\app\app.ico
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
LZMANumBlockThreads=4
OutputDir=..\target\installer
OutputBaseFilename={#AppName}-Setup-x64-{#AppVersion}
; A running DialogReader is asked to quit first ([Code], CloseApp). AppMutex stops Setup when one still runs, and
; Restart Manager is the last net. A DialogReader that ran before a silent update is started again ([Run]).
AppMutex=DialogReader.SingleInstance
CloseApplications=force
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
Source: "{#Stage}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
; Kokoro (Max, 2026-11-23: bundled): ONNX Runtime, the Visual C++ runtime it needs, the model and the voices
Source: "{#Stage}\onnxruntime.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\onnxruntime-LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\onnxruntime-ThirdPartyNotices.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\msvcp140.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\msvcp140_1.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\vcruntime140.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\vcruntime140_1.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\kokoro\kokoro-v1.0.onnx"; DestDir: "{app}\kokoro"; Flags: ignoreversion
Source: "{#Stage}\kokoro\voices-v1.0.bin"; DestDir: "{app}\kokoro"; Flags: ignoreversion
Source: "{#Stage}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "Start {#AppName}"; Flags: nowait postinstall skipifsilent
Filename: "{app}\{#AppExe}"; Parameters: "--minimized"; Flags: nowait; Check: StartAgainAfterSilentUpdate

[Code]
// Closing a running DialogReader (crates/app/src/single.rs). Close hides the app to the tray, so it usually runs.
// Setup and the uninstaller post WM_QUIT_APP to its hidden listener window (class DialogReader.Instance, title
// DialogReader): it ends as tray Quit does. One that does not end within 10 s, or an old one without that window,
// is ended by force. This runs in InitializeSetup and InitializeUninstall, which come before the AppMutex check.
const
  WM_QUIT_APP = 32850;
  InstanceClass = 'DialogReader.Instance';
  InstanceTitle = 'DialogReader';
  MutexName = 'DialogReader.SingleInstance';
  PROCESS_TERMINATE = $0001;
  SYNCHRONIZE = $00100000;
  WAIT_OBJECT_0 = 0;
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{6B0C2E4D-6F7A-4B4E-9C51-3D1B6E1F9A27}_is1';

var
  WasRunning: Boolean;

function FindWindowW(lpClassName, lpWindowName: String): HWND;
external 'FindWindowW@user32.dll stdcall';
function GetWindowThreadProcessId(hWnd: HWND; var lpdwProcessId: Longword): Longword;
external 'GetWindowThreadProcessId@user32.dll stdcall';
function OpenProcess(dwDesiredAccess: Longword; bInheritHandle: Bool; dwProcessId: Longword): Longword;
external 'OpenProcess@kernel32.dll stdcall';
function TerminateProcess(hProcess: Longword; uExitCode: Longword): Bool;
external 'TerminateProcess@kernel32.dll stdcall';
function WaitForSingleObject(hHandle: Longword; dwMilliseconds: Longword): Longword;
external 'WaitForSingleObject@kernel32.dll stdcall';
function CloseHandle(hObject: Longword): Bool;
external 'CloseHandle@kernel32.dll stdcall';

// Waits up to Ms milliseconds for the app's mutex to go. True when it is gone.
function MutexGone(Ms: Integer): Boolean;
var
  Waited: Integer;
begin
  Waited := 0;
  while CheckForMutexes(MutexName) and (Waited < Ms) do begin
    Sleep(100);
    Waited := Waited + 100;
  end;
  Result := not CheckForMutexes(MutexName);
end;

// Ends every DialogReader.exe that runs from Exe (64-bit PowerShell: a 32-bit one cannot read a 64-bit path).
procedure EndExe(Exe: String);
var
  Code: Integer;
begin
  StringChangeEx(Exe, '''', '''''', True);
  Log('Ending DialogReader in ' + Exe);
  Exec(ExpandConstant('{sysnative}\WindowsPowerShell\v1.0\powershell.exe'),
    '-NoProfile -NonInteractive -Command "Get-Process -Name DialogReader -ErrorAction SilentlyContinue | ' +
    'Where-Object { $_.Path -eq ''' + Exe + ''' } | Stop-Process -Force"',
    '', SW_HIDE, ewWaitUntilTerminated, Code);
end;

// Asks a running DialogReader to quit and waits for it to end. Exe (the installed exe, when known) is the fallback.
procedure CloseApp(Exe: String);
var
  Wnd: HWND;
  Pid, Proc: Longword;
begin
  if not CheckForMutexes(MutexName) then
    exit;
  WasRunning := True;
  Wnd := FindWindowW(InstanceClass, InstanceTitle);
  if Wnd <> 0 then begin
    Pid := 0;
    GetWindowThreadProcessId(Wnd, Pid);
    Proc := 0;
    if Pid <> 0 then
      Proc := OpenProcess(PROCESS_TERMINATE or SYNCHRONIZE, False, Pid);
    Log('Asking DialogReader to quit');
    PostMessage(Wnd, WM_QUIT_APP, 0, 0);
    if Proc <> 0 then begin
      if WaitForSingleObject(Proc, 10000) <> WAIT_OBJECT_0 then begin
        Log('DialogReader did not quit within 10 s: ending it');
        TerminateProcess(Proc, 1);
        WaitForSingleObject(Proc, 5000);
      end;
      CloseHandle(Proc);
    end;
    if MutexGone(3000) then
      exit;
  end;
  if Exe <> '' then begin
    EndExe(Exe);
    MutexGone(5000);
  end;
end;

// The exe of the version installed now ('' when there is none).
function InstalledExe(): String;
var
  Dir: String;
begin
  Result := '';
  if RegQueryStringValue(HKEY_CURRENT_USER, UninstallKey, 'InstallLocation', Dir) and (Dir <> '') then
    Result := AddBackslash(Dir) + '{#AppExe}';
end;

function InitializeSetup(): Boolean;
begin
  CloseApp(InstalledExe());
  Result := True;
end;

function InitializeUninstall(): Boolean;
begin
  CloseApp(ExpandConstant('{app}\{#AppExe}'));
  Result := True;
end;

// A silent update (the future in-app update) starts DialogReader again when it was running. Not after a normal
// install: its last page has a "Start DialogReader" box.
function StartAgainAfterSilentUpdate(): Boolean;
begin
  Result := WasRunning and WizardSilent();
end;

// Uninstall: Start with Windows must not point at a removed exe. The app's Run value goes; a scheduled task (made
// only by a DialogReader that ran as administrator) cannot be removed without admin rights and is left alone.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'DialogReader');
end;
