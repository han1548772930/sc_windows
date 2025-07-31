use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use windows::Win32::Foundation::*;

/// 翻译请求结构体
#[derive(Serialize)]
pub struct TranslateRequest {
    pub q: Vec<String>,    // 要翻译的文本数组
    pub source: String,    // 源语言
    pub target: String,    // 目标语言
    pub format: String,    // 格式
    pub alternatives: u32, // 备选翻译数量
    pub api_key: String,   // API密钥
}

/// 翻译响应结构体
#[derive(Deserialize)]
pub struct TranslateResponse {
    #[serde(rename = "translatedText")]
    pub translated_text: Vec<String>, // 翻译结果数组
    #[serde(rename = "detectedLanguage")]
    pub detected_language: Vec<DetectedLanguage>, // 检测到的语言数组
}

#[derive(Deserialize)]
pub struct DetectedLanguage {
    pub confidence: f32,  // 置信度
    pub language: String, // 语言代码
}

/// 翻译管理器
pub struct TranslationManager {
    api_endpoint: String,
    source_language: String,
    target_language: String,
}

impl TranslationManager {
    /// 创建新的翻译管理器
    pub fn new() -> Self {
        Self {
            api_endpoint: "".to_string(),
            source_language: "auto".to_string(),
            target_language: "en".to_string(),
        }
    }

    /// 设置源语言
    pub fn set_source_language(&mut self, language: String) {
        self.source_language = language;
    }

    /// 设置目标语言
    pub fn set_target_language(&mut self, language: String) {
        self.target_language = language;
    }

    /// 批量翻译文本
    pub async fn translate_texts(
        &self,
        texts: Vec<String>,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        println!("开始翻译 {} 个文本", texts.len());
        for (i, text) in texts.iter().enumerate() {
            println!("文本[{}]: '{}'", i, text);
        }

        let client = reqwest::Client::new();
        let request = TranslateRequest {
            q: texts.clone(),
            source: self.source_language.clone(),
            target: self.target_language.clone(),
            format: "text".to_string(),
            alternatives: 0,
            api_key: "".to_string(),
        };

        println!("发送翻译请求到: {}", self.api_endpoint);
        println!("请求数据: {:?}", serde_json::to_string(&request)?);

        let response = client
            .post(&self.api_endpoint)
            .json(&request)
            .send()
            .await?;

        println!("收到响应状态: {}", response.status());

        let response_text = response.text().await?;
        println!("响应内容: {}", response_text);

        let translate_response: TranslateResponse = serde_json::from_str(&response_text)?;

        println!(
            "解析成功，翻译结果数量: {}",
            translate_response.translated_text.len()
        );
        for (i, translation) in translate_response.translated_text.iter().enumerate() {
            println!("翻译结果[{}]: '{}'", i, translation);
        }

        Ok(translate_response.translated_text)
    }

    /// 异步翻译并通过Windows消息通知结果
    pub fn translate_async(&self, texts: Vec<String>, hwnd: HWND) {
        let api_endpoint = self.api_endpoint.clone();
        let source_language = self.source_language.clone();
        let target_language = self.target_language.clone();
        let hwnd_raw = hwnd.0 as usize;

        tokio::spawn(async move {
            let manager = TranslationManager {
                api_endpoint,
                source_language,
                target_language,
            };

            match manager.translate_texts(texts).await {
                Ok(translations) => {
                    println!("翻译完成，发送结果到窗口");
                    // 将翻译结果发送到窗口
                    for (index, translation) in translations.into_iter().enumerate() {
                        let translation_ptr = Box::into_raw(Box::new(translation));
                        unsafe {
                            let hwnd = HWND(hwnd_raw as *mut _);
                            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                                Some(hwnd),
                                windows::Win32::UI::WindowsAndMessaging::WM_USER + 1,
                                WPARAM(index),
                                LPARAM(translation_ptr as isize),
                            );
                        }
                    }
                }
                Err(e) => {
                    println!("翻译失败: {}", e);
                }
            }
        });
    }
}

impl Default for TranslationManager {
    fn default() -> Self {
        Self::new()
    }
}
