use std::io::Result as IoResult;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process;
use std::{error::Error, fmt, path::PathBuf};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

// Windows进程创建标志：隐藏窗口
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW_FLAG: u32 = 0x08000000;

use serde::{
    Deserialize, // for `ocr_and_parse`
    Serialize,   // for `WriteDict`
};

#[derive(Debug, Clone)]
pub struct OsNotSupportedError;
impl fmt::Display for OsNotSupportedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "OS not supported")
    }
}
impl Error for OsNotSupportedError {}

type Point = [usize; 2];

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum OcrRec {
    Content { code: u32, data: Vec<ContentData> },
    Message { code: u32, data: String },
}

#[derive(Deserialize, Debug, Clone)]
pub struct ContentData {
    #[serde(rename(deserialize = "box"))]
    pub rect: Rectangle,
    pub score: f64,
    pub text: String,
}

pub type Rectangle = [Point; 4];

/**
 * The image to be recognized.
 */
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ImageData {
    ImagePathDict { image_path: String },
    ImageBase64Dict { image_base64: String },
}

impl ImageData {
    /**
     * Create an `ImageData` from a file path.
     */
    pub fn from_path<S>(path: S) -> ImageData
    where
        S: AsRef<str> + std::fmt::Display,
    {
        ImageData::ImagePathDict {
            image_path: path.to_string(),
        }
    }
    /**
     * Create an `ImageData` from a base64 string.
     */
    pub fn from_base64(base64: String) -> ImageData {
        ImageData::ImageBase64Dict {
            image_base64: base64,
        }
    }
    /**
     * Create an `ImageData` from a byte slice.
     * Requires the `bytes` feature.
     */
    #[cfg(feature = "bytes")]
    pub fn from_bytes<T>(bytes: T) -> ImageData
    where
        T: AsRef<[u8]>,
    {
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        ImageData::ImageBase64Dict {
            image_base64: engine.encode(bytes),
        }
    }
}

impl From<&Path> for ImageData {
    fn from(path: &Path) -> Self {
        ImageData::from_path(path.to_string_lossy())
    }
}
impl From<PathBuf> for ImageData {
    fn from(path: PathBuf) -> Self {
        ImageData::from_path(path.to_string_lossy())
    }
}

/**
 * A paddleocr-json instance.
 */
pub struct Ppocr {
    #[allow(dead_code)]
    exe_path: PathBuf,
    process: process::Child,
}

