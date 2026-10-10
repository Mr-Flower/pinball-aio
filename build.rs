// Mette l'icona dentro l'eseguibile Windows.
fn main() {
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("assets/vpinball.ico")
        .compile()
        .expect("icona non incorporata");
}
