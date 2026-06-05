# Drag Multi-Select for Gallery Grid — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add long-press-then-slide multi-select to the gallery grid, matching system photo album behavior.

**Architecture:** Extend `useGallerySelection` hook with drag-select state tracking. Add a native non-passive touchmove listener on the grid's scroll container in `VirtualGalleryGrid` to prevent scrolling and detect cells under the finger via `document.elementFromPoint`. Wire the new props through `GalleryCard`.

**Tech Stack:** React 18 hooks, DOM Touch Events API, TailwindCSS, TypeScript

---

### Task 1: Extend `useGallerySelection` with drag-select state

**Files:**
- Modify: `src/hooks/useGallerySelection.ts`

- [ ] **Step 1: Add `isDragSelectingRef` and modify long-press handler**

Add a new ref `isDragSelectingRef` initialized to `false`. When the long-press timer fires (line ~104-111), set `isDragSelectingRef.current = true` alongside the existing `setIsSelectionMode(true)` call.

In the long-press timer callback, change:
```typescript
longPressTimerRef.current = setTimeout(() => {
  if (!wasScrollingAtTouchStartRef.current) {
    setIsSelectionMode(true);
    isSelectionModeRef.current = true;
    setSelectedIds(new Set([imagePath]));
  }
}, LONG_PRESS_DURATION);
```
to:
```typescript
longPressTimerRef.current = setTimeout(() => {
  if (!wasScrollingAtTouchStartRef.current) {
    setIsSelectionMode(true);
    isSelectionModeRef.current = true;
    isDragSelectingRef.current = true;
    setSelectedIds(new Set([imagePath]));
  }
}, LONG_PRESS_DURATION);
```

- [ ] **Step 2: Add `isDragSelectingRef` declaration**

Near the other refs (around line 50-53), add:
```typescript
const isDragSelectingRef = useRef(false);
```

- [ ] **Step 3: Modify `handleTouchMove` to skip cancellation during drag-select**

The existing `handleTouchMove` (line ~114-129) cancels the long-press timer when finger moves >15px. During active drag-select (after long-press fires), we must NOT cancel — the finger is supposed to move.

Change `handleTouchMove` to:
```typescript
const handleTouchMove = useCallback((event: React.TouchEvent) => {
  // During drag-select, the native listener on the container handles everything.
  // Don't interfere with the long-press cancellation logic.
  if (isDragSelectingRef.current) {
    return;
  }

  if (!touchStartPosRef.current || !longPressTimerRef.current) {
    return;
  }

  const touch = event.touches[0];
  const dx = touch.clientX - touchStartPosRef.current.x;
  const dy = touch.clientY - touchStartPosRef.current.y;
  const distance = Math.sqrt(dx * dx + dy * dy);

  if (distance > TOUCH_MOVE_THRESHOLD) {
    clearTimeout(longPressTimerRef.current);
    longPressTimerRef.current = null;
    touchStartPosRef.current = null;
  }
}, []);
```

- [ ] **Step 4: Modify `handleTouchEnd` to reset drag-select state**

In `handleTouchEnd` (line ~131-138), add reset of `isDragSelectingRef`:
```typescript
const handleTouchEnd = useCallback(() => {
  if (longPressTimerRef.current) {
    clearTimeout(longPressTimerRef.current);
    longPressTimerRef.current = null;
  }
  isDragSelectingRef.current = false;
  touchStartPosRef.current = null;
  wasScrollingAtTouchStartRef.current = false;
}, []);
```

- [ ] **Step 5: Add `handleDragSelect` function**

Add a new callback that adds a mediaId to the selection (idempotent, no toggle):
```typescript
const handleDragSelect = useCallback((mediaId: string) => {
  setSelectedIds((prev) => {
    if (prev.has(mediaId)) return prev;
    const next = new Set(prev);
    next.add(mediaId);
    return next;
  });
}, []);
```

- [ ] **Step 6: Update the return object**

Add `isDragSelectingRef` and `handleDragSelect` to the return object (line ~325-344):
```typescript
return {
  isSelectionMode,
  selectedIds,
  // ... existing fields ...
  isDragSelectingRef,
  handleDragSelect,
};
```

- [ ] **Step 7: Update `UseGallerySelectionResult` type**

Add the two new fields to the result type (line ~22-41):
```typescript
type UseGallerySelectionResult = {
  // ... existing fields ...
  isDragSelectingRef: React.RefObject<boolean>;
  handleDragSelect: (mediaId: string) => void;
};
```

- [ ] **Step 8: Build to verify**

Run: `./build.sh windows android`
Expected: Build succeeds with no errors.

---

### Task 2: Add native touchmove listener and drag-select logic to `VirtualGalleryGrid`

**Files:**
- Modify: `src/components/VirtualGalleryGrid.tsx`

- [ ] **Step 1: Add new props to the interface**

Add to `VirtualGalleryGridProps` (after line 29, before the `onNearEnd` prop):
```typescript
/** Drag-select: called with the mediaId under the finger during drag */
onDragSelect?: (mediaId: string) => void;
/** Ref to check if drag-select is active (from useGallerySelection) */
isDragSelectingRef?: React.RefObject<boolean>;
```

- [ ] **Step 2: Destructure new props**

In the component function, add to the destructuring (line ~34-47):
```typescript
onDragSelect,
isDragSelectingRef,
```

- [ ] **Step 3: Add native touchmove listener useEffect**

Add a new `useEffect` after the existing ResizeObserver effect (after line ~71) that installs a native non-passive touchmove listener on the scroll container:

```typescript
// Native non-passive touchmove listener for drag-select scroll prevention
useEffect(() => {
  const el = containerRef.current;
  if (!el) return;

  const handleNativeTouchMove = (event: TouchEvent) => {
    if (!isDragSelectingRef?.current) return;

    event.preventDefault();

    if (!onDragSelect) return;

    const touch = event.touches[0];
    if (!touch) return;

    const element = document.elementFromPoint(touch.clientX, touch.clientY);
    if (!element) return;

    const cell = (element as HTMLElement).closest<HTMLElement>('[data-media-id]');
    if (!cell) return;

    const mediaId = cell.dataset.mediaId;
    if (mediaId) {
      onDragSelect(mediaId);
    }
  };

  el.addEventListener('touchmove', handleNativeTouchMove, { passive: false });

  return () => {
    el.removeEventListener('touchmove', handleNativeTouchMove);
  };
}, [onDragSelect, isDragSelectingRef]);
```

- [ ] **Step 4: Build to verify**

Run: `./build.sh windows android`
Expected: Build succeeds with no errors.

---

### Task 3: Wire new props in `GalleryCard`

**Files:**
- Modify: `src/components/GalleryCard.tsx`

- [ ] **Step 1: Destructure new fields from `useGallerySelection`**

In the destructuring of `useGallerySelection` result (line ~47-65), add:
```typescript
isDragSelectingRef,
handleDragSelect,
```

- [ ] **Step 2: Pass new props to `VirtualGalleryGrid`**

Add the new props to the `<VirtualGalleryGrid>` JSX (line ~263-276), after `onTouchEnd`:
```tsx
onDragSelect={handleDragSelect}
isDragSelectingRef={isDragSelectingRef}
```

- [ ] **Step 3: Build to verify**

Run: `./build.sh windows android`
Expected: Build succeeds with no errors.

- [ ] **Step 4: Commit**

```bash
git add src/hooks/useGallerySelection.ts src/components/VirtualGalleryGrid.tsx src/components/GalleryCard.tsx
git commit -m "feat(gallery): add long-press slide multi-select for gallery grid"
```
