# Crate `paddleocr`

[![](https://img.shields.io/crates/v/paddleocr.svg)](https://crates.io/crates/paddleocr/)

A simple wrapper for [`hiroi-sora/PaddleOCR-json`](https://github.com/hiroi-sora/PaddleOCR-json).

## Usage

```rust
let mut p = paddleocr::Ppocr::new(
    PathBuf::from(".../PaddleOCR-json.exe"), // path to binary
    Default::default(), // language config_path, default `zh_CN`
)
.unwrap(); // initialize

let now = std::time::Instant::now(); // benchmark
{
    // OCR files
    println!("{}", p.ocr(Path::new(".../test1.png").into()).unwrap());
    println!("{}", p.ocr(Path::new(".../test2.png").into()).unwrap());
    println!("{}", p.ocr(Path::new(".../test3.png").into()).unwrap());

    // OCR clipboard
    println!("{}", p.ocr_clipboard().unwrap());    
}
println!("Elapsed: {:.2?}", now.elapsed());
```

Use `ocr_and_parse` to get structured results.

By enabling the `bytes` feature, you can pass image data as a byte array (`AsRef<[u8]>`).
