# RAW 胶片滤镜设计

> CameraFTP - A Cross-platform FTP companion for camera photo transfer
> Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
> SPDX-License-Identifier: AGPL-3.0-or-later

## 概述

在 Android 平台上，将 FTP 接收到的 RAW 图片应用胶片模拟滤镜（富士 Film Simulation 风格的 3D LUT），编码为高质量 JPEG，保存到 DCIM/CameraFTP 目录。

核心思路：利用 32³ `.cube` 3D LUT 文件对 RAW 解码后的线性 sRGB 数据进行色彩映射，再施加 sRGB 色调曲线，最终编码输出。

### 目标平台

- **Android**（当前仅 Android，Rust 后端 + Kotlin 桥接）
- **最低构建目标**：与现有项目一致

### 性能目标

- 24MP RAW 文件处理时间：**典型 5–6 秒**，最差 <9 秒
- 内存峰值：<250MB RSS

---

## 胶片模拟原理

### 为什么 RAW 解码后需要色彩风格 LUT

LibRaw 的 `dcraw_process()` 仅执行基础处理（黑电平减除 → 白平衡 → 反马赛克 → 通用色彩矩阵 → gamma），**不应用相机照片风格**。输出为「平、淡、中性」的图像，与相机直出 JPEG 存在显著差距。

3D LUT（Three-Dimensional Lookup Table）将 RGB 输入空间中的每个点映射到新的 RGB 输出值，编码了完整的色彩风格变换（色调曲线、饱和度调整、色相偏移、亮度依赖的色彩映射），能将中性的 RAW 解码输出转换为接近特定胶片模拟的外观。

### 3D LUT 技术参数

| 参数 | 值 |
|------|-----|
| 格式 | Adobe/DaVinci `.cube` |
| 网格精度 | 32³ = 32,768 个映射点 |
| 插值方式 | 三线性插值（trilinear）或四面体插值（tetrahedral） |
| 色彩空间 | sRGB / Display P3 |
| 输入要求 | 线性 sRGB 数据（无 gamma） |
| 单文件大小 | ~500KB–1MB |

### 可用胶片模拟

