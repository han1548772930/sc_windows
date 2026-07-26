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

`doWork` 位于 VA `0x181C3D0D0`。循环体没有任何 sleep、yield 或定时器：`0x181C3D10A` 是回边，
每轮重新检查 `+0x68` 的停止标志后立即再次调用抓帧步骤。节流完全来自下游的条件变量阻塞，
而非时钟。

### 抓帧方式：Magnification API（不是 BitBlt）

这是本文此前记录错误、现已更正的关键事实。`GrabWorker::doWork` 调用 VA `0x183E46E40`
的抓帧 helper，该 helper 驱动的是 **Windows Magnification API**：

| VA | 调用 |
|---|---|
| `0x183E46F2C` | `MagInitialize` |
| `0x183E46FED` | `CreateWindowExW("MagnifierHostClass", ex=0x80028, style=0x80000000)` |
| `0x183E47011` | `SetLayeredWindowAttributes(alpha=0, LWA_ALPHA)` |
| `0x183E4707C` | `CreateWindowExW("Magnifier", style=0x50000000)` |
| `0x183E4738B` | `MagSetImageScalingCallback(callback=0x183E48E30)` |
| `0x183E474B6` | `Sleep(10)`，位于 `PeekMessage` 泵内 |
| `0x183E474BE` | `cmpl $0x8, %ebx` —— 泵最多 9 轮 |

窗口类名字符串：`MagnifierHostClass` @ VA `0x188D071C8`、`MagnifierHost` @ `0x188D071F0`、
`Magnifier` @ `0x188D07210`（均为 UTF-16）。四个 `Mag*` 函数名位于 GetProcAddress 名字表
（文件偏移 `0x8d06128`–`0x8d06178`），**紧邻 `GetMergeOffsetInner`**。

VA `0x183E48E30` 的回调每轮做两次 `memcpy`（一次两条扫描线），源为 32bpp BGRA，
**没有 `GetDIBits`、没有编码解码、没有格式转换**。

全 `.text` 普查：`BitBlt` 仅 5 个调用点、`PrintWindow` 仅 2 个，全部属于打包的 WebRTC
（视频通话/屏幕共享），weshot 一个都不使用。GDI 是降级路径，仅在放大镜不可用或配置项
`longscreenshot_use_grabwindow`（VA `0x1888EA7A0`）为 true 时启用。

`weshot::graphics::window_filter_t` 即 `MagSetWindowFilterList(MW_FILTERMODE_EXCLUDE)` 的封装：
先用 `IsWindow()` 过滤 `SetExcludeHwnds` 传入的句柄，再交给放大镜排除。截图工具自身窗口
因此不会进入画面 —— 不需要在自己的窗口上挖洞。

### 帧交接：单槽覆盖，无队列

生产者写入**单个共享槽**（VA `0x18ADBC838`），无条件覆盖消费者尚未取走的帧，然后递增
VA `0x18ADBC848` 的计数器并 `notify_all`。**没有队列、没有深度上限、生产者从不阻塞**。
旧帧被静默丢弃而非缓冲 —— 即 latest-frame-wins 合并。

消费侧是 `std::condition_variable` 无限等待（互斥量 `0x18ADBC820`、条件变量 `0x18ADBC828`），
deadline 为 `INT64_MAX`，只被 `notify_all` 唤醒，永不因时钟唤醒。


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

### handle_works 的完整控制流

`0x181C3E510` 的主路径逐条确认如下：

| VA | 指令 | 含义 |
|---|---|---|
| `0x181C3E58D` | `callq 0x1804F6F40` / `testb %al,%al` / `je` | 队列为空则走另一分支 |
| `0x181C3E637` | `callq 0x183E47640` | **逐帧去重：新帧与上一帧是否完全相同** |
| `0x181C3E63F` | `jne 0x181C3E83E` | 相同则**整帧丢弃**，不进入 ORB |
| `0x181C3E645` | `incl 0x1c(%rsi)` | 帧序号自增（仅对未被去重的帧） |
| `0x181C3E6E8` | `callq 0x183E48880` → `ebx` | 计算 offset |
| `0x181C3E712` | `callq 0x1804F7340` | baseline = 当前帧（**无条件**） |
| `0x181C3E718` | `testl %ebx,%ebx` / `je 0x181C3E76B` | offset 为 0 → 不拼接 |
| `0x181C3E72B` | `negl` / `cmovsl` | `ecx = abs(offset)` |
| `0x181C3E738` | `mulsd 0x18858AB80` | `frame_height * 0.6` |
| `0x181C3E740` | `ucomisd` / `jae 0x181C3E785` | 超限 → 跳过拼接，走 `0x180872500(dl=1)` |
| `0x181C3E7A4` | `callq 0x1808725E0` | 执行拼接，传入 offset(`r8d`) 与序号(`r9d`) |

