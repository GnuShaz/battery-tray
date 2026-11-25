fn main() {
    // Slint нужен только на Windows, чтобы `cargo test` с превью иконок работал везде.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        slint_build::compile("ui/popup.slint").unwrap();
    }
}
