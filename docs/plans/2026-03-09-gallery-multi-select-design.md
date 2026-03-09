# 图库多选功能设计

## 概述

为图库功能添加长按多选、删除和分享功能。用户可通过长按图片进入多选模式，批量选择图片后执行删除或分享操作。

## 交互流程

```
长按图片 → 进入多选模式 → 点击切换选中状态 → 点击FAB → 选择操作 → 执行
                                                              ↓
                                              点击"取消选择"或返回键退出
```

## UI 设计

### 1. 多选模式状态

- **进入**：长按任意图片
- **退出**：
  - 点击 FAB 菜单中的"取消选择"
  - 按返回键
- **状态**：`isSelectionMode: boolean` + `selectedIds: Set<number>`

### 2. 选中指示器

- 位置：图片左上角
- 未选中：空心圆圈 `border-2 border-white/70 bg-black/30`
- 选中：实心蓝色圆圈 + 白色勾选图标 `bg-blue-500`

### 3. 右下角浮动按钮 (FAB)

- 位置：`fixed bottom-20 right-4`（底部导航栏上方）
- 样式：圆形按钮 `w-14 h-14 rounded-full bg-blue-500 shadow-lg`
- 图标：加号/菜单图标
- 徽章：选中数量显示在 FAB 右上角（选中1张以上时）

### 4. 操作菜单

- 触发：点击 FAB
- 位置：FAB 上方弹出，垂直排列
- 样式：白色圆角卡片 `bg-white rounded-xl shadow-xl min-w-[140px]`
- 菜单项：
  - 🗑️ 删除 (X张)
  - 📤 分享 (X张)
  - ✖️ 取消选择

### 5. 删除确认

- 弹窗：`confirm("确定删除 X 张图片？")`
- 确认后执行删除

## 技术实现

### 前端修改

**GalleryCard.tsx**

```typescript
// 新增状态
const [isSelectionMode, setIsSelectionMode] = useState(false);
const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());

// 长按处理
const handleLongPress = (id: number) => {
  setIsSelectionMode(true);
  setSelectedIds(new Set([id]));
};

// 点击处理
const handleClick = (id: number) => {
  if (isSelectionMode) {
    setSelectedIds(prev => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  } else {
    // 原有逻辑：打开图片
  }
};

// 删除
const handleDelete = async () => {
  if (confirm(`确定删除 ${selectedIds.size} 张图片？`)) {
    const success = await window.GalleryAndroid?.deleteImages(JSON.stringify([...selectedIds]));
    if (success) {
      loadImages();
      setIsSelectionMode(false);
      setSelectedIds(new Set());
    }
  }
};

// 分享
const handleShare = async () => {
  await window.GalleryAndroid?.shareImages(JSON.stringify([...selectedIds]));
};
```

### Android Bridge 扩展

**GalleryBridge.kt**

```kotlin
@JavascriptInterface
fun deleteImages(idsJson: String): Boolean {
    val ids = JSONArray(idsJson).let { json ->
        (0 until json.length()).map { json.getInt(it) }
    }
    // 通过 MediaStore 删除，需要写入权限
    return try {
        val uri = MediaStore.Images.Media.EXTERNAL_CONTENT_URI
        ids.forEach { id ->
            context.contentResolver.delete(
                ContentUris.withAppendedId(uri, id.toLong()),
                null, null
            )
        }
        true
    } catch (e: Exception) {
        Log.e(TAG, "Delete failed", e)
        false
    }
}

@JavascriptInterface
fun shareImages(idsJson: String): Boolean {
    val ids = JSONArray(idsJson).let { json ->
        (0 until json.length()).map { json.getInt(it) }
    }
    // 构建 Share Intent，支持多图
    return try {
        val uris = ids.map { id ->
            ContentUris.withAppendedId(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                id.toLong()
            )
        }
        val intent = Intent(Intent.ACTION_SEND_MULTIPLE).apply {
            type = "image/*"
            putParcelableArrayListExtra(Intent.EXTRA_STREAM, ArrayList(uris))
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        context.startActivity(Intent.createChooser(intent, "分享图片"))
        true
    } catch (e: Exception) {
        Log.e(TAG, "Share failed", e)
        false
    }
}
```

### 类型定义更新

**types/global.ts**

```typescript
interface GalleryAndroid {
  getImages(): Promise<string>;
  deleteImages(idsJson: string): Promise<boolean>;
  shareImages(idsJson: string): Promise<boolean>;
}
```

## 文件变更清单

| 文件 | 变更 |
|------|------|
| `src/components/GalleryCard.tsx` | 添加多选状态、FAB、菜单、长按/点击处理 |
| `src/types/global.ts` | 更新 `GalleryAndroid` 接口 |
| `src-tauri/gen/android/.../GalleryBridge.kt` | 添加 `deleteImages`、`shareImages` 方法 |

## 注意事项

1. **权限**：删除操作需要 `WRITE_EXTERNAL_STORAGE` 权限（Android 10 及以下），Android 11+ 需要 `MANAGE_EXTERNAL_STORAGE` 或使用 MediaStore 的写入 API
2. **返回键**：监听返回键事件，多选模式下退出多选而非关闭应用
3. **空选择**：FAB 菜单在选中数量为0时隐藏删除/分享选项，或禁用它们