注意 `0x181C3E712` 位于 offset 判定**之前**且无分支：baseline 每帧都被替换，
与 offset 是否为 0、是否超限无关。

### 逐帧去重闸门

`0x183E47640` 的结构：

```
0x183E47678  conv(prev, &tmp_a)      ; 0x1804F7490
0x183E4768E  conv(next, &tmp_b)      ; 0x1804F7490
0x183E4769C  eax = compare(a, b)     ; 0x1804CE420
0x183E476A2  ebx = eax               ; 返回值即比较结果
```

`0x1804F7490` 是 `QPixmap::toImage()`：读 `0x18(%rcx)` 取数据指针，
若为空或 `0x14(%rcx)` 标志为 0 走 `0x1804C5AB0`，否则调虚表 `0x88(%rax)`。

`0x1804CE420` 是 `QImage::operator==`，逐条确认：

| VA | 指令 | 含义 |
|---|---|---|
| `0x1804CE430` | `movq 0x18(%rdx),%r12` / `movq 0x18(%rcx),%r13` | 取两侧 `QImageData*` |
| `0x1804CE438` | `cmpq %r13,%r12` / `je` → true | **同一份数据直接判等**（隐式共享） |
| `0x1804CE455` | `cmpl 0x8(%r12), 0x8(%r13)` | 比较高度 |
| `0x1804CE464` | `cmpl 0x4(%r12), 0x4(%r13)` | 比较宽度 |
| `0x1804CE473` | `cmpl 0x30(...)` | 比较 format |
| `0x1804CE482` | `cmpl $0x4,%eax` | format 4 = `Format_RGB32` 走快路径 |
| `0x1804CE4B5` | `movl (%r8,%r11,4),%esi` / `xorl (%rcx,%r11,4),%esi` | 逐像素异或 |
| `0x1804CE4C0` | `testl $0xffffff,%esi` | **只比较低 24 位，屏蔽 alpha** |
| `0x1804CE4C6` | `je` 继续 / 否则 `jmp 0x1804CE5EB` 返回 false | 任一像素不同即为不等 |
| `0x1804CE4D0` | `addq %rdx,%rcx` / `addq %r9,%r8` | 按 `bytesPerLine`(`0x38`) 步进下一行 |

即：**画面完全相同（忽略 alpha）的帧在 ORB 之前就被丢弃**，既不推进帧序号，
也不触碰 baseline、累计位置或画布。

`0x1804CE4E0` 起是非 RGB32 格式的分支：按 `depth`(`0x0C`) 计算每行有效字节数
（`0x1804CE4E2`–`0x1804CE4F2`：`width*depth`，向上取整到 8 再右移 3），
校验两侧 `bytesPerLine` 与之相符后走 `0x186D87AE0` 做整块比较。

### offset 计算的调用边界

`0x181C3E6E8` 调用的 `0x183E48880` 只是一层适配器，真正的匹配在外部 DLL：

- `0x183E48145` 检查全局函数指针 `0x18ADDFCC8`，为空则调 `0x183E48120` 加载。
- `0x183E4816B` 处的立即数 `0x634F746168436557` 即 ASCII `"WeChatOc"`，
  配合 `0x183E48179` 的 `movb $0x72`（`'r'`）拼出 **`WeChatOcr`**。
- `0x183E48ABD` 的 `callq *%r10`（`r10` 取自 `0x18ADDFCC8`）是真正的匹配入口。

适配器把两个 `QPixmap` 转成 `QImage` 后拆成裸参数，逐个确认：

| helper VA | 读取字段 | 含义 |
|---|---|---|
| `0x1804C6BE0` | 经 `0x1804C6A40` 后取 `0x28` | `bits()` 像素指针 |
| `0x1804C70A0` | `0x4(%rax)` | `width()` |
| `0x1804C6B70` | `0x8(%rax)` | `height()` |
| `0x1804C6B60` | `0x38(%rax)` | `bytesPerLine()` |

`0x183E48903`–`0x183E48917` 检查 format 是否落在 `[4, 14)` 且位于掩码
`0x207` 中，是则查表 `0x188D07224` 得到通道数，否则先 `convertToFormat(4)`
（`0x183E48942`，`r8d=4`）再按单通道处理。两幅图各做一次。

