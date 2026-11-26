//! Prints what page 02 would list on this PC: exe, size, mode and the game check. Shows no window.
//! `cargo run -p dr-runtime --example list_windows`

fn main() {
    let opts = dr_runtime::windows_list::ListOpts {
        profile_exes: vec![],
        skip_pid: None,
        icons: false,
    };
    for w in dr_runtime::windows_list::list(&opts) {
        println!(
            "{:5} {:28} {}",
            if w.game { "game" } else { "-" },
            w.exe,
            w.meta()
        );
    }
}
