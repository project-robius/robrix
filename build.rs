fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("packaging/robrix_logo.ico");
        res.compile().expect("Failed to compile Windows resources");
    }
}
