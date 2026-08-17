//! Errors-only LSP server binary for Josh. `josh lsp` execs this binary; it
//! can also be launched directly. Speaks the Language Server Protocol over
//! stdin/stdout and exits when the client closes stdin.

fn main() {
    match josh_lsp::run() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("josh lsp: {error}");
            std::process::exit(2);
        }
    }
}
