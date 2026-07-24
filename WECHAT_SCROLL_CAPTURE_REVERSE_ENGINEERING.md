# 微信滚动截图逆向分析记录

## 1. 分析范围

本文记录对微信 Windows 版滚动截图功能的静态分析结果。

- 微信版本：`4.1.11.55`
- 主程序模块：`Weixin.dll`
- OCR/图像模块：`WeChatOcr.bin`
- `WeChatOcr.bin` 内部模块：`wxocr.dll`

文中“已确认”表示能够由反汇编、导出函数或常量直接证明。无法从当前静态证据确认的行为会单独标注，不将推断写成事实。

## 2. 使用的逆向工具

### LLVM 工具链

- `llvm-objdump.exe`：按虚拟地址反汇编函数，检查调用顺序、字段访问和常量。
- `llvm-strings.exe`：提取类名、Qt 元对象方法名和工作线程名称，并显示文件偏移。
- `llvm-readobj.exe`：读取 PE 节区、导入表和地址布局。

使用示例：

```powershell
llvm-objdump.exe -d --start-address=0x181C3E510 --stop-address=0x181C3ED80 Weixin.dll
llvm-strings.exe --radix=x Weixin.dll
llvm-readobj.exe --sections --coff-imports Weixin.dll
```

### Visual Studio PE 工具

- `dumpbin.exe /imports`：交叉检查 Win32 导入函数，例如 `GetDC`、`BitBlt` 和 `PrintWindow`。

### 辅助分析工具

- PowerShell：整理反汇编结果、筛选字符串和统计地址引用。
- `rg`：在反汇编文本和导入表中检索函数、常量及地址。
- OpenCV API 文档：核对 matcher 枚举值和 ORB 参数含义。

没有使用调试器注入微信进程，也没有修改微信二进制。结论来自静态反汇编、PE 元数据、导出函数和常量分析。

## 3. 微信滚动截图架构

微信滚动截图相关 Qt 类名已从 `Weixin.dll` 中确认：

- `weshot::LongScreenShoter`
- `weshot::GrabWorker`
- `weshot::OpenCVWorker`
- `weshot::SpliceWorker`

对应线程名称：

- `longscreenshoter_grab_thread`
- `longscreenshoter_opencvworker_thread`
- `longscreenshoter_splice_thread`

总体数据流为：

```text
LongScreenShoter
    |
    +-- GrabWorker       连续抓取屏幕选区，生成有序工作项
    |
    +-- OpenCVWorker     依次计算相邻帧垂直位移，维护累计位置和边界
    |
    +-- SpliceWorker     只拼接超出已捕获范围的新像素条带
```

三个阶段位于独立线程中。`OpenCVWorker` 按工作序号处理帧，不能任意清空中间帧后继续拿旧基准匹配新帧。

## 4. GrabWorker

### 已确认行为

`GrabWorker` 保存以下信息：

- `QScreen*`
- 屏幕坐标选区
- `QVector<HWND>` 排除窗口列表
- 停止标志

Qt 元对象中可见的方法：

- `SetTarget`
- `SetScreen`
- `SetExcludeHwnds`
- `doWork`
- `Stop`

`doWork` 是连续循环，没有发现固定睡眠阈值。每轮将屏幕、选区和排除窗口列表传给抓屏 helper，然后投递捕获结果。

这证明微信的长截图像素来源是屏幕合成结果，而不是对目标窗口直接调用 `PrintWindow`。排除窗口列表用于避免截图工具自身窗口进入画面。

## 5. OpenCVWorker

### 外层处理函数

已定位：

```text
weshot::OpenCVWorker::handle_works
VA 0x181C3E510
```

外层 worker 的主要行为：

1. 按工作序号取得相邻图像。
2. 调用图像模块计算垂直 offset。
3. offset 为零时不扩展画布。
4. 校验 `abs(offset) <= image.height * 0.6`。
5. 更新累计位置和已捕获上下边界。
6. 只有新帧越过现有边界时，才向 `SpliceWorker` 发送拼接工作。

