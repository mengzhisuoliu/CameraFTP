# Android 高性能图片查看器设计文档

**日期**: 2026-03-19
**版本**: 4.0
**状态**: 设计修订中

---

## 1. 目标

为 Android 平台设计一个高性能图片查看器，支持 ≥100MP 图片预览，保证 ≥60FPS 流畅度和 **高质量无极缩放**（1x ~ 5x 任意比例）。

### 1.1 支持的图片格式

| 格式 | 支持状态 | 说明 |
|------|----------|------|
| JPEG | ✅ 完全支持 | 主要目标格式，相机照片 |
| PNG | ✅ 完全支持 | 截图、编辑图片 |
| HEIF/HEIC | ❌ 暂不支持 | 移除支持，避免 libvips 依赖 |

> **设计决策**: 移除 HEIF 支持，使用纯 Rust 库替代 libvips，简化 Android 跨平台编译和依赖管理。

---

## 2. 技术方案分析

### 2.1 图像库选型：image-rs vs zune-image

经过调研对比，选择 **`image` crate (v0.25+)** 作为主要图像处理库：

| 维度 | image-rs (v0.25+) | zune-image |
|------|-------------------|------------|
| **JPEG 解码** | ⭐⭐⭐⭐⭐ 内部使用 zune-jpeg | ⭐⭐⭐⭐⭐ zune-jpeg 原生 |
| **PNG 解码** | ⭐⭐⭐ 较慢 | ⭐⭐⭐⭐⭐ 快 3x |
| **格式支持** | JPEG, PNG, GIF, WebP, TIFF, BMP... | JPEG, PNG, PPM, QOI, PSD, HDR |
| **成熟度** | ⭐⭐⭐⭐⭐ 广泛使用，生态完善 | ⭐⭐⭐ 较新 |
| **文档** | ⭐⭐⭐⭐⭐ 完善 | ⭐⭐⭐ 基础 |
| **内存保护** | ✅ `set_memory_limit()` | ❌ 无限制 |

**选择理由**：
1. **JPEG 性能相同** - image-rs v0.25 内部使用 zune-jpeg，性能与原生相当
2. **成熟生态系统** - 更好的错误处理、文档、社区支持
3. **内存保护** - 内置 `set_memory_limit()` 防止 OOM
4. **格式兼容性** - 与现有 nom-exif 等库集成更好

### 2.2 高质量无极缩放方案

**核心问题**：传统金字塔方案在高倍放大时质量下降

```
传统金字塔方案的缩放质量：
────────────────────────────
用户缩放 2x   → 从 Level 0 放大 2x  → ⭐⭐⭐ 轻微模糊
用户缩放 3x   → 从 Level 0 放大 3x  → ⭐⭐ 像素化可见
用户缩放 5x   → 从 Level 0 放大 5x  → ❌ 严重像素化
```

**解决方案**：保留原图 + 动态 Lanczos3 重采样

```
┌─────────────────────────────────────────────────────────────────┐
│                  高质量无极缩放方案                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  内存布局：                                                       │
│  ─────────                                                      │
│  CPU 内存                                                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ 原图 (Level 0): 10000x10000 (300MB)                      │    │
│  │                 ↑ 始终保留，用于高质量渲染                 │    │
│  │ 缩略图 (Level 2): 2500x2500 (19MB)                       │    │
│  │                   用于快速预览和低缩放                     │    │
│  └─────────────────────────────────────────────────────────┘    │
│  总计：~320MB（比释放原图方案多 ~190MB，但换来高质量无极缩放）       │
│                                                                 │
│  渲染流程（任意缩放级别）：                                        │
│  ──────────────────────                                         │
│  用户缩放到 2.3x                                                  │
│       ↓                                                          │
│  选择源图像：                                                     │
│    - scale ≤ 0.5 → 使用缩略图（快速）                             │
│    - scale > 0.5 → 使用原图（高质量）                             │
│       ↓                                                          │
│  计算视口在源图像中的区域（如 4350x4350 像素）                      │
│       ↓                                                          │
│  从源图像裁剪该区域                                               │
│       ↓                                                          │
│  使用 Lanczos3 高质量重采样到屏幕分辨率                            │
│       ↓                                                          │
│  上传到 GPU 并渲染                                                │
│                                                                 │
│  质量保证：⭐⭐⭐⭐⭐ 任意缩放级别都保持最高质量                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 2.3 图片加载与渲染流程

```
┌─────────────────────────────────────────────────────────────────┐
│                    图片加载与渲染流程                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Step 1: 快速元数据提取                                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ image::ImageReader::open() + with_guessed_format()      │   │
│  │ → 获取: width, height, format                           │   │
│  │ → 时间: < 10ms                                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           ↓                                     │
│  Step 2: 完整解码原图                                            │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ reader.no_limits().decode() → DynamicImage              │   │
│  │ → 内存: width × height × 3 (RGB) 或 ×4 (RGBA)           │   │
│  │ → 100MP RGB ≈ 300MB                                     │   │
│  │ → 注意：必须调用 no_limits() 移除 512MB 默认限制          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           ↓                                     │
│  Step 3: 生成缩略图（用于快速预览）                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ fast_image_resize::Resizer::resize()                    │   │
│  │ → 目标: max(width, height) = 2500  (1/4 原图)            │   │
│  │ → 内存: ~19MB (100MP 缩略图)                             │   │
│  │ → 滤镜: Lanczos3 高质量                                  │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           ↓                                     │
│  Step 4: 渲染准备完成                                            │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ 保留: 原图 + 缩略图                                      │   │
│  │ 总内存: ~320MB (100MP)                                   │   │
│  │ 准备就绪，等待用户交互                                    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 2.4 动态策略选择

