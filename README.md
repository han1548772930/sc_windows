# 截图工具 (Screenshot Tool)

一个功能丰富的Windows原生截图工具，支持截图、标注、OCR文字识别等功能。

## ✨ 主要功能

### 📸 截图功能
- **全屏截图**: 支持全屏幕截图
- **区域选择**: 鼠标拖拽选择任意区域
- **窗口检测**: 自动高亮检测到的窗口边界
- **实时预览**: 截图过程中实时显示选择区域

### 🎨 绘图标注
- **绘图工具**: 矩形、圆形、箭头、自由画笔
- **文字标注**: 支持添加文字说明
- **颜色选择**: 多种颜色可选
- **线条粗细**: 可调节绘图线条粗细
- **撤销功能**: 支持撤销上一步操作

### 🔍 OCR文字识别
- **智能识别**: 基于PaddleOCR引擎的高精度文字识别
- **结果展示**: 独立窗口显示识别结果和原图
- **多语言支持**: 支持5种语言的OCR识别
  - 🇨🇳 **简体中文** (默认) - 最稳定，识别精度最高
  - 🇺🇸 **英文** - 支持英文文本识别
  - 🇹🇼 **繁体中文** - 支持繁体中文识别
  - 🇯🇵 **日文** - 支持日文文本识别
  - 🇰🇷 **韩文** - 支持韩文文本识别
- **语言切换**: 在设置界面可选择OCR识别语言
- **智能配置**: 根据选择的语言自动加载对应的识别模型
- **异步启动**: OCR引擎异步启动，不阻塞界面操作
- **状态指示**: OCR按钮根据引擎状态自动启用/禁用

### 📌 固钉功能
- **窗口固钉**: 将截图结果固定在桌面上
- **拖拽移动**: 固钉窗口支持拖拽移动
- **始终置顶**: 固钉窗口始终显示在最前面

### 💾 保存功能
- **快速保存**: 一键保存到剪贴板
- **文件保存**: 支持保存为PNG等格式
- **自定义路径**: 用户可选择保存位置

### 🔧 系统集成
- **系统托盘**: 最小化到系统托盘运行
- **全局热键**: `Ctrl+Alt+S` 快速启动截图
- **右键菜单**: 托盘右键菜单快速访问功能
- **设置界面**: 简洁的设置窗口，支持OCR语言选择
![alt text](image.png)
## 🚀 快速开始

### 安装要求
- Windows 10/11
- 支持Direct2D的显卡
- **PaddleOCR引擎**: 需要 `PaddleOCR-json_v1.4.exe` 文件夹与主程序在同一目录

### 文件结构
```
截图工具/
├── sc_windows.exe                    # 主程序
├── PaddleOCR-json_v1.4.exe/        # OCR引擎文件夹
│   ├── PaddleOCR-json.exe          # OCR主程序
│   ├── models/                      # 识别模型文件
│   │   ├── config_en.txt           # 英文识别配置
│   │   ├── config_chinese_cht.txt  # 繁体中文识别配置
│   │   ├── config_japan.txt        # 日文识别配置
│   │   ├── config_korean.txt       # 韩文识别配置
│   │   └── *.infer/                # 各语言识别模型
│   └── *.dll                        # 运行时依赖库
└── README.md                        # 说明文档

开发环境文件结构/
├── src/                             # 主程序源代码
├── paddleocr/                       # 本地PaddleOCR库
│   ├── src/lib.rs                   # PaddleOCR Rust封装
│   └── Cargo.toml                   # 本地库配置
├── PaddleOCR-json_v1.4.exe/        # OCR引擎文件夹
├── Cargo.toml                       # 项目配置
└── README.md                        # 说明文档
```

### 使用方法

1. **启动程序**: 运行exe文件，程序将最小化到系统托盘
   - 程序启动时会自动检测PaddleOCR引擎
   - 如果检测到OCR引擎，会异步启动并准备就绪
2. **开始截图**:
   - 使用热键 `Ctrl+Alt+S`
   - 或点击托盘图标