**返回值只有一个 int**：`0x183E48AC1` 的 `movl 0xF0(%rbp), %esi` 取回 offset，
`0x183E48AF1` 的 `movl %esi, %eax` 直接返回。没有支持数、没有置信度、
没有第二个输出参数。

因此外层 `handle_works` 拿到 offset 后，除了 `0x181C3E718` 的零值判定和
`0x181C3E740` 的 60% 上限，**没有任何进一步的校验**：不存在对匹配质量的
二次确认，也不存在像素级的对齐复核。offset 的正确性完全由 `wxocr.dll`
内部的匹配逻辑保证。

### wxocr.dll 的定位

`0x188D07198` 处的字符串为 `wxocr.dll`，`0x188D071B0` 为 `GetMergeOffsetInner`，
`0x183E482DA` 处的间接调用即 `GetProcAddress`，结果存入 `0x18ADDFCC8`。

该 DLL 位于
`%APPDATA%\Tencent\xwechat\xplugin\plugins\WeChatOcr\<build>\extracted\wxocr.dll`，
ImageBase `0x180000000`，导出 `GetMergeOffsetInner` 的 RVA 为 `0x22520`。
以下地址均以该模块为准。

### GetMergeOffsetInner 的返回值语义

结果结构体前两个 int 的写入点共四处，值域只有两种：

| VA | 触发条件 | 写入值 |
|---|---|---|
| `0x180022754` | 静态行覆盖整帧（`0x180022738` / `0x18002274D`） | `{0, 0}` |
| `0x18002277B` | 入参校验失败（尺寸 < 3 等） | `{0, 0}` |
| `0x180022919` | descriptors 为空（`Mat::empty()` @ `0x180022901`） | `{0, 0}` |
| `0x180022EA2` | 候选列表为空 | `{0xF423F, 0}` |
| `0x180022EAE` | 正常路径 | `{best_offset, best_support}` |

**`{0, 0}` 表示"无位移"，不是错误。** 四个前置分支写的是同一个值，与"匹配成功但
offset 为 0"不可区分 —— 调用方在 `0x181C3E718` 只做 `testl %ebx,%ebx`，两者走同一条
"不拼接"的路。

只有候选列表为空时才写哨兵 `0xF423F`（999999），而该值必然超过
`0x181C3E740` 的 60% 上限，因此同样不拼接。

也就是说：**特征不足、无一致候选、支持数不够，在微信里都不是失败**，都只是这一帧
不产生位移。模块内不存在"匹配失败"的独立状态。

### 最小支持数阈值（GetMergeOffsetInner）

这是外层缺失的那道防线所在。候选 offset 的直方图建立后，
`0x180022C96`–`0x180022CA8` 逐个筛选候选：

```
0x180022C74  esi  = 0x20(%rax)          ; 得票最高候选的支持数
0x180022A4B  r15d = 0                   ; 参与匹配的关键点计数器，清零
0x180022B44  incl %r15d                 ; 每接受一个匹配点自增
0x180022C01  incl %r15d                 ;   （两条路径）

0x180022C9B  eax = 0x20(%r8)            ; 当前候选的支持数
0x180022C9F  leal (%rax,%rax,4), %eax   ; eax = support * 5
0x180022CA2  leal (%rsi,%rax,4), %eax   ; eax = best_support + support * 20
0x180022CA5  cmpl %r15d, %eax
0x180022CA8  jl   0x180022CD5           ; 小于总点数 -> 丢弃该候选
```

即判据为：

```
best_support + candidate_support * 20 >= total_matched_points
```

支持数不足的候选被整个丢弃，不参与最终 offset 的产生。这正是重复图案
（同一列时间戳、重复的侧栏标签）下防止误匹配的机制：这类页面会产生大量
互相矛盾的对应点，得票最高者也可能只占总数的极小比例，此判据将其否决。

匹配点自身的接受条件在 `0x180022D74`–`0x180022E18`：

| VA | 条件 |
|---|---|
| `0x180022D79` | `distance <= xmm6`（距离上限） |
| `0x180022DDD` | `abs(x1 - x2) <= 4`（水平位移容差） |
| `0x180022E15` | `abs(y1 - y2 - candidate) <= 1`（纵向一致性容差） |

`0x180022DB2`/`0x180022DC7` 等处的 `addsd %xmm7` 是坐标取整前的 `+0.5`。

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

### SpliceWorker 对象布局