根据图片尺寸动态选择渲染策略，平衡内存占用和渲染质量：

```
┌─────────────────────────────────────────────────────────────────┐
│                    动态策略选择流程                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  图片加载后，根据尺寸选择策略：                                    │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    尺寸判断                              │    │
│  │                                                         │    │
│  │   小图 (< 10MP)          中图 (10-50MP)      大图 (> 50MP)│    │
│  │   例: 3000x3000         例: 7000x7000      例: 10000x10000│    │
│  │        ↓                      ↓                     ↓    │    │
│  │   ┌─────────┐           ┌─────────┐          ┌─────────┐  │    │
│  │   │ 直接模式 │           │ 轻量模式 │          │ 完整模式 │  │    │
│  │   └─────────┘           └─────────┘          └─────────┘  │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 2.5 三种渲染模式

#### 模式 1：直接模式（小图 < 10MP）

```
┌─────────────────────────────────────────────────────────────────┐
│  直接模式（Direct Mode）                                         │
│  适用: < 10MP（如 3000x3000 ≈ 9MP）                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  特点：                                                          │
│  - 原图直接上传到 GPU 纹理                                        │
│  - 无需 CPU 端重采样，GPU 硬件缩放                               │
│  - 内存占用最小                                                  │
│                                                                 │
│  内存：                                                          │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ CPU: ~27MB (3000x3000x3)                                │    │
│  │ GPU: ~27MB (完整纹理)                                    │    │
│  │ 总计: ~54MB                                              │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  渲染：GPU 硬件双线性/双三次插值                                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

#### 模式 2：轻量模式（中图 10-50MP）

```
┌─────────────────────────────────────────────────────────────────┐
│  轻量模式（Lightweight Mode）                                    │
│  适用: 10MP - 50MP（如 7000x7000 ≈ 49MP）                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  特点：                                                          │
│  - 保留原图在 CPU                                                │
│  - 按需裁剪视口区域 + CPU Lanczos3 重采样                        │
│  - 无缩略图（省内存）                                            │
│                                                                 │
│  内存：                                                          │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ CPU: ~147MB (7000x7000x3)                               │    │
│  │ GPU: ~12MB (视口纹理 1080x1920x4)                        │    │
│  │ 总计: ~160MB                                             │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  渲染：CPU Lanczos3 重采样 + GPU 纹理上传                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

#### 模式 3：完整模式（大图 > 50MP）

```
┌─────────────────────────────────────────────────────────────────┐
│  完整模式（Full Mode）                                           │
│  适用: > 50MP（如 10000x10000 = 100MP）                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  特点：                                                          │
│  - 保留原图 + 缩略图                                             │
│  - 低缩放用缩略图（快速），高缩放用原图（高质量）                   │
│  - 最佳无极缩放体验                                              │
│                                                                 │
│  内存：                                                          │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ CPU: ~300MB (原图) + ~19MB (缩略图) = ~320MB             │    │
│  │ GPU: ~12MB (视口纹理)                                    │    │
│  │ 总计: ~332MB                                              │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  渲染：智能选择源图 + CPU Lanczos3 重采样                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 2.6 模式选择阈值

| 图片尺寸 | 像素数 | 模式 | CPU 内存 | GPU 内存 | 总计 |
|----------|--------|------|----------|----------|------|
| < 10MP | < 10,000,000 | 直接模式 | ~30MB | ~30MB | ~60MB |
| 10-50MP | 10M - 50M | 轻量模式 | ~150MB | ~12MB | ~165MB |
| > 50MP | > 50,000,000 | 完整模式 | ~320MB | ~12MB | ~332MB |

### 2.7 内存管理策略

| 场景 | 策略 | 预期内存 |
|------|------|----------|
| 5MP 图片 (2500x2000) | 直接模式 | ~15MB + ~15MB = ~30MB |
| 24MP 图片 (6000x4000) | 轻量模式 | ~72MB + ~12MB = ~84MB |
| 45MP 图片 (8256x5504) | 轻量模式 | ~136MB + ~12MB = ~148MB |
| 100MP 图片 (10000x10000) | 完整模式 | ~320MB + ~12MB = ~332MB |
| 切换图片 | 先释放前一张图片 | 始终只有一张图片在内存 |

**优化效果**：

| 方案 | 5MP | 24MP | 100MP |
|------|-----|------|-------|
| 原方案（统一完整模式） | ~50MB | ~150MB | ~320MB |
| **动态策略** | **~30MB** | **~84MB** | **~332MB** |
| 节省 | 40% | 44% | - |

---

## 3. 架构设计

### 3.1 整体架构

采用"纯原生 GPU 渲染"方案：
- **Frontend**: 仅负责 UI 控件（EXIF 显示、按钮、导航指示器）
- **Backend**: 使用 wgpu 直接渲染到 Android SurfaceView
- **手势处理**: 完全在原生层实现，避免 WebView 性能瓶颈