3. **选择区域**: 鼠标拖拽选择要截图的区域
4. **标注编辑**: 使用工具栏进行绘图标注
5. **文字识别**:
   - 点击OCR按钮识别图片中的文字
   - 如果OCR引擎未就绪，按钮会显示为灰色不可点击
   - 引擎就绪后按钮自动变为可用状态
   - 识别结果会在新窗口中显示，包含原图和识别的文字
6. **OCR语言设置**:
   - 右键点击系统托盘图标，选择"设置"
   - 在设置窗口中选择"OCR识别语言"
   - 可选择：简体中文、英文、繁体中文、日文、韩文
   - 点击"确定"保存设置，下次OCR时会使用选择的语言
7. **保存结果**:
   - 点击确认按钮保存到剪贴板
   - 点击保存按钮选择文件保存位置
   - 点击固钉按钮将截图固定在桌面

### 工具栏说明
- 🏹 **箭头**: 选择和移动工具
- ⬜ **矩形**: 绘制矩形框
- ⭕ **圆形**: 绘制圆形
- ✏️ **画笔**: 自由绘制
- 📝 **文字**: 添加文字标注
- ↩️ **撤销**: 撤销上一步操作
- 🔍 **OCR**: 文字识别
- 💾 **保存**: 保存到文件
- 📌 **固钉**: 固定到桌面
- ✅ **确认**: 保存到剪贴板并关闭
- ❌ **取消**: 取消截图

## 🛠️ 技术特性

- **Rust语言**: 使用Rust开发，性能优异，内存安全
- **Windows API**: 直接调用Windows原生API，系统集成度高
- **Direct2D**: 使用Direct2D进行高性能图形渲染
- **PaddleOCR集成**: 集成PaddleOCR-json引擎，高精度文字识别
- **异步处理**: OCR引擎异步启动和状态检查，不阻塞UI
- **智能状态管理**: 按钮状态根据OCR引擎可用性自动更新
- **SVG图标**: 使用SVG矢量图标，界面清晰美观

## 📋 系统要求

- **操作系统**: Windows 10 1903 或更高版本
- **内存**: 至少 200MB 可用内存（包含OCR引擎）
- **显卡**: 支持Direct2D的显卡
- **磁盘空间**: 约 150MB（包含PaddleOCR引擎和模型文件）
- **运行时**: Visual C++ 2019 Redistributable（通常系统已包含）

## 🔧 编译说明

```bash
# 克隆项目
git clone <repository-url>
cd sc_windows

# 确保PaddleOCR引擎文件夹存在
# 下载PaddleOCR-json_v1.4.exe文件夹并放置在项目根目录

# 编译发布版本
cargo build --release

# 运行程序
./target/release/sc_windows.exe
```

### 获取PaddleOCR引擎
1. 下载 PaddleOCR-json_v1.4.exe 文件夹
2. 将整个文件夹放置在与 `sc_windows.exe` 相同的目录中
3. 确保文件夹结构完整，包含所有 `.dll` 文件和 `models/` 目录

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🤝 贡献

欢迎提交Issue和Pull Request来改进这个项目！

## ⚠️ 重要说明

### OCR功能使用注意事项
- **文件依赖**: OCR功能需要 `PaddleOCR-json_v1.4.exe` 文件夹与主程序在同一目录
- **首次启动**: 程序启动时会自动检测并启动OCR引擎，可能需要几秒钟时间
- **按钮状态**: OCR按钮会根据引擎状态自动变为可用（正常）或禁用（灰色）
- **错误处理**: 如果OCR引擎启动失败，会显示友好的错误提示信息

### 故障排除
如果OCR功能无法使用，请检查：
1. `PaddleOCR-json_v1.4.exe` 文件夹是否存在
2. 文件夹内的 `PaddleOCR-json.exe` 是否完整
3. 所有 `.dll` 文件是否齐全
4. `models/` 目录是否包含模型文件
5. 是否有足够的磁盘空间和内存

#### OCR语言识别问题
- **简体中文**: 默认语言，最稳定可靠
- **英文**: 通常工作良好，适合英文文档识别
- **繁体中文**: 支持繁体中文文本识别
- **日文/韩文**: 如果识别失败，建议切换回简体中文或英文
- **语言切换**: 在设置中更改OCR语言后，重新进行识别即可生效

---
