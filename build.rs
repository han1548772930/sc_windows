fn main() {
    // 在 Windows 上嵌入资源文件
    #[cfg(target_os = "windows")]
    {
        embed_resource::compile("resources.rc", embed_resource::NONE);
    }
}