| 偏移 | 含义 | 证据 |
|---:|---|---|
| `0x10` | 画布 QPixmap | `0x181C3D326` / `0x181C3D89D` |
| `0x28` | 交换用的临时 pixmap 槽 | `0x181C3D8CE` |
| `0x30` | 首帧宽度 | `0x181C3D33E` |
| `0x34` | 首帧高度 | `0x181C3D34B` |
| `0x38` | **恒为 0**，见下 | 构造时清零，全模块无写入点 |
| `0x3C` | 向下已捕获的最大深度 | `0x181C3D86A` / `0x181C3D93A` |
| `0x40` | 当前帧顶相对首帧顶的位移 | `0x181C3D856` / `0x181C3D85B` |
| `0x48` | QScreen* | `0x181C3C8E5` |

`handle_init_pixmap` = `0x181C3D310`，签名 `(this, QPixmap*)`：存 pixmap 到 `0x10`，
宽高分别写入 `0x30`/`0x34`，**不触碰 `0x38`**。

**`0x38` 恒为 0。** 构造函数 `0x181C3C8CA` 的 `movups %xmm0, 0x30(%rdi)` 与
`0x181C3C8CE` 的 `movq $0x0, 0x3e(%rdi)` 把 `0x30`–`0x45` 全部清零；此后在整个
`0x181C20000`–`0x181C80000` 范围内**没有任何指令写入该偏移**（已用
`movl %exx, 0x38(%rxx)` / `addl` / `movl $imm` 三种形式穷举）。它只被读两次：
`0x181C3D503`（日志）和 `0x181C3D853`（下述算术）。

### 拼接的完整算术

代入 `0x38 = 0` 后语义完全确定：

```
0x181C3D853  ecx = 0x38(rdi)          ; = 0
0x181C3D856  eax = 0x40(rdi)          ; position
0x181C3D859  eax -= ebx               ; position -= offset
0x181C3D85B  0x40(rdi) = position     ; 无条件写回
0x181C3D85E  r14d = 0
0x181C3D861  r14d -= position         ; grow = -position
0x181C3D864  jle -> 0x181C3D92D       ; position >= 0 走向下分支

向上增长（position < 0）：
0x181C3D86A  0x3C(rdi) += grow        ; 最大深度随原点平移
0x181C3D86E  0x40(rdi) = 0            ; ecx 此时为 0，position 归零
0x181C3D8C8  callq 0x183E47A40(...)   ; 见下节，画在顶部

向下增长（0x181C3D92D）：
0x181C3D930  r14d = position - 0x3C(rdi)
0x181C3D934  jle -> 0x181C3D9F4       ; 不为正 -> 画布不变
0x181C3D93A  0x3C(rdi) = position     ; 推进最大深度
0x181C3D994  callq 0x183E47780(...)   ; 画在底部
```

两个计数器都以**首帧顶边**为原点：`0x40` 是当前位置，`0x3C` 是向下到达过的最深处。
向下增长的判据用的是**历史最大深度**而非画布高度 —— 来回滚动经过已捕获区域时
`position - 0x3C <= 0`，不产生任何增长，因此同一段内容不会被写入两次。

向上增长时 `0x40` 归零、`0x3C` 加上 `grow`，等于把坐标原点平移到新的画布顶部，
两个计数器仍描述同一批文档行。


### 两个合成 helper 的方向

绘制目标由 `0x180C0FC50`（`QPainter::drawPixmap(QPointF, QPixmap)`）的第二参数确定：

**`0x183E47A40` —— 向上增长**

```
0x183E47AC2  rect = (0, 0, width-1, crop_h-1)   ; y1 = 0，取帧的【顶部】
0x183E47B4B  esi = new_canvas.height()
0x183E47B57  eax = old_canvas.height()
0x183E47B61  esi -= eax                          ; y = grow
0x183E47B80  drawPixmap(y = grow, old_canvas)    ; 旧画布【下移】grow
0x183E47B9B  drawPixmap(y = 0,    cropped)       ; 裁剪条画在顶部
```

**`0x183E47780` —— 向下增长**

```
0x183E47806  ecx = fh - crop_h
0x183E4780F  rect = (0, fh-crop_h, width-1, fh-1) ; 取帧的【底部】
0x183E478B0  drawPixmap(y = 0, old_canvas)        ; 旧画布保持原位
0x183E478CC  esi = new_h - crop_h
0x183E478EE  drawPixmap(y = new_h - crop_h, cropped) ; 裁剪条画在底部
```

