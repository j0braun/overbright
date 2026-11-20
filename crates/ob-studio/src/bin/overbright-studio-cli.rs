//! `overbright-studio-cli`: Overbright Studio from the command line (docs/modding/studio-cli.md). A console
//! program on every system, so scripts and shells wait for it and read its output.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    // The version line, as the game, the launcher and the editor give it.
    if argv.first().map(String::as_str) == Some("--version") {
        println!("{} studio command line (preview)", ob_core::build_info::version_line());
        return;
    }
    let env = ob_studio::cli::Env::system();
    let code = ob_studio::cli::run(&argv, &env, &mut std::io::stdout().lock(), &mut std::io::stderr().lock());
    std::process::exit(code);
}