`0.6` 常量位于：

```text
VA 0x18858AB80
```

该限制是微信明确存在的外层大位移准入条件之一。

### GetMergeOffsetInner

`WeChatOcr.bin` 是 ZIP 格式容器。提取出的 `wxocr.dll` 导出：

```text
GetMergeOffsetInner
RVA 0x22520
VA  0x180022520
```

已确认 ORB 参数：

| 参数 | 值 |
|---|---:|
| nfeatures | 2000 |
| scaleFactor | 1.2 |
| nlevels | 8 |
| edgeThreshold | 31 |
| firstLevel | 0 |
| WTA_K | 2 |
| scoreType | HARRIS |
| patchSize | 31 |
| fastThreshold | 20 |

已确认匹配规则：

- `DescriptorMatcher::create(6)`，即 `BRUTEFORCE_SL2`/平方 L2 距离。
- KNN 的 `k = 5`。
- Lowe ratio 为 `0.75`。
- descriptor distance 上限为 `20`。
- 匹配点水平坐标差绝对值不超过 `4px`。
- 垂直差绝对值小于 `2px` 时归一化为零。
- 候选 offset 的支持范围为 `+/-1px`。
- 选择支持数最多的候选 offset。
- 相邻帧顶部和底部完全相同的静态行会被排除，同时保留 `31px` 特征边界。

微信外层只消费最终 offset。虽然内部会计算支持数，但没有证据表明外层使用额外的“最小支持数阈值”。

## 6. SpliceWorker

Qt 元对象中可见的方法：

- `handle_init_pixmap`
- `handle_splice_pimxap`（微信二进制中的原始拼写）
- `update_preview`
- `SetScreen`

`OpenCVWorker` 维护累计位置、最小边界和最大边界。匹配成功不等于一定写入画布：

- 新帧仍位于已捕获范围内：只推进匹配基准。
- 新帧越过顶部：拼接顶部新增条带。
- 新帧越过底部：拼接底部新增条带。
- offset 为零：不改变画布。
- 超过 60% 限制或无法匹配：拒绝该帧，不把像素写入画布。

这避免了回滚后再次经过旧内容时重复拼接整帧。

## 7. 已确认的算法常量

以下数值均能从微信二进制或 `wxocr.dll` 反汇编中直接确认：

| 类别 | 数值 |
|---|---:|
| ORB 特征数 | 2000 |
| ORB 缩放因子 | 1.2 |
| ORB 金字塔层数 | 8 |
| ORB 边界 | 31 |
| ORB WTA_K | 2 |
| ORB patchSize | 31 |
| ORB fastThreshold | 20 |
| DescriptorMatcher | BRUTEFORCE_SL2 (6) |
| KNN k | 5 |
| Lowe ratio | 0.75 |
| descriptor distance 上限 | 20 |
| 水平坐标误差 | 4px |
| 垂直差归零范围 | 绝对值小于 2px |
| offset 支持范围 | +/-1px |
| 最大位移 | 帧高的 60% |

## 8. 当前静态分析未确认的事项

以下内容没有足够的静态证据，不应当作为微信既有规则：

- 外层最小特征支持数阈值。
- 根据滚轮速度动态改变位移上限。
- 除 ORB 规则外的额外整图相似度阈值。
- 队列积压时是否主动覆盖旧帧。
- `SetExcludeHwnds` 抓屏 helper 对每个排除窗口的具体像素修复算法。
- 动态内容、视频和透明窗口的全部特殊处理分支。

已确认 `GrabWorker` 连续产生工作项，`OpenCVWorker` 按工作序号处理，`SpliceWorker` 只接收需要扩展边界的像素条带；但 Qt 信号连接的所有运行时队列策略仍需动态调试才能完全确定。