`handle_splice_pimxap` 在 `0x181C3D864` 的 `jle` 不成立（即 `0x38 - 0x40 > 0`）时
调用 `0x183E47A40`，也就是**向上**增长；`jle` 成立时走 `0x181C3D92D` 调用
`0x183E47780`，向下增长。

由此可反推 `0x40` 的语义：它是**帧顶到画布顶的距离**。`0x181C3D859` 的
`subl %ebx, %eax` 每帧把 offset 从中减去，减到小于 0 时向上补，加到超过画布高度
时向下补。

### 拷贝的矩形（0x183E47A40）

参数 `(sret, old_canvas, new_frame, grow, QScreen*)`。第五个参数在两个 helper 中
均未被使用。裁剪矩形：

```
0x183E47A72  edi = new_frame.height()   ; 0x1804F6F60 读 0xc(QPlatformPixmap*)
0x183E47A7E  ebx = new_frame.height()
0x183E47A99  eax = new_frame.width()    ; 0x1804F6F70 读 0x8(...)
0x183E47A9F  ecx = (edi + 3) >> 2       ; frame_height / 4，向上取整
0x183E47AAA  ecx += esi                 ; + grow
0x183E47AB1  edx = ebx / 2              ; frame_height / 2
0x183E47AB7  cmovlel %ecx, %edx         ; edx <= ecx 时取 ecx —— 即 max
0x183E47AC2  rect = (0, 0, width-1, edx-1)
0x183E47AD8  cropped = new_frame.copy(rect)
```

即裁剪高度为：

```
crop_h = max(frame_height / 2, frame_height / 4 + grow)
```

注意 `0x183E47AB7` 的 `cmovlel` 取的是**较大者**：`edx` 不大于 `ecx` 时用 `ecx`。
两个尺寸读取（`0x183E47A72`/`0x183E47A7E`）调用的都是 `0x1804F6F60`（height），
宽度只在 `0x183E47A99` 读一次，仅用于矩形的 `x2`。

裁剪条恒定不小于半帧。这既不是 `grow` 行，也不是整帧。取自帧的哪一端由方向决定，
见上一节。

新画布的构造与绘制：

```
0x183E47B07  dst = QPixmap(old.width(), old.height() + grow)   ; 0x1804F65F0
0x183E47B19  dst.fill(Qt::transparent)                         ; 0x180BFA1D0，索引 0x13
0x183E47B45  painter.begin(dst)                                ; 0x180C05770
0x183E47B80  painter.drawPixmap(QPointF(0, grow), old_canvas)
0x183E47B9B  painter.drawPixmap(QPointF(0, 0),    cropped)
0x183E47BA5  painter.end()
```

**每次拼接都重新分配整块画布**，把旧画布整体重绘进去，再把裁剪条画在一端。
新表面先填透明。由于裁剪条高度（不小于半帧）通常大于 `grow`，两次 `drawPixmap`
的 y 区间**重叠**：裁剪条画在后，覆盖旧画布对应位置的像素。这正是消除拼缝的
机制 —— 最新一次捕获的像素胜出。

### update_preview 的两个调用点

`update_preview`（VA `0x181C3B110`）在整个模块中只有两处调用：

| VA | 时机 |
|---|---|
| `0x181C391B0` | 会话初始化，紧随 `0x181C3AFF0`（模式判定）之后 |
| `0x181C3E18B` | 一次拼接成功之后 |

`0x181C3E18B` 处的前置判断（VA `0x181C3E16B`–`0x181C3E179`）是画布上限检查：
`width * height > 0x8F0D17F` 或 `width >= 0x7530` 时跳转到 `0x181C3E196` 走结束分支，
不再更新预览。也就是说预览更新是**拼接驱动**的，没有独立的刷新定时器 —— 这与
第 7 节"无 `QTimer`/`startTimer`"的普查结论一致。

## 6.1 预览窗口的摆放与缩放

### 会话对象字段

`update_preview` 的 `this`（`rsi`）在偏移 `0x60` 起是一个 `QRect`，按 Qt 的
`QRect{x1,y1,x2,y2}` 布局：

| 偏移 | 含义 |
|---:|---|
| `0x60` | 选区 left |
| `0x64` | 选区 top |
| `0x68` | 选区 right |
| `0x6C` | 选区 bottom |
| `0x100` | 摆放模式（0/1/2），由 `0x181C3AFF0` 写入 |
| `0x88` | 预览窗口对象 |
| `0xB0` | 第二个窗口对象（模式相关，见下） |
| `0xE0` | 画布 pixmap |
| `0x28` | 屏幕对象，`0x14`/`0x1C` 为可用区域的 left/right |