```
┌─────────────────────────────────────────────────────────────────┐
│                        表现层 (Frontend)                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  React + TypeScript                                     │   │
│  │  - ImageViewerOverlay（EXIF 信息、导航指示器、操作按钮）     │   │
│  │  - 状态同步（通过 Tauri IPC 获取当前图片索引、缩放级别）       │   │
│  └─────────────────────────────────────────────────────────┘   │
│                          ↕ Tauri IPC                           │
├─────────────────────────────────────────────────────────────────┤
│                        调度层 (Bridge)                          │
│  ├─ Tauri Commands: open_image_viewer, close_image_viewer      │
│  └─ Tauri Events: viewer-state-update (图片切换、缩放变化)       │
├─────────────────────────────────────────────────────────────────┤
│                        核心层 (Backend)                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Rust Engine                                            │   │
│  │  ├─ ImageLoader（图像解码、缩略图生成）                   │   │
│  │  ├─ ViewportRenderer（视口裁剪、动态重采样）              │   │
│  │  ├─ GpuRenderer（wgpu GPU 渲染）                        │   │
│  │  ├─ GestureHandler（原生手势识别）                       │   │
│  │  └─ AndroidBridge（JNI 桥接、SurfaceView 管理）          │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

**架构决策理由**:
- OpenSeadragon 与 wgpu GPU 渲染是互斥方案，无法协同工作
- 纯原生 GPU 方案可获得最佳性能和 100MP+ 支持
- Frontend 仅作为 UI 覆盖层，不参与渲染流程
- **简化瓦片管理**：直接从原图/缩略图裁剪视口区域，无需复杂瓦片缓存

---

## 4. 模块边界与接口

### 4.1 Rust 后端模块

| 模块 | 文件路径 | 职责 | 对外接口 |
|------|----------|------|----------|
| `image_loader` | `src-tauri/src/image_viewer/loader.rs` | 图像解码、缩略图生成 | `ImageLoader::load(path) -> Result<LoadedImage, ImageError>` |
| `viewport_renderer` | `src-tauri/src/image_viewer/viewport.rs` | 视口裁剪、动态重采样 | `ViewportRenderer::render(viewport) -> Result<ViewportTexture, RenderError>` |
| `gpu_renderer` | `src-tauri/src/image_viewer/gpu.rs` | GPU 纹理管理、渲染 | `GpuRenderer::render(texture) -> Result<RenderStats, RenderError>` |
| `gesture_handler` | `src-tauri/src/image_viewer/gesture.rs` | 手势识别、视口计算 | `on_touch_event(event) -> GestureAction` |
| `android_surface` | `src-tauri/src/image_viewer/android.rs` | SurfaceView 生命周期 | `attach(window) -> Result<ANativeWindow, SurfaceError>` |
| `viewer_bridge` | `src-tauri/src/image_viewer/bridge.rs` | Tauri 命令、事件 | `open_image_viewer(args) / emit_state_update(state)` |

#### 4.1.1 完整接口定义

```rust
use image::{DynamicImage, ImageReader, RgbImage};
use fast_image_resize::{Resizer, Image as ResizeImage, FilterType, ResizeOptions, PixelType};
use std::sync::Arc;

/// 渲染模式（根据图片尺寸动态选择）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderMode {
    /// 直接模式：小图（< 10MP），原图直接上传 GPU
    Direct,
    /// 轻量模式：中图（10-50MP），仅保留原图
    Lightweight,
    /// 完整模式：大图（> 50MP），保留原图 + 缩略图
    Full,
}

impl RenderMode {
    /// 根据图片尺寸选择渲染模式
    pub fn from_dimensions(width: u32, height: u32) -> Self {
        let megapixels = (width as u64 * height as u64) as f32 / 1_000_000.0;

        if megapixels < 10.0 {
            RenderMode::Direct
        } else if megapixels < 50.0 {
            RenderMode::Lightweight
        } else {
            RenderMode::Full
        }
    }
}

/// 加载后的图像数据
pub struct LoadedImage {
    /// 渲染模式
    pub mode: RenderMode,
    /// 原始图像
    pub original: Arc<RgbImage>,
    /// 缩略图（仅完整模式）
    pub thumbnail: Option<Arc<RgbImage>>,
    /// 元数据
    pub exif: Option<ExifData>,
    /// EXIF 方向
    pub orientation: ExifOrientation,
}

/// 图像加载器
pub struct ImageLoader;

impl ImageLoader {
    /// 加载图片，根据尺寸自动选择模式
    pub fn load(path: &Path) -> Result<LoadedImage, ImageError> {
        // Step 1: 解码图片
        let reader = ImageReader::open(path)?
            .with_guessed_format()?;

        let img = reader.no_limits().decode()?;
        let rgb_image = img.to_rgb8();
        let (width, height) = rgb_image.dimensions();

        // Step 2: 确定渲染模式
        let mode = RenderMode::from_dimensions(width, height);

        // Step 3: 根据模式生成缩略图
        let thumbnail = match mode {
            RenderMode::Full => {
                Some(Arc::new(Self::generate_thumbnail(&rgb_image, width, height)?))
            }
            _ => None,
        };

        Ok(LoadedImage {
            mode,
            original: Arc::new(rgb_image),
            thumbnail,
            exif: None,
            orientation: ExifOrientation::Normal,
        })
    }

    /// 生成缩略图（max dimension = 2500）
    fn generate_thumbnail(src: &RgbImage, src_w: u32, src_h: u32) -> Result<RgbImage, ImageError> {
        let max_dim = 2500;
        let scale = if src_w > src_h {
            max_dim as f32 / src_w as f32
        } else {
            max_dim as f32 / src_h as f32
        };

        if scale >= 1.0 {
            return Ok(src.clone());
        }

        let dst_w = (src_w as f32 * scale) as u32;
        let dst_h = (src_h as f32 * scale) as u32;

        let src_image = ResizeImage::from_vec_u8(
            PixelType::U8x3, src_w, src_h, src.as_raw().clone(),
        )?;

        let mut dst_image = ResizeImage::new(PixelType::U8x3, dst_w, dst_h);

        let mut resizer = Resizer::new();
        resizer.resize(&src_image, &mut dst_image,
            &ResizeOptions::new().filter(FilterType::Lanczos3))?;

        RgbImage::from_raw(dst_w, dst_h, dst_image.into_vec())
            .ok_or(ImageError::MemoryError("Failed to create thumbnail".into()))
    }
}