impl Ppocr {
    /**
    Initialize a new instance.

    # Examples

    ```no_run
    let mut p = paddleocr::Ppocr::new(
        PathBuf::from(".../PaddleOCR-json.exe"), // path to binary
        Default::default(), // language config_path, default `zh_CN`
    )
    .unwrap(); // initialize
    ```
    */
    pub fn new(exe_path: PathBuf, config_path: Option<PathBuf>) -> Result<Ppocr, Box<dyn Error>> {
        std::env::set_var("RUST_BACKTRACE", "full");
        if !cfg!(target_os = "windows") {
            return Err(Box::new(OsNotSupportedError {}));
        }
        if !exe_path.exists() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Executable not found",
            )));
        }

        let wd = exe_path
            .canonicalize()?
            .parent()
            .ok_or_else(|| "No parent directory found")?
            .to_path_buf();

        let mut command = process::Command::new(&exe_path);
        command.current_dir(wd);
        if let Some(config_path) = config_path {
            command.args(&["--config_path", &config_path.to_string_lossy()]);
        }

        // 在Windows上隐藏控制台窗口
        #[cfg(target_os = "windows")]
        command.creation_flags(CREATE_NO_WINDOW_FLAG);

        let process = command
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .stdin(process::Stdio::piped())
            .spawn()?;

        let mut p = Ppocr { exe_path, process };

        for _i in 1..10 {
            match p.read_line() {
                Ok(line) => {
                    if line.contains("OCR init completed.")
                        || line.contains("Image path dose not exist")
                    {
                        break; // successfully initialized
                    } /*  else if line.contains("PaddleOCR-json v1.2.1") {
                    // in v1.2.1 the last line cannot be read by read_line
                    p.write_fmt(format_args!("\n")).err();
                    } */
                }
                Err(e) => {
                    return Err(Box::new(e));
                }
            }
        }

        Ok(p)
    }

    fn read_line(&mut self) -> IoResult<String> {
        let mut buff = String::new();
        let mut stdout = BufReader::new(self.process.stdout.as_mut().unwrap());
        match stdout.read_line(&mut buff) {
            Ok(_siz) => Ok(buff),
            Err(e) => Err(e),
        }
    }

    #[inline]
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> IoResult<()> {
        let inner = self.process.stdin.as_mut().ok_or(std::io::Error::new(
            std::io::ErrorKind::Other,
            "stdin not piped",
        ))?;
        inner.write_fmt(fmt)
    }

    /**
    OCRs the image at the given path. Note that the returned JSON is not parsed or checked, and a valid JSON does not necessarily mean it is successful.

    # Examples

    ```no_run
    let mut p = paddleocr::Ppocr::new(
        PathBuf::from(".../PaddleOCR-json.exe"), // path to binary
        Default::default(), // language config_path, default `zh_CN`
    )
    .unwrap(); // initialize
    println!("{}", p.ocr(Path::new(".../test.png").into()));
    ```
    # Results

    ## Return values

    通过API调用一次OCR，无论成功与否，都会返回一个字典。

    字典中，根含两个元素：状态码`code`和内容`data`。

    状态码`code`为整数，每种状态码对应一种情况：

    ### `100` 识别到文字

    - data内容为数组。数组每一项为字典，含三个元素：
      - `text` ：文本内容，字符串。
      - `box` ：文本包围盒，长度为4的数组，分别为左上角、右上角、右下角、左下角的`[x,y]`。整数。
      - `score` ：识别置信度，浮点数。
    - 例：
      ```
        {'code':100,'data':[{'box':[[13,5],[161,5],[161,27],[13,27]],'score':0.9996442794799805,'text':'飞舞的因果交流'}]}
      ```

    ### `101` 未识别到文字

    - data为字符串：`No text found in image. Path:"图片路径"`
    - 例：```{'code':101,'data':'No text found in image. Path: "D:\\空白.png"'}```
    - 这是正常现象，识别没有文字的空白图片时会出现这种结果。

    ### `200` 图片路径不存在

    - data：`Image path dose not exist. Path:"图片路径".`
    - 例：`{'code':200,'data':'Image path dose not exist. Path: "D:\\不存在.png"'}`
    - 注意，在系统未开启utf-8支持（`使用 Unicode UTF-8 提供全球语言支持"`）时，不能读入含emoji等特殊字符的路径（如`😀.png`）。但一般的中文及其他 Unicode 字符路径是没问题的，不受系统区域及默认编码影响。

    ### `201` 图片路径string无法转换到wstring

    - data：`Image path failed to convert to utf-16 wstring. Path: "图片路径".`
    - 使用API时，理论上不会报这个错。
    - 开发API时，若传入字符串的编码不合法，有可能报这个错。

    ### `202` 图片路径存在，但无法打开文件

    - data：`Image open failed. Path: "图片路径".`
    - 可能由系统权限等原因引起。

    ### `203` 图片打开成功，但读取到的内容无法被opencv解码

    - data：`Image decode failed. Path: "图片路径".`
    - 注意，引擎不以文件后缀来区分各种图片，而是对存在的路径，均读入字节尝试解码。若传入的文件路径不是图片，或图片已损坏，则会报这个错。
    - 反之，将正常图片的后缀改为别的（如`.png`改成`.jpg或.exe`），也可以被正常识别。

    ### `210` 剪贴板打开失败

    - data：`Clipboard open failed.`
    - 可能由别的程序正在占用剪贴板等原因引起。

    ### `211` 剪贴板为空

    - data：`Clipboard is empty.`

    ### `212` 剪贴板的格式不支持

    - data：`Clipboard format is not valid.`
    - 引擎只能识别剪贴板中的位图或文件。若不是这两种格式（如复制了一段文本），则会报这个错。

    ### `213` 剪贴板获取内容句柄失败

    - data：`Getting clipboard data handle failed.`
    - 可能由别的程序正在占用剪贴板等原因引起。

    ### `214` 剪贴板查询到的文件的数量不为1

    - data：`Clipboard number of query files is not valid. Number: 文件数量`
    - 只允许一次复制一个文件。一次复制多个文件再调用OCR会得到此报错。

    ### `215` 剪贴板检索图形对象信息失败

    - data：`Clipboard get bitmap object failed.`
    - 剪贴板中是位图，但获取位图信息失败。可能由别的程序正在占用剪贴板等原因引起。

    ### `216` 剪贴板获取位图数据失败

    - data：`Getting clipboard bitmap bits failed.`
    - 剪贴板中是位图，获取位图信息成功，但读入缓冲区失败。可能由别的程序正在占用剪贴板等原因引起。

    ### `217` 剪贴板中位图的通道数不支持

    - data：`Clipboard number of image channels is not valid. Number: 通道数`
    - 引擎只允许读入通道为1（黑白）、3（RGB）、4（RGBA）的图片。位图通道数不是1、3或4，会报这个错。

    ### `300` base64字符串解析为string失败

    - data：`Base64 decode failed.`
    - 传入非法Base64字符串引起。（注意，传入Base64信息不应带有`data:image/jpg;base64,`前缀。）

    ### `301` base64字符串解析成功，但读取到的内容无法被opencv解码

    - data：`Base64 data imdecode failed.`

    ### `400` json对象 转字符串失败

    - data：`Json dump failed.CODE_ERR_JSON_DUMP`
    - 输入异常：传入非法json字符串，或者字符串含非utf-8编码字符导致无法解析引起。

    ### `401` json字符串 转对象失败

    - data：`Json dump failed.CODE_ERR_JSON_DUMP`
    - 输出异常：输出时OCR结果无法被编码为json字符串。

    ### `402` json对象 解析某个键时失败

    - data：`Json parse key 键名 failed.`
    - 比错误码`400`更精准的提示。如果发生异常，程序优先报`402`，无法处理才报`400`。

    ### `403` 未发现有效任务

    - data：`No valid tasks.`
    - 本次传入的指令中不含有效任务。
        */

    pub fn ocr(&mut self, image: ImageData) -> IoResult<String> {
        let s = serde_json::to_string(&image).unwrap().replace("\n", "");
        self.write_fmt(format_args!("{}\n", s))?;
        self.read_line()
    }

    /**
    OCRs the image in clipboard. Note that the returned JSON is not parsed or checked, and a valid JSON does not necessarily mean it is successful.

    # Examples

    ```no_run
    let mut p = paddleocr::Ppocr::new(
        PathBuf::from(".../PaddleOCR-json.exe"), // path to binary
        Default::default(), // language config_path, default `zh_CN`
    )
    .unwrap(); // initialize
    println!("{}", p.ocr_clipboard());
    ```
    */
    #[inline]
    pub fn ocr_clipboard(&mut self) -> IoResult<String> {
        self.ocr(ImageData::from_path("clipboard"))
    }

    pub fn ocr_and_parse(&mut self, image: ImageData) -> Result<Vec<ContentData>, String> {
        let ocr_result = self.ocr(image);
        let Ok(ocr_string) = ocr_result.as_ref() else {
            return Err("OCR failed".to_string());
        };
        match serde_json::from_str::<OcrRec>(&ocr_string) {
            Ok(OcrRec::Content { data, .. }) => Ok(data),
            Ok(OcrRec::Message { code, data }) => Err(format!("Error Message {}: {}", code, data)),
            Err(e) => Err(format!("Response JSON parse failed: {}", e)),
        }
    }
}