注意 `0x68 - 0x60 + 1` 是选区**宽度**（Qt `QRect::width()` 的闭区间定义），
不是高度；`0x1C(rax) - 0x14(rax)` 取的是屏幕可用区的**水平**范围。整个摆放判定
是水平方向的。

### 摆放模式判定（VA `0x181C3A0FA`）

```
w      = 0x68 - 0x60 + 1                 ; 选区宽度
margin = qRound(w * 0.2) + 24            ; 0.2 @ 0x1886EBF40，+24 @ 0x181C3A18F
avail  = screen.right - screen.left + 1  ; 0x181C3A154/0x181C3A197

if (0x68 + margin <= avail)  -> 模式 0   ; 右侧放得下，不缩放
else if (margin <= 0x60 - 1) -> 模式 1   ; 左侧放得下
else                         -> 模式 2   ; 两侧都放不下，叠在选区上
```

`0x181C3A162`/`0x181C3A16A`/`0x181C3A17C` 一组调用会在存在多屏时用实际屏幕矩形
替换 `avail`，失败时 `eax = 0`（VA `0x181C3A18B`）。

### 缩放系数（VA `0x181C3B110`）

`update_preview` 开头（`0x181C3B138`–`0x181C3B14B`）通过虚表 `0x18(%rax)` 调用
`metric(12)` 并乘以 `0x1885504D0 = 2^-16`。Qt 中 `QPaintDevice::PdmDevicePixelRatioScaled`
正是 12，其返回值以 65536 为定点基数，所以 `xmm6 = devicePixelRatio`。
`0x181C3B197`–`0x181C3B1C5` 是 `qFuzzyIsNull` 断言（`qsize.h`，行 `0xCA`，
阈值 `0x1884E4760 = 1e-12`），确认这是除数。

画布逻辑尺寸由物理像素除以 DPR 得到（`0x181C3B1CD`/`0x181C3B21B`）：

```
ebx = qRound(pixmap.width  / dpr)   ; canvas_w，取自 -0x20(%rbp)
edi = qRound(pixmap.height / dpr)   ; canvas_h，取自 -0x1c(%rbp)
```

寄存器归属由 `0x181C3B266` 的打包顺序确认：`ebx` 进低 32 位、`edi` 进高 32 位后
存入 `-0x10(%rbp)`，而 Qt `QSize` 布局为 `{int wd; int ht;}`，故 `ebx` 是宽、
`edi` 是高。

`qRound` 在此二进制中被内联为
`x >= 0 ? (int)(x + 0.5) : (int)((x - 1.0)) + (int)(frac + 0.5)`，常量
`0x1884EE800 = 0.5`、`0x1884EE808 = -1.0`。

三种模式的缩放系数、高度上限与摆放偏移：

| 模式 | 条件 | 缩放 VA | 缩放系数 |
|---:|---|---|---|
| 0 | 右侧放得下 | `0x181C3B4DB` | `min(⌊(gap_right*10 − 240) / canvas_w⌋ * 0.1, 0.4)` |
| 1 | 左侧放得下 | `0x181C3B2DD` | `min(⌊(sel.left*10 − 250) / canvas_w⌋ * 0.1, 0.4)` |
| 2 | 两侧都放不下 | `0x181C3B349` | `400.0 / sel_width` |

常量：`0x1888E07A0 = 0.1`，`0x188679600 = 0.4`，`0x1888EA570 = 400.0`。
模式 1 的 `left*10 − 250` 来自 `0x181C3B2E0` 的 `leal (%rax,%rax,4)` 与
`0x181C3B2E3` 的 `leal -0xfa(,%rax,2)`；模式 0 的 `gap_right*10 − 240` 同理来自
`0x181C3B4E6`/`0x181C3B4E9`，其中
`gap_right = (screen.right − screen.left + 1) − sel.right`（`0x181C3B4E0`）。

三点值得注意：

1. **除数是 `canvas_w`，不是 `canvas_h`。** 而画布宽度恒等于选区宽度（拼接只在
   纵向增长），因此模式 0/1 的缩放系数在一次会话内**恒定**，画布变长不会让预览
   变小。
2. **`idivl` 是整数除法，且发生在乘 0.1 之前**，所以缩放系数被量化到 0.1 的整数
   倍。结合 0.4 上限，取值只可能是 0.1/0.2/0.3/0.4。