/// 视口渲染器（支持三种模式）
pub struct ViewportRenderer {
    resizer: Resizer,
}

impl ViewportRenderer {
    pub fn new() -> Self {
        Self { resizer: Resizer::new() }
    }

    /// 渲染当前视口（根据模式选择策略）
    pub fn render(&mut self, loaded: &LoadedImage, viewport: &Viewport)
        -> Result<RenderOutput, RenderError>
    {
        match loaded.mode {
            // 直接模式：返回原图数据，GPU 直接处理
            RenderMode::Direct => {
                Ok(RenderOutput::FullTexture {
                    data: loaded.original.as_raw().clone(),
                    width: loaded.original.width(),
                    height: loaded.original.height(),
                })
            }

            // 轻量模式：从原图裁剪并重采样
            RenderMode::Lightweight => {
                let texture = self.render_from_source(
                    &loaded.original,
                    loaded.original.width(),
                    loaded.original.height(),
                    viewport,
                )?;
                Ok(RenderOutput::ViewportTexture(texture))
            }

            // 完整模式：智能选择源图
            RenderMode::Full => {
                // 低缩放用缩略图，高缩放用原图
                let (src, src_w, src_h) = if viewport.scale <= 0.5 {
                    let thumb = loaded.thumbnail.as_ref().unwrap();
                    (thumb, thumb.width(), thumb.height())
                } else {
                    (&loaded.original, loaded.original.width(), loaded.original.height())
                };

                let texture = self.render_from_source(src, src_w, src_h, viewport)?;
                Ok(RenderOutput::ViewportTexture(texture))
            }
        }
    }

    /// 从源图像裁剪并重采样到视口尺寸
    fn render_from_source(
        &mut self,
        src: &RgbImage,
        src_w: u32, src_h: u32,
        viewport: &Viewport,
    ) -> Result<ViewportTexture, RenderError>
    {
        let crop = self.calculate_crop_region(viewport, src_w, src_h);

        let src_image = ResizeImage::from_vec_u8(
            PixelType::U8x3, src_w, src_h, src.as_raw().clone(),
        )?;

        let mut dst_image = ResizeImage::new(PixelType::U8x3, viewport.width, viewport.height);

        self.resizer.resize(&src_image, &mut dst_image,
            &ResizeOptions::new()
                .filter(FilterType::Lanczos3)
                .crop(crop.x, crop.y, crop.width, crop.height)
        )?;

        Ok(ViewportTexture {
            data: dst_image.into_vec(),
            width: viewport.width,
            height: viewport.height,
        })
    }

    /// 计算裁剪区域
    fn calculate_crop_region(&self, viewport: &Viewport, src_w: u32, src_h: u32) -> CropRegion {
        let center_x = viewport.x + viewport.width as f32 / 2.0 / viewport.scale;
        let center_y = viewport.y + viewport.height as f32 / 2.0 / viewport.scale;

        let crop_w = (viewport.width as f32 / viewport.scale).min(src_w as f32);
        let crop_h = (viewport.height as f32 / viewport.scale).min(src_h as f32);

        let x = (center_x - crop_w / 2.0).clamp(0.0, (src_w as f32 - crop_w).max(0.0));
        let y = (center_y - crop_h / 2.0).clamp(0.0, (src_h as f32 - crop_h).max(0.0));

        CropRegion {
            x: x as u32,
            y: y as u32,
            width: crop_w as u32,
            height: crop_h as u32,
        }
    }
}

/// 渲染输出（两种形式）
pub enum RenderOutput {
    /// 完整纹理（直接模式，GPU 硬件缩放）
    FullTexture {
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// 视口纹理（轻量/完整模式，CPU 重采样）
    ViewportTexture(ViewportTexture),
}

/// 视口纹理
pub struct ViewportTexture {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// 裁剪区域
pub struct CropRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// 视口状态
pub struct Viewport {
    pub x: f32,      // 视口左上角 X（图像坐标）
    pub y: f32,      // 视口左上角 Y（图像坐标）
    pub scale: f32,  // 缩放级别 (1.0 = 100%)
    pub width: u32,  // 视口宽度（屏幕像素）
    pub height: u32, // 视口高度（屏幕像素）
}

/// 渲染结果统计
pub struct RenderStats {
    pub frame_time_ms: f32,
}

/// 手势动作
#[derive(Debug, Clone)]
pub enum GestureAction {
    None,
    Pan { dx: f32, dy: f32 },           // 平移
    Zoom { scale: f32, center: (f32, f32) }, // 缩放
    SwitchImage { direction: i8 },      // -1 = 上一张, 1 = 下一张
    ToggleZoom,                         // 双击切换缩放
}

/// 查看器状态（用于 Frontend 同步）
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ViewerState {
    pub current_index: usize,
    pub total_count: usize,
    pub current_path: String,
    pub scale: f32,
    pub is_exif_visible: bool,
    pub has_next: bool,
    pub has_prev: bool,
}
```

#### 4.1.2 错误类型定义

```rust
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("Failed to decode image: {0}")]
    DecodeError(String),
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("File not found: {0}")]
    NotFound(String),
    #[error("Image too large: {width}x{height}, max is {max_width}x{max_height}")]
    TooLarge { width: u32, height: u32, max_width: u32, max_height: u32 },
    #[error("Memory allocation failed: {0}")]
    MemoryError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("GPU context lost")]
    ContextLost,
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Invalid viewport")]
    InvalidViewport,
    #[error("Surface not available")]
    SurfaceNotAvailable,
}

