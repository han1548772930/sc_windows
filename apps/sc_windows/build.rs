fn main() {
    // Embed the Windows resource file.
    #[cfg(target_os = "windows")]
    {
        let _ = embed_resource::compile("resources.rc", embed_resource::NONE);
    }
}