3. 模式判定保证了 `gap >= qRound(sel_w*0.2) + 24`，代入可得系数下界约 0.2，
   因此不会出现缩放为 0 的退化情况。

模式 2 的除数是选区宽度（`0x68 − 0x60 + 1`），`canvas_w * (400 / sel_width)`
在 `canvas_w == sel_width` 时恒为 400 —— 即模式 2 的预览宽度固定 400px。选区窄于
400px 时这是**放大**。

### 高度上限与摆放坐标

缩放后的尺寸写入局部 `QSize`（`0x181C3B3B8` 存宽、`0x181C3B408` 存高），再传给
`0x181C3E360` 做高度约束。该 helper 的参数为 `(this, QSize*, int limit)`：

```
if (size.height > limit)  size = scaledToHeight(limit      * dpr)   ; 0x181C3E37F
else                      size = scaledToWidth (size.width * dpr)   ; 0x181C3E3AC
```

先按 DPR 换算到物理像素设定 pixmap，再把逻辑尺寸除回 DPR 返回
（`0x181C3E457`/`0x181C3E4A6`）。等价于「保持长宽比装入 `宽 × limit` 的框」。

最终位置用 SSE 整数运算一次算出（`psubd`/`paddd`），常量为 16 字节对齐的
`int32[4]`：

| 模式 | limit | limit VA | 位置 VA | 常量 | 结果位置 |
|---:|---|---|---|---|---|
| 0 | `bottom − 12` | `0x181C3B5AA` | `0x181C3B5C0` | `0x1888EA5A0={-12,0}`, `0x18858ABA0={0,1}` | `(right + 12, bottom − h + 1)` |
| 1 | `bottom − 12` | `0x181C3B40B` | `0x181C3B421` | `0x1888EA590={-24,1}` | `(left − w − 24, bottom − h + 1)` |
| 2 | `bottom − top − 99` | `0x181C3B4AE` | `0x181C3B4C0` | `0x1888EA580={-12,-12}` | `(right − w − 12, bottom − h − 12)` |

三者互相自洽：模式 0/1 让预览自选区底边向上生长，limit `bottom − 12` 恰好对应
到达屏幕顶部时留 12px；模式 2 把预览叠在选区右下角内侧 12px 处，limit
`sel_height − 99` 保证选区顶部仍露出约 87px。

### 两个窗口对象的先后顺序

`0x181C3B5E2` 调用返回值与 4 比较（`0x181C3B5EF`），决定 `0x88` 和 `0xB0` 两个
窗口的 resize/move 顺序：

- `== 4`：先 resize `0x88`，再 resize `0xB0`，最后 move `0xB0`（`0x181C3B5F4`–`0x181C3B61F`）
- `!= 4`：先 move `0x88`，再 resize `0x88`（`0x181C3B627`–`0x181C3B641`）

两条路径最后都以 `0x88` 的虚表 `0x58(%rax)` 带 `dl = 1` 收尾（`0x181C3B653`），
即 `setVisible(true)`。

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
| 支持数权重 | 20（`best + support*20 >= total`） |
| 最大位移 | 帧高的 60% |

### 流水线与时序常量

| 项目 | 数值 | VA |
|---|---:|---|
| 放大镜消息泵 sleep | 10ms | `0x183E474B6` |
| 放大镜泵最大轮数 | 9 | `0x183E474BE` |
| 鼠标按下去抖 | 501ms (`0x1F5`) | `0x181C3DB70` |
| 画布单边上限 | 30,000px (`0x7530`) | `0x181C3E110` |
| 画布面积上限 | 149,999,999px (`0x8F0D17F`) | `0x181C3E110` |
| 帧槽容量 | 1（覆盖式，无队列） | `0x18ADBC838` |
| 逐帧去重 | 整帧像素相等即丢弃（忽略 alpha） | `0x181C3E637` → `0x1804CE420` |
| 大位移上限 | 帧高 × 0.6 | `0x18858AB80` / `0x181C3E740` |
| Qt 连接类型 | 0 = AutoConnection（跨线程即 Queued） | 8 处 connect |
| QThread 优先级 | 7 = InheritPriority（即不调整） | — |

`0x181C3DB70` 的签名为 `f(this, bool pressed)`，是**鼠标按键状态回调**，不是滚轮
静止判定：