#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    #[error("Failed to attach to window")]
    AttachFailed,
    #[error("Surface disconnected")]
    Disconnected,
}
```

### 4.2 Android 原生层模块

| 模块 | 文件路径 | 职责 | 关键方法 |
|------|----------|------|----------|
| `ImageViewerActivity` | `gen/android/.../viewer/ImageViewerActivity.kt` | 管理生命周期、启动 Surface | `onCreate()`, `onDestroy()` |
| `NativeSurfaceView` | `gen/android/.../viewer/NativeSurfaceView.kt` | 创建 SurfaceView、处理触摸事件 | `surfaceCreated()`, `onTouchEvent()` |
| `GestureDetector` | `gen/android/.../viewer/GestureDetector.kt` | 识别手势、转发到 Rust | `onScale()`, `onScroll()`, `onDoubleTap()` |

```kotlin
// NativeSurfaceView.kt
class NativeSurfaceView(context: Context) : SurfaceView(context), SurfaceHolder.Callback {
    
    init {
        holder.addCallback(this)
        setZOrderOnTop(true)  // 确保 SurfaceView 在 WebView 之上
    }
    
    override fun surfaceCreated(holder: SurfaceHolder) {
        // 获取 ANativeWindow 并传递给 Rust
        val window = holder.surface
        RustBridge.attachSurface(window)
    }
    
    override fun onTouchEvent(event: MotionEvent): Boolean {
        // 转发触摸事件到 Rust
        RustBridge.onTouchEvent(event.action, event.x, event.y, event.pointerCount)
        return true
    }
}
```

### 4.3 前端模块

| 模块 | 文件路径 | 职责 |
|------|----------|------|
| `ImageViewerOverlay` | `src/components/ImageViewerOverlay.tsx` | UI 覆盖层（EXIF、按钮、导航） |
| `viewerStore` | `src/stores/viewerStore.ts` | 查看器状态管理 |
| `ViewerConfigCard` | `src/components/ViewerConfigCard.tsx` | 配置界面 |

---

## 5. 数据流与交互流程

### 5.1 打开图片流程

```
用户点击图片
    │
    ▼
GalleryCard.tsx 检查配置：内置查看器 or 外部应用
    │
    ▼ (内置查看器)
invoke('open_image_viewer', { path, index, total })
    │
    ▼
viewer_bridge.rs
    ├─ 启动 ImageViewerActivity (通过 JNI)
    ├─ 创建 NativeSurfaceView
    └─ 初始化 Rust 组件：
         ImageLoader::load(path) → ImagePyramid
         GpuRenderer::init(surface)
         TileManager::new(cache_size)
    │
    ▼
GpuRenderer 渲染初始视口（缩放 1x，居中显示）
    │
    ▼
viewer_bridge 发送事件到 Frontend：viewer-state-update
    │
    ▼
ImageViewerOverlay.tsx 显示 UI 覆盖层（EXIF、按钮）
```

### 5.2 手势处理与渲染流程（原生层）

```
用户触摸屏幕
    │
    ▼
NativeSurfaceView.onTouchEvent(event)
    │
    ▼
GestureDetector 识别手势类型
    │
    ├─ 双指捏合 ───────────────────────────────┐
    ├─ 单指拖动（缩放状态下）─────────────────────┤
    ├─ 单指滑动（非缩放状态）─────────────────────┤ 所有手势
    ├─ 双击 ────────────────────────────────────┤ 统一转发
    └─ 系统返回键 ──────────────────────────────┘
    │                                          │
    ▼                                          ▼
RustBridge.onTouchEvent()           RustBridge.onBackPressed()
    │                                          │
    ▼                                          ▼
gesture_handler.rs 处理手势            viewer_bridge.rs
    │                                   关闭查看器
    ├─ Pan → 更新 Viewport.x/y
    ├─ Zoom → 更新 Viewport.scale（限制 5x）
    ├─ SwitchImage → 加载新图片
    └─ ToggleZoom → scale 1x ↔ 2x
    │
    ▼
ViewportRenderer::render(viewport)
    │
    ├─ 选择源图像（原图 or 缩略图）
    ├─ 计算视口裁剪区域
    └─ Lanczos3 高质量重采样
    │
    ▼
GpuRenderer::render(viewport_texture) → 60 FPS
    │
    ▼
发送 viewer-state-update 事件到 Frontend（同步状态）
```

### 5.3 手势交互规则

| 手势 | 条件 | 行为 |
|------|------|------|
| 双指捏合 | 任意位置 | 缩放图片（1x ~ 5x） |
| 单指拖动 | scale > 1 | 平移图片（受边界限制） |
| 单指滑动 | scale = 1 | 切换上一张/下一张（带过渡动画） |
| 双击 | 任意位置 | 快速切换：1x ↔ 2x |
| 系统返回键 | 任意状态 | 关闭查看器，返回图库 |

**设计理由**：
- 避免与 Android 系统手势冲突（边缘滑动返回）
- 简化实现，减少误触
- 符合主流相册应用交互习惯

### 5.4 配置读取流程

```
用户打开配置界面
    │
    ▼
ViewerConfigCard.tsx 从 configStore 读取 viewerConfig
    │
    ▼
用户切换"打开方式"选项
    │
    ▼
updateDraft({ viewerConfig: { openMethod: 'external' } })
    │
    ▼
自动保存到 Rust 后端
    │
    ▼