impl Drop for Ppocr {
    /**
     * Kill the process when the instance is dropped.
     */
    fn drop(&mut self) {
        self.process.kill().err();
    }
}

#[cfg(test)]

mod tests {
    use std::path::{Path, PathBuf};

    use crate::{ImageData, Ppocr};
    #[test]
    fn recognize() {
        let mut p = Ppocr::new(
            PathBuf::from(
                "E:/code/paddleocr/v1.4.0/PaddleOCR-json.exe", // path to binary
            ),
            Default::default(),
        )
        .unwrap(); // initialize

        let now = std::time::Instant::now(); // benchmark
        {
            // OCR files
            println!(
                "{}",
                p.ocr(Path::new("C:/Users/Neko/Pictures/test1.png").into())
                    .unwrap()
            );
            println!(
                "{}",
                p.ocr(Path::new("C:/Users/Neko/Pictures/test2.png").into())
                    .unwrap()
            );
            println!(
                "{}",
                p.ocr(Path::new("C:/Users/Neko/Pictures/test3.png").into())
                    .unwrap()
            );
            println!(
                "{}",
                p.ocr(Path::new("C:/Users/Neko/Pictures/test4.png").into())
                    .unwrap()
            );
            println!(
                "{}",
                p.ocr(Path::new("C:/Users/Neko/Pictures/test5.png").into())
                    .unwrap()
            );

            // OCR clipboard
            println!("{}", p.ocr_clipboard().unwrap());
        }
        println!("Elapsed: {:.2?}", now.elapsed());
    }

    #[test]
    fn parse() {
        let mut p = Ppocr::new(
            PathBuf::from("E:/code/paddleocr/v1.4.0/PaddleOCR-json.exe"), // path to binary
            Default::default(), // language config_path, default `zh_CN`
        )
        .unwrap(); // initialize

        // OCR files
        p.ocr_and_parse(Path::new("C:/Users/Neko/Pictures/test2.png").into())
            .unwrap();

        p.ocr_and_parse(ImageData::from_bytes(include_bytes!(
            "C:/Users/Neko/Pictures/test3.png"
        )))
        .unwrap();
    }
}