```
0x181C3DB9B  testb %dl,%dl / je 0x181C3DC3D   ; dl = 是否按下
按下分支：
  0x181C3DBA3  rax = now()
  0x181C3DBB2  jle 0x181C3DCE1                ; 0xD8(rsi)==0 -> 首次按下，仅记录时间
  0x181C3DBB8  rax -= 0xD8(rsi)               ; 距上次按下的间隔
  0x181C3DBBB  cmpq $0x1F5, %rax
  0x181C3DBC1  jl   -> 直接返回                ; 间隔 <501ms，忽略本次按下
  0x181C3DCE1  0xD8(rsi) = now()              ; 否则记录时间戳
松开分支（0x181C3DC3D）：
  释放 0xC8 处对象，0x181C3DCD1 处将 0xD8 清零
```

**静态分析中未发现"滚轮停止后经过固定时间即结束手势"的逻辑。** 结合第 7 节
"无 `QTimer`/`startTimer`"的普查结论，微信的拼接完全由帧驱动，不存在基于时间的
手势结束判定。

### 预览摆放与缩放常量

| 项目 | 数值 | VA |
|---|---:|---|
| 摆放判定边距系数 | 0.2 | `0x1886EBF40` |
| 摆放判定边距基数 | 24 | `0x181C3A18F` |
| 模式 0 分子偏置 | −240 | `0x181C3B4E9` |
| 模式 1 分子偏置 | −250 | `0x181C3B2E3` |
| 模式 0/1 缩放系数 | 0.1 | `0x1888E07A0` |
| 模式 0/1 缩放上限 | 0.4 | `0x188679600` |
| 模式 2 目标宽度 | 400.0 | `0x1888EA570` |
| DPR 定点基数 | 2⁻¹⁶ | `0x1885504D0` |
| Qt metric 编号 | 12 (`PdmDevicePixelRatioScaled`) | `0x181C3B13C` |
| 模式 0 偏移 | `{-12, 0}` / `{0, 1}` | `0x1888EA5A0` / `0x18858ABA0` |
| 模式 1 偏移 | `{-24, 1}` | `0x1888EA590` |
| 模式 2 偏移 | `{-12, -12}` | `0x1888EA580` |
| 模式 0/1 高度上限 | `bottom − 12` | `0x181C3B5AA` / `0x181C3B40B` |
| 模式 2 高度上限 | `bottom − top − 99` | `0x181C3B4AE` |

### 明确的否定结论

以下机制经普查确认**不存在**于长截图模块（VA `0x181C20000`–`0x181C80000`）：

- 抓帧循环内无 `Sleep`/`msleep`/`usleep`/`WaitForSingleObject`
- 无 `QueryPerformanceCounter`/`GetTickCount`/`SetThreadPriority`
- 无 `QTimer`/`startTimer`/`QBasicTimer` —— 预览刷新由拼接成功驱动，不存在固定
  刷新周期
- `GetScrollInfo` **未出现在导入表中** —— 微信不读取滚动条位置
- DXGI Desktop Duplication 代码虽编译进二进制，但 `CreateDXGIFactory`/`D3D11CreateDevice`
  在 `.text` 中**零调用点**，不可达
- Windows.Graphics.Capture 存在但仅属于 WebRTC 的 `WgcCaptureSession`，weshot 不可达


## 8. 当前静态分析未确认的事项

以下内容没有足够的静态证据，不应当作为微信既有规则：

- 根据滚轮速度动态改变位移上限。
- 除 ORB 规则外的额外整图相似度阈值（外层确认没有；`wxocr.dll` 内部除
  支持数阈值外是否还有其他判据尚未穷尽）。
- 队列积压时是否主动覆盖旧帧。
- `SetExcludeHwnds` 抓屏 helper 对每个排除窗口的具体像素修复算法。
- 动态内容、视频和透明窗口的全部特殊处理分支。
- `0x181C3B5E2` 处返回值为 4 的具体含义（该值只影响 `0x88`/`0xB0` 两个窗口的
  resize/move 先后顺序，不影响最终几何）。
- `0xB0` 窗口对象的角色 —— 只在返回值为 4 的分支被 resize/move。
- 预览窗口的背景绘制方式：`update_preview` 全程只做几何计算和 `setVisible`，
  未见任何 `QPalette`/`setStyleSheet`/`fillRect` 调用，背景来源需动态调试确认。

已确认 `GrabWorker` 连续产生工作项，`OpenCVWorker` 按工作序号处理，`SpliceWorker` 只接收需要扩展边界的像素条带；但 Qt 信号连接的所有运行时队列策略仍需动态调试才能完全确定。