src-tauri/src/config.rs 保存到 config.json
```

---

## 6. 配置设计

### 6.1 数据结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ImageOpenMethod {
    BuiltInViewer,
    ExternalApp,
}

impl Default for ImageOpenMethod {
    fn default() -> Self {
        ImageOpenMethod::BuiltInViewer
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ImageViewerConfig {
    pub open_method: ImageOpenMethod,
    pub max_zoom: f32,          // 默认 5.0
    pub show_exif: bool,        // 默认 true
}

impl Default for ImageViewerConfig {
    fn default() -> Self {
        Self {
            open_method: ImageOpenMethod::default(),
            max_zoom: 5.0,
            show_exif: true,
        }
    }
}

// 集成到 AppConfig（使用 Default 而非 Option）
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    // ... 现有字段 ...
    pub viewer_config: ImageViewerConfig,  // 不是 Option
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            // ... 其他字段 ...
            viewer_config: ImageViewerConfig::default(),
        }
    }
}
```

### 6.2 UI 配置项

- **图片打开方式**：单选按钮组
  - 使用内置查看器（默认）
  - 使用外部应用打开
- **最大缩放级别**：滑块（1x - 10x，默认 5x）
- **显示 EXIF 信息**：开关（默认开启）

> **变更说明**：移除了 `tile_cache_size` 配置项，因为新方案不再使用瓦片缓存。

---

## 7. 测试策略

### 7.1 单元测试覆盖

| 模块 | 测试内容 | 框架 | 测试文件 |
|------|----------|------|----------|
| `image_loader` | 图像解码、缩略图生成、EXIF 解析 | `cargo test` | `loader_test.rs` |
| `viewport_renderer` | 视口计算、裁剪区域计算、重采样质量 | `cargo test` | `viewport_test.rs` |
| `gesture_handler` | 手势识别逻辑、视口计算 | `cargo test` | `gesture_test.rs` |
| `gpu_renderer` | 纹理上传、渲染（使用 mock） | `mockall` + `cargo test` | `gpu_test.rs` |
| `viewer_bridge` | Tauri 命令、错误处理、事件发送 | `cargo test` | `bridge_test.rs` |

### 7.2 关键测试用例

```rust
// === 渲染模式选择测试 ===
#[test]
fn test_render_mode_selection() {
    // 小图 (< 10MP) → 直接模式
    assert_eq!(RenderMode::from_dimensions(3000, 3000), RenderMode::Direct);
    assert_eq!(RenderMode::from_dimensions(2500, 2000), RenderMode::Direct);

    // 中图 (10-50MP) → 轻量模式
    assert_eq!(RenderMode::from_dimensions(5000, 5000), RenderMode::Lightweight);
    assert_eq!(RenderMode::from_dimensions(7000, 5000), RenderMode::Lightweight);

    // 大图 (> 50MP) → 完整模式
    assert_eq!(RenderMode::from_dimensions(8000, 8000), RenderMode::Full);
    assert_eq!(RenderMode::from_dimensions(10000, 10000), RenderMode::Full);
}

// === 图像加载测试 ===
#[test]
fn test_load_small_image_direct_mode() {
    // 5MP 图片应使用直接模式
    let loaded = ImageLoader::load(test_image_5mp()).unwrap();
    assert_eq!(loaded.mode, RenderMode::Direct);
    assert!(loaded.thumbnail.is_none());  // 直接模式不生成缩略图
}

#[test]
fn test_load_medium_image_lightweight_mode() {
    // 24MP 图片应使用轻量模式
    let loaded = ImageLoader::load(test_image_24mp()).unwrap();
    assert_eq!(loaded.mode, RenderMode::Lightweight);
    assert!(loaded.thumbnail.is_none());  // 轻量模式不生成缩略图
}

#[test]
fn test_load_large_image_full_mode() {
    // 100MP 图片应使用完整模式
    let loaded = ImageLoader::load(test_image_100mp()).unwrap();
    assert_eq!(loaded.mode, RenderMode::Full);
    assert!(loaded.thumbnail.is_some());  // 完整模式生成缩略图

    // 缩略图最大边应 <= 2500
    let thumb = loaded.thumbnail.unwrap();
    assert!(thumb.width().max(thumb.height()) <= 2500);
}

// === 渲染测试 ===
#[test]
fn test_render_direct_mode() {
    let loaded = ImageLoader::load(test_image_5mp()).unwrap();
    let mut renderer = ViewportRenderer::new();
    let viewport = Viewport { scale: 1.0, ..default_viewport() };

    let output = renderer.render(&loaded, &viewport).unwrap();

    // 直接模式应返回完整纹理
    match output {
        RenderOutput::FullTexture { width, height, .. } => {
            assert_eq!(width, loaded.original.width());
            assert_eq!(height, loaded.original.height());
        }
        _ => panic!("Expected FullTexture for direct mode"),
    }
}

#[test]
fn test_render_lightweight_mode() {
    let loaded = ImageLoader::load(test_image_24mp()).unwrap();
    let mut renderer = ViewportRenderer::new();
    let viewport = Viewport { scale: 2.0, ..default_viewport() };

    let output = renderer.render(&loaded, &viewport).unwrap();

    // 轻量模式应返回视口纹理
    match output {
        RenderOutput::ViewportTexture(texture) => {
            assert_eq!(texture.width, viewport.width);
            assert_eq!(texture.height, viewport.height);
        }
        _ => panic!("Expected ViewportTexture for lightweight mode"),
    }
}

#[test]
fn test_render_full_mode_source_selection() {
    let loaded = ImageLoader::load(test_image_100mp()).unwrap();
    let mut renderer = ViewportRenderer::new();

    // 低缩放 (scale <= 0.5) 应使用缩略图
    let low_scale_viewport = Viewport { scale: 0.3, ..default_viewport() };
    let output = renderer.render(&loaded, &low_scale_viewport).unwrap();
    assert!(matches!(output, RenderOutput::ViewportTexture(_)));

    // 高缩放 (scale > 0.5) 应使用原图
    let high_scale_viewport = Viewport { scale: 1.5, ..default_viewport() };
    let output = renderer.render(&loaded, &high_scale_viewport).unwrap();
    assert!(matches!(output, RenderOutput::ViewportTexture(_)));
}

#[test]
fn test_viewport_crop_region() {
    let renderer = ViewportRenderer::new();
    let viewport = Viewport {
        x: 500.0, y: 500.0,
        scale: 2.0,
        width: 1080, height: 1920
    };
    let region = renderer.calculate_crop_region(&viewport, 10000, 10000);

    // 验证裁剪区域在图像边界内
    assert!(region.x + region.width <= 10000);
    assert!(region.y + region.height <= 10000);
}

// === 手势测试 ===
#[test]
fn test_pan_gesture_at_max_zoom() {
    let mut viewport = Viewport { x: 0.0, y: 0.0, scale: 5.0, width: 1080, height: 1920 };
    let action = GestureAction::Pan { dx: -10000.0, dy: 0.0 };

    apply_gesture(&mut viewport, action, image_size());

    assert!(viewport.x >= 0.0);
}

#[test]
fn test_switch_image_at_default_zoom() {
    let viewport = Viewport { x: 0.0, y: 0.0, scale: 1.0, width: 1080, height: 1920 };
    let gesture = detect_gesture(touch_events_swipe_right());

    assert!(matches!(gesture, GestureAction::SwitchImage { direction: -1 }));
}
```

