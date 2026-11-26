//! The Windows file dialogs for Export and Import of a game (M5): `IFileSaveDialog` and `IFileOpenDialog`, owned by
//! the main window. Only `RealDesktop` calls them; tests use a fake desktop and never open a dialog.

use std::path::PathBuf;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOLDERID_Documents, FOS_FILEMUSTEXIST, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, FileOpenDialog,
    FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog, IShellItem, KF_FLAG_DEFAULT,
    SHGetKnownFolderItem, SIGDN_FILESYSPATH,
};
use windows::core::{HSTRING, Interface, w};

const FILTER: [COMDLG_FILTERSPEC; 1] = [COMDLG_FILTERSPEC {
    pszName: w!("DialogReader game (*.json)"),
    pszSpec: w!("*.json"),
}];

/// COM for this thread while the dialog is open (the dialog runs on its own thread, src/vm/mod.rs).
struct Com(bool);

impl Com {
    fn init() -> Com {
        Com(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

/// Asks where to save, starting with `suggested` as the file name. `None` when cancelled.
pub fn save_path(owner: Option<isize>, suggested: &str) -> Option<PathBuf> {
    let _com = Com::init();
    unsafe {
        let d: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        d.SetFileTypes(&FILTER).ok()?;
        d.SetDefaultExtension(w!("json")).ok()?;
        d.SetFileName(&HSTRING::from(suggested)).ok()?;
        let opts = d.GetOptions().ok()?;
        d.SetOptions(opts | FOS_OVERWRITEPROMPT | FOS_PATHMUSTEXIST)
            .ok()?;
        d.SetTitle(w!("Export game profile")).ok()?;
        run(&d.cast().ok()?, owner)
    }
}

/// Asks for a file to import. `None` when cancelled.
pub fn open_path(owner: Option<isize>) -> Option<PathBuf> {
    let _com = Com::init();
    unsafe {
        let d: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        d.SetFileTypes(&FILTER).ok()?;
        let opts = d.GetOptions().ok()?;
        d.SetOptions(opts | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST)
            .ok()?;
        d.SetTitle(w!("Import game profile")).ok()?;
        run(&d.cast().ok()?, owner)
    }
}

/// Shows the dialog (modal to `owner`) and returns the picked file. It opens in Documents the first time (not the
/// app's working folder); after that Windows remembers the last folder used.
unsafe fn run(d: &IFileDialog, owner: Option<isize>) -> Option<PathBuf> {
    unsafe {
        if let Ok(docs) =
            SHGetKnownFolderItem::<IShellItem>(&FOLDERID_Documents, KF_FLAG_DEFAULT, None)
        {
            let _ = d.SetDefaultFolder(&docs);
        }
        d.Show(owner.map(|h| HWND(h as *mut _))).ok()?;
        let item = d.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        path.map(PathBuf::from)
    }
}