来源：[abpy/FujifilmCameraProfiles](https://github.com/abpy/FujifilmCameraProfiles)（MIT License）

| 名称 | 风格描述 |
|------|---------|
| Provia | 标准色彩，自然还原 |
| Velvia | 高饱和，适合风光 |
| Astia | 柔和色彩，适合人像 |
| Classic Chrome | 低饱和、暗调偏暖 |
| Pro Neg Std | 专业负片标准 |
| Pro Neg Hi | 专业负片高对比 |
| Eterna | 电影色彩，低对比 |
| Reala Ace | 真实色彩还原 |
| Classic Neg | 经典负片，复古色调 |
| Nostalgic Neg | 怀旧负片，暖调 |
| Bleach Bypass | 漂白旁路，去饱和高对比 |

---

## 架构总览

```
                         ┌─────────────────────────────────┐
                         │        RAW 文件 (FTP 接收)        │
                         └───────────┬─────────────────────┘
                                     │
                          ┌──────────▼──────────┐
                          │    RAW 类型检测       │  扩展名/魔数判断
                          │ (非RAW → 跳过)       │
                          └──────────┬──────────┘
                                     │
                          ┌──────────▼──────────┐
                          │  检查 FilmFilterConfig │
                          │  enabled + autoFilter │
                          │  → 选择目标滤镜       │
                          └──────────┬──────────┘
                                     │
               ┌─────────────────────▼─────────────────────┐
               │          共享 RAW 解码器                     │
               │  RawDecoder::decode(RawDecodeConfig::       │
               │                      film_filter())         │
               │  → 线性 sRGB f32 数据                       │
               └─────────────────────┬─────────────────────┘
                                     │
                          ┌──────────▼──────────┐
                          │   3D LUT 应用        │
                          │   LutEngine::apply() │
                          │   rayon 并行         │
                          └──────────┬──────────┘
                                     │
                          ┌──────────▼──────────┐
                          │   sRGB 色调曲线       │
                          │   linear → sRGB      │
                          │   f32 → u8 量化      │
                          └──────────┬──────────┘
                                     │
                          ┌──────────▼──────────┐
                          │  高质量 JPEG 编码     │
                          │  quality = 95        │
                          └──────────┬──────────┘
                                     │
                          ┌──────────▼──────────┐
                          │  EXIF 注入           │
                          │  从原 RAW 提取元数据  │
                          └──────────┬──────────┘
                                     │
               ┌─────────────────────▼─────────────────────┐
               │     MediaStoreBridge.saveToCameraFtp()     │
               │     DCIM/CameraFTP/{name}_{filter}_{dt}.jpg│
               └───────────────────────────────────────────┘
```

---

## 开源库与原生函数职责清单

### Rust 侧（Tauri 后端）

| 组件 | 库/API | 版本 | 许可证 | 具体职责 |
|------|--------|------|--------|---------|
| RAW 解码器 | **rsraw** (LibRaw Rust wrapper) | 0.1.1 | MIT | 封装 LibRaw，执行 `unpack()` + `dcraw_process()`，输出线性 sRGB f32 数据 |
| 3D LUT 引擎 | **oximedia-lut** | 0.1.1 | MIT | ① 解析 `.cube` 文件（32³ 网格）<br>② 三线性/四面体插值<br>③ 支持 sRGB / Display P3 色彩空间 |
| 并行计算 | **rayon** | latest | MIT/Apache-2.0 | 逐像素 LUT 应用并行化（24M 像素自动分片） |
| sRGB 色调曲线 | 自实现 (Rust) | — | AGPL-3.0 | 分段 sRGB gamma 函数：线性段 + 幂函数段，f32 → u8 |
| JPEG 编码 | **image** crate | 0.25 | MIT | 高质量 JPEG 编码（quality=95），24MP RGBA → JPEG |
| EXIF 解析与注入 | **nom-exif** | 2.7 | MIT | 从原 RAW 提取 EXIF（ISO、光圈、快门、拍摄时间、GPS 等） |
| 文件命名 | 自实现 (Rust) | — | AGPL-3.0 | 从 EXIF DateTimeOriginal 提取时间，格式化为 yyyyMMdd_HHmmss |

### Kotlin 侧（Android Bridge）

| 组件 | API | 具体职责 |
|------|-----|---------|
| MediaStore 写入 | **MediaStoreBridge.kt**（现有） | 通过 ContentResolver 将 JPEG 写入 `DCIM/CameraFTP/` 目录 |
| 文件扫描 | **MediaScannerConnection** | 通知 Android MediaStore 更新索引 |

### LUT 资源

| 来源 | 格式 | 数量 | 总大小 |
|------|------|------|--------|
| abpy/FujifilmCameraProfiles | `.cube` (sRGB) | 11 个 | ~5–10MB |
| abpy/FujifilmCameraProfiles | `.cube` (Display P3) | 11 个 | ~5–10MB |

仅捆绑 sRGB 变体（~5–10MB），放置于 Android `assets/luts/` 目录，首次使用时解压到内部存储。

---

## 数据链路（字节级详细流程）

### 步骤 1：RAW 类型检测与配置检查

```
输入: FTP PUT 事件的文件路径
处理:
  1. 检查扩展名 ∈ RAW_EXTENSIONS
  2. 检查 FilmFilterConfig.enabled == true
  3. 检查 FilmFilterConfig.autoFilter == true（自动模式）
     或 手动触发指定文件
  4. 确定目标滤镜名: auto_filter_name 或用户选择
输出: 进入处理队列
```

### 步骤 2：RAW 解码为线性 sRGB

```
调用: RawDecoder::open(path) → decode(RawDecodeConfig::film_filter())
参数:
  output_color = 1          // sRGB 色彩空间基色
  gamma = (1.0, 1.0)        // 线性输出（无 gamma）
  no_auto_bright = true     // 禁用自动亮度
  interpolation = 3         // AHD 反马赛克
输出: DecodedRaw { pixels: Vec<f32>, width, height, ... }
耗时: ~1.5-3.0s
说明: 输出为线性 gamma、sRGB 基色空间的 f32 像素数据，是 3D LUT 的正确输入格式。
```

### 步骤 3：3D LUT 应用

```
调用: LutEngine::load("assets/luts/{filter_name}.cube")
      LutEngine::apply(&mut pixels)

处理:
  1. 加载 .cube 文件，构建 32³ 三维网格
  2. rayon 并行遍历所有像素：
     for each pixel (R, G, B) in pixels:
       // 归一化到 [0, 1]
       r = clamp(R / max_value, 0.0, 1.0)
       g = clamp(G / max_value, 0.0, 1.0)
       b = clamp(B / max_value, 0.0, 1.0)
       // 三线性插值查找
       (r', g', b') = trilinear_interpolate(lut_grid, r, g, b)
       // 写回
       pixel.R = r' * max_value
       pixel.G = g' * max_value
       pixel.B = b' * max_value

输出: lut_applied_pixels: Vec<f32>  // 线性 sRGB，已应用胶片色彩风格
耗时: ~0.3-0.8s (24MP, rayon 并行)
```

### 步骤 4：sRGB 色调曲线应用 + 量化

```
处理:
  for each pixel channel v:
    // sRGB 分段 gamma 函数
    if v <= 0.0031308:
      sdr = 12.92 * v
    else:
      sdr = 1.055 * pow(v, 1.0/2.4) - 0.055
    // 量化为 8-bit
    output_byte = clamp(round(sdr * 255), 0, 255)

输出: RgbaImage (8-bit sRGB RGBA)
耗时: ~0.1-0.3s
```

### 步骤 5：高质量 JPEG 编码

```
调用: image::codecs::jpeg::JpegEncoder::encode(image, quality=95)
输出: jpeg_bytes: Vec<u8>  // ~15-30MB (24MP, q=95)
耗时: ~1.5-4.0s (image crate, 无 NEON)
```

### 步骤 6：EXIF 注入

```
处理:
  1. nom-exif 从原 RAW 提取 EXIF 元数据：
     - DateTimeOriginal (拍摄时间)
     - Make/Model (相机型号)
     - ExposureTime / FNumber / ISO (曝光参数)
     - FocalLength (焦距)
     - GPS (如可用)
     - Orientation (旋转方向)
  2. 构造 EXIF APP1 段
  3. 插入到 JPEG 字节流的 FF D8 之后
输出: 包含完整 EXIF 的 JPEG 字节流
耗时: ~0.05-0.1s
```

### 步骤 7：MediaStore 写入

```
调用: MediaStoreBridge.saveToCameraFtp(jpeg_bytes, filename)
路径: DCIM/CameraFTP/{原文件名去掉扩展名}_{滤镜名}_{yyyyMMdd_HHmmss}.jpg
MIME: image/jpeg
耗时: ~0.2-0.5s
```

### 步骤 8：可选删除原 RAW

```
条件: config.filmFilter.autoDeleteRaw == true 且转换成功
调用: MediaStoreBridge 删除原 RAW 条目
```

---

## 文件命名规则

格式：`{原文件名去掉扩展名}_{滤镜名}_{yyyyMMdd_HHmmss}.jpg`

datetime 取自 EXIF `DateTimeOriginal`，若不可得则使用文件修改时间。

| 原始 RAW 文件 | 滤镜 | 输出文件 |
|--------------|------|---------|
| `DSC_0024.NEF` | Classic Chrome | `DSC_0024_ClassicChrome_20260312_091500.jpg` |
| `IMG_0001.CR3` | Provia | `IMG_0001_Provia_20260418_143025.jpg` |
| `_DSC0001.ARW` | Velvia | `_DSC0001_Velvia_20260405_185632.jpg` |

存储路径：**DCIM/CameraFTP/**（通过 MediaStoreBridge 写入）。

---

## 性能预估

**测试基准：** 24MP RAW（6000×4000，如 Sony A7 III），中端设备 Snapdragon 778G (Cortex-A78)

| 阶段 | 耗时 | 备注 |
|------|------|------|
| RAW 解码（共享） | 1.5–3.0s | LibRaw AHD + 线性 sRGB 输出 |
| 3D LUT 应用 | 0.3–0.8s | oximedia-lut + rayon 并行 |
| sRGB 色调曲线 + 量化 | 0.1–0.3s | 24M 像素 |
| 高质量 JPEG 编码 | 1.5–4.0s | image crate，q=95，24MP，无 NEON |
| EXIF 注入 | 0.05–0.1s | |
| MediaStore 写入 | 0.2–0.5s | |
| **总计** | **3.65–8.7s** | **典型 5–6s** |

### 内存峰值分析

| 缓冲区 | 大小 | 存活时段 |
|--------|------|----------|
| RAW 解码 f32 RGBA | ~96MB | 步骤 2–3 |
| LUT 应用后 f32 RGBA | ~96MB | 步骤 3–4（可原地覆盖步骤 2 的缓冲区） |
| 8-bit RGBA Image | ~24MB | 步骤 4–5 |
| JPEG 编码输出 | ~15–30MB | 步骤 5–7 |
| LUT 网格 (32³) | ~0.5MB | 常驻 |
| **峰值 RSS** | **~150MB** | 步骤 2–3 期间（原地覆盖优化后） |

优化：LUT 应用步骤可直接在原像素缓冲区上原地修改（in-place），避免双倍内存占用。

---

## 配置项

在现有 `AppConfig` 中新增字段：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FilmFilterConfig {
    /// 总开关
    pub enabled: bool,
    /// FTP 接收后自动应用滤镜
    pub auto_filter: bool,
    /// 自动应用时使用的滤镜名称（None = 不自动应用）
    pub auto_filter_name: Option<String>,
    /// JPEG 编码质量 (0-100)
    pub jpeg_quality: u8,
    /// 转换后自动删除原 RAW 文件
    pub auto_delete_raw: bool,
}

impl Default for FilmFilterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_filter: false,
            auto_filter_name: None,
            jpeg_quality: 95,
            auto_delete_raw: false,
        }
    }
}
```

### 可用滤镜名称枚举

```rust
pub const AVAILABLE_FILTERS: &[(&str, &str)] = &[
    ("Provia",        "标准色彩，自然还原"),
    ("Velvia",        "高饱和，适合风光"),
    ("Astia",         "柔和色彩，适合人像"),
    ("ClassicChrome", "低饱和、暗调偏暖"),
    ("ProNegStd",     "专业负片标准"),
    ("ProNegHi",      "专业负片高对比"),
    ("Eterna",        "电影色彩，低对比"),
    ("RealaAce",      "真实色彩还原"),
    ("ClassicNeg",    "经典负片，复古色调"),
    ("NostalgicNeg",  "怀旧负片，暖调"),
    ("BleachBypass",  "漂白旁路，去饱和高对比"),
];
```

---

## 与现有架构的集成点

| 集成点 | 现有模块 | 扩展方式 |
|--------|---------|---------|
| FTP 接收触发 | `ftp/listeners.rs` FtpDataListener | 在 PUT 事件处理中增加胶片滤镜自动入队逻辑（与 Ultra HDR 共享 RAW 类型检测） |
| 转换服务 | 新建 `src-tauri/src/film_filter/` | 参考 `ai_edit/service.rs` 的双通道队列架构（manual + auto） |
| 配置管理 | `config.rs` AppConfig | 在 AppConfig 中增加 `film_filter: FilmFilterConfig` 字段 |
| MediaStore 写入 | **MediaStoreBridge.kt**（现有） | 复用现有 `saveToCameraFtp()` 方法 |
| 前端配置 UI | 新建组件 | 在 Config tab 中增加胶片滤镜配置面板 |
| Gallery 索引 | `file_index/service.rs` | 转换完成后将新文件加入索引 |
| Tauri 命令注册 | `src-tauri/src/lib.rs` | 注册新的 Tauri 命令 |
| LUT 资源管理 | Android `assets/luts/` | 首次使用时从 assets 解压到内部存储目录 |

### 新增模块结构

```
src-tauri/src/film_filter/
├── mod.rs                  # 公共导出
├── config.rs               # FilmFilterConfig 结构体
├── service.rs              # 转换服务（双通道队列）
├── processor.rs            # RAW 解码 + LUT 应用 + JPEG 编码管线
├── lut_engine.rs           # .cube 文件加载 + 三线性插值 + rayon 并行
├── tone_curve.rs           # sRGB gamma 分段函数
├── exif_inject.rs          # EXIF 提取 + JPEG APP1 段注入
└── types.rs                # FilmFilterName, FilterResult 等数据类型

assets/luts/                # 3D LUT 资源文件（Android 打包时包含）
├── Provia.cube
├── Velvia.cube
├── Astia.cube
├── ClassicChrome.cube
├── ProNegStd.cube
├── ProNegHi.cube
├── Eterna.cube
├── RealaAce.cube
├── ClassicNeg.cube
├── NostalgicNeg.cube
└── BleachBypass.cube

src/components/
└── FilmFilterConfigCard.tsx  # 配置面板 UI（滤镜选择 + 质量滑块）
```

---

## 与 Ultra HDR 的共享关系

### 共享组件

| 组件 | Ultra HDR 使用方式 | 胶片滤镜使用方式 |
|------|-------------------|----------------|
| `RawDecoder::decode()` | `RawDecodeConfig::ultra_hdr()` → 线性 sRGB 用于增益图 | `RawDecodeConfig::film_filter()` → 线性 sRGB 用于 LUT |
| `RawDecoder::extract_preview()` | 提取内嵌 JPEG 作为 SDR 基底 | 不使用 |
| `nom-exif` EXIF 解析 | 提取拍摄时间用于文件命名 | 提取完整 EXIF 用于注入输出 JPEG |
| `image` crate JPEG 编码 | 编码小尺寸增益图 | 编码全尺寸高质量 JPEG |
| MediaStoreBridge | 写入原目录 | 写入 DCIM/CameraFTP |

### 预留：一次解码两次消费

当前两项功能独立运行，不涉及同一文件同时触发两项处理。架构上预留 `RawDecoder` 作为共享入口：

```
未来（预留，当前不实现）:
  RawDecoder::decode() → DecodedRaw
    ├── ultra_hdr::process(decoded)    → Ultra HDR JPEG
    └── film_filter::process(decoded)  → 胶片风格 JPEG
```

`RawDecodeConfig` 的 `output_color` 和 `gamma` 参数可满足两项功能的差异化需求。

---

## 前端命令清单

| Tauri 命令 | 用途 |
|-----------|------|
| `load_film_filter_config` | 加载胶片滤镜配置 |
| `save_film_filter_config` | 保存胶片滤镜配置 |
| `list_film_filters` | 列出可用滤镜（名称 + 描述） |
| `trigger_film_filter` | 手动触发单文件滤镜应用（含结果回调） |
| `enqueue_film_filter` | 批量入队（多文件 + 滤镜选择，fire-and-forget） |
| `cancel_film_filter` | 取消进行中的处理 |

---

## 事件流

```
FilmFilterProgressEvent:
  - Queued    { file_name: String, filter_name: String, position: usize }
  - Progress  { file_name: String, stage: String, percent: u8 }
  - Completed { file_name: String, output_path: String, filter_name: String, duration_ms: u64 }
  - Failed    { file_name: String, error: String }
  - Done      { processed: usize, failed: usize }
```

---

## 降级策略

| 条件 | 降级行为 |
|------|---------|
| LUT 文件未找到 | 记录错误，跳过该滤镜，通知用户重新下载资源 |
| RAW 解码失败 | 记录错误日志，跳过该文件，不删除原 RAW |
| 处理超时（>30s） | CancellationToken 取消，保留原 RAW |
| 存储空间不足 | 跳过处理，通过事件通知前端 |
| EXIF 不可用 | 文件名中使用文件修改时间替代拍摄时间 |
| oximedia-lut 不可用 | 降级到 wagahai-lut（备选 LUT crate） |

---

## 已排除的方案

| 方案 | 排除原因 |
|------|---------|
| DCP Profile 直接解析应用 | DCP 的 HueSatMap/LookTable 操作在 HSV 空间，需要实现完整的 HSV 转换和插值管线，复杂度远高于 3D LUT |
| HaldCLUT 格式 | 解析更简单但查找效率低于 .cube 的直接网格访问 |
| Android RenderScript / Vulkan Compute 应用 LUT | LUT 应用本身已足够快（0.3-0.8s），GPU 调度开销不划算 |
| 在 Kotlin 侧应用 LUT | 像素数据在 Rust 侧，传到 Kotlin 需要 JNI 大 buffer 传输，得不偿失 |
| image crate JPEG 编码器替换 | turbojpeg / libjpeg-turbo 需 NDK 编译 C 代码，当前性能可接受，未来可考虑 |