### 7.3 集成测试

| 测试场景 | 验证内容 |
|----------|----------|
| 打开 100MP 图片 | 初始化时间 < 500ms，内存使用 ~320MB |
| 快速缩放 | 60 FPS 保持，无卡顿 |
| 无极缩放质量 | 任意缩放级别（1x~5x）保持清晰 |
| 快速滑动切换 | 过渡动画流畅，无闪烁 |
| 内存压力测试 | 连续浏览 50 张图片，无 OOM |
| GPU 降级 | 模拟 GPU 失败，正确降级并提示用户 |

---

## 8. 错误处理策略

| 错误类型 | 处理方式 | 用户可见 |
|----------|----------|----------|
| 图像加载失败 | 显示错误提示 + 重试按钮 | "无法加载图片，点击重试" |
| GPU 初始化失败 | 降级到 WebView 渲染 | "使用兼容模式打开" |
| 内存不足 | 清理 LRU 缓存 + 提示 | "内存不足，已清理缓存" |
| 分片生成超时 | 显示低分辨率占位 + 后台重试 | 低质量预览，加载完成后切换 |
| 不支持的格式 | 提示使用外部应用打开 | "该格式不支持，使用外部应用打开" |

**GPU 降级方案**：
- 如果 wgpu 初始化失败，使用 Android WebView 的硬件加速显示图片
- 降级后不支持 100MP+ 图片，提示用户使用外部应用
- 降级状态保存到 session，下次直接使用降级方案

---

## 9. 依赖库

| 用途 | 库名 | 版本 | 说明 |
|------|------|------|------|
| 图像解码 | `image` | 0.25+ | 内部使用 zune-jpeg，性能与 libjpeg-turbo 相当 |
| 图像缩放 | `fast_image_resize` | 5.0+ | 纯 Rust，SIMD 加速，支持 Lanczos3 |
| GPU 渲染 | `wgpu` | 0.19+ | 跨平台 GPU API |
| EXIF 解析 | `nom-exif` | 2.7+ | 已在项目中使用 |
| 错误处理 | `thiserror` | 2.0+ | 已在项目中使用 |

### 9.1 image crate 关键特性

```rust
use image::{ImageReader, DynamicImage, RgbImage};

// 加载大图片（必须移除默认 512MB 限制）
let img = ImageReader::open("large.jpg")?
    .no_limits()                    // 移除内存限制
    .with_guessed_format()?         // 自动检测格式
    .decode()?;

// 转换为 RGB8 格式
let rgb_image: RgbImage = img.to_rgb8();
let (width, height) = rgb_image.dimensions();

// 内存保护（可选）
let img = ImageReader::open("large.jpg")?
    .set_memory_limit(1_000_000_000)  // 限制 1GB
    .decode()?;
```

### 9.2 fast_image_resize 关键特性

```rust
use fast_image_resize::{Resizer, Image as ResizeImage, FilterType, ResizeOptions, PixelType};

// 高质量下采样（Lanczos3 滤镜）
let src = ResizeImage::from_vec_u8(PixelType::U8x3, src_w, src_h, src_data)?;
let mut dst = ResizeImage::new(PixelType::U8x3, dst_w, dst_h);

let mut resizer = Resizer::new();
resizer.resize(&src, &mut dst, &ResizeOptions::new()
    .filter(FilterType::Lanczos3))?;

// 裁剪并重采样（视口渲染核心操作）
resizer.resize(&src, &mut dst, &ResizeOptions::new()
    .filter(FilterType::Lanczos3)
    .crop(crop_x, crop_y, crop_w, crop_h))?;
```

### 9.3 依赖选择理由

| 库 | 选择理由 |
|-----|---------|
| `image` | 1. v0.25 内部使用 zune-jpeg，性能与原生相当<br>2. 成熟生态系统，文档完善<br>3. 内置 `set_memory_limit()` 内存保护<br>4. 与现有 nom-exif 集成良好 |
| `fast_image_resize` | 1. 纯 Rust，无 FFI<br>2. SIMD 加速<br>3. 支持高质量 Lanczos3 滤镜<br>4. 支持裁剪+缩放一步完成 |

**移除的依赖**：
- ~~`libvips`~~：Android 集成过于复杂，使用纯 Rust 方案替代
- ~~`OpenSeadragon`~~：与 GPU 渲染互斥，使用原生手势处理
- ~~HEIF/HEIC 支持~~：需要 libvips 或 libheif，暂时移除
- ~~`lru` 瓦片缓存~~：简化方案不再需要瓦片缓存

---

## 10. 实现顺序建议

### Phase 1: 基础架构（Week 1）
- [ ] 创建 `src-tauri/src/image_viewer/` 模块结构
- [ ] 集成 `image` crate + `fast_image_resize`
- [ ] 实现 `RenderMode` 枚举和尺寸判断逻辑
- [ ] 实现 `ImageLoader`（解码 + 按模式生成缩略图）+ 单元测试
- [ ] 实现 `ViewportRenderer`（三种模式渲染）+ 单元测试

### Phase 2: Android 原生层（Week 2）
- [ ] 创建 `ImageViewerActivity.kt`
- [ ] 实现 `NativeSurfaceView`
- [ ] JNI 桥接层（attach/detach surface）

### Phase 3: GPU 渲染（Week 3）
- [ ] 集成 `wgpu`
- [ ] 实现 `GpuRenderer`（支持 FullTexture 和 ViewportTexture）
- [ ] 验证 60 FPS 渲染性能

### Phase 4: 手势与交互（Week 4）
- [ ] 实现 `GestureDetector.kt`
- [ ] 实现 `gesture_handler.rs`
- [ ] 手势 ↔ 视口更新联动

### Phase 5: Frontend 集成（Week 5）
- [ ] `ImageViewerOverlay.tsx`
- [ ] 状态同步（Tauri Events）
- [ ] UI 控件（EXIF、导航指示器）

### Phase 6: 配置与测试（Week 6）
- [ ] `ViewerConfigCard.tsx`
- [ ] 配置持久化
- [ ] 集成测试 + 性能优化
- [ ] 三种渲染模式验证
- [ ] 无极缩放质量验证

---

## 11. 决策记录

| 日期 | 决策 | 理由 |
|------|------|------|
| 2026-03-17 | 选择完整方案（Native Overlay + GPU 渲染） | 最高性能，满足 100MP+ 需求 |
| 2026-03-17 | 使用纯原生 GPU 渲染，移除 OpenSeadragon | 与 GPU 渲染互斥，原生方案性能更好 |
| 2026-03-17 | 手势处理完全在原生层实现 | 避免 WebView 性能瓶颈，获得最佳响应 |
| 2026-03-17 | 单指滑动切换图片（非缩放状态） | 简单直观，避免与系统手势冲突 |
| 2026-03-17 | 固定 5x 最大缩放 | 平衡性能与实用性 |
| 2026-03-17 | 使用纯 Rust 图像处理（替代 libvips） | Android 集成更简单，避免 FFI 复杂性 |
| 2026-03-17 | 全局配置 + 记住选择 | 用户意图明确，实现简单 |
| 2026-03-17 | ImageViewerConfig 使用 Default trait（非 Option） | 简化空值处理 |
| 2026-03-19 | 使用 `image` crate 替代 `zune-image` | image-rs v0.25 内部已用 zune-jpeg，生态更成熟 |
| 2026-03-19 | 移除 HEIF/HEIC 支持 | HEIF 需要 libheif/libvips，增加依赖复杂度 |
| 2026-03-19 | 保留原图 + 动态 Lanczos3 重采样 | 实现高质量无极缩放，任意比例都保持最高质量 |
| 2026-03-19 | 移除瓦片缓存机制 | 简化实现，直接从原图裁剪视口区域 |
| 2026-03-19 | 用 ViewportRenderer 替代 TileManager | 直接渲染视口区域，无需复杂瓦片管理 |
| **2026-03-19** | **动态策略选择** | **根据图片尺寸选择渲染模式，小图节省内存** |
| **2026-03-19** | **直接模式（< 10MP）** | **小图直接上传 GPU，无需 CPU 重采样** |
| **2026-03-19** | **轻量模式（10-50MP）** | **中图仅保留原图，省去缩略图内存** |
| **2026-03-19** | **完整模式（> 50MP）** | **大图保留原图+缩略图，保证高质量无极缩放** |

---

## 12. 参考文档

### 12.1 核心库文档

- [image crate 文档](https://docs.rs/image/latest/image/)
- [image crate GitHub](https://github.com/image-rs/image)
- [fast_image_resize](https://github.com/Cykooz/fast_image_resize)
- [fast_image_resize API 文档](https://docs.rs/fast_image_resize/latest/fast_image_resize/)

### 12.2 框架文档

- [Tauri v2 Mobile](https://tauri.app/start/)
- [wgpu 文档](https://wgpu.rs/)
- [Android SurfaceView](https://developer.android.com/reference/android/view/SurfaceView)

### 12.3 相关讨论

- [image-rs 采用 zune-jpeg](https://github.com/image-rs/image/issues/1845)
- [image-rs memory limits](https://github.com/image-rs/image/issues/938)
- [Memory-safe PNG decoders benchmark](https://www.reddit.com/r/rust/comments/1ha7uyi/memorysafe_png_decoders_now_vastly_outperform_c/)
