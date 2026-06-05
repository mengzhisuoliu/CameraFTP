# Drag Multi-Select for Gallery Grid

## Overview

Add long-press-then-slide multi-select to the Android gallery grid, matching the system photo album's drag-select interaction. After long-pressing an item, the user keeps holding and slides across other items to batch-select them. Releasing the finger stays in selection mode for further tap adjustments.

## Current Behavior

- Long press (400ms) on a cell enters selection mode with that item selected
- Moving finger >15px during the long-press wait cancels the long press (allows scrolling)
- After entering selection mode, tap individual items to toggle selection
- Deselecting the last item exits selection mode

## New Behavior

1. Long press fires → enters selection mode, selects initial item (unchanged)
2. **NEW**: If user keeps holding and slides finger → each cell under the finger is added to selection
3. **NEW**: Scroll is suppressed during drag-select to prevent conflict
4. Touch end → stays in selection mode (unchanged)

## Technical Design

### Approach: elementFromPoint + native non-passive touchmove listener

- **Cell detection**: `document.elementFromPoint(clientX, clientY)` + `closest('[data-media-id]')` to find the cell under the finger during drag
- **Scroll prevention**: Native `{ passive: false }` touchmove listener on the scroll container, calls `preventDefault()` during drag-select

### Changes

#### 1. `useGallerySelection.ts`

Add a mutable ref `isDragSelectingRef` to track drag-select state:

- When long press timer fires: set `isDragSelectingRef.current = true`
- Modify `handleTouchMove`: if `isDragSelecting`, skip the move-threshold cancellation logic (finger already committed to drag-select)
- Modify `handleTouchEnd`: reset `isDragSelectingRef.current = false`
- Add `handleDragSelect(mediaId: string)`: adds mediaId to `selectedIds` (idempotent, no toggle — only adds)
- Expose `isDragSelectingRef` in return value

#### 2. `VirtualGalleryGrid.tsx`

- Accept new props: `onDragSelect?: (mediaId: string) => void` and `isDragSelectingRef?: React.RefObject<boolean>`
- Install a native non-passive touchmove listener on `containerRef` via `useEffect`
- In the listener: if `isDragSelectingRef.current` is true:
  1. Call `event.preventDefault()` to suppress scrolling
  2. Get touch coordinates from `event.touches[0]`
  3. Call `document.elementFromPoint(clientX, clientY)`
  4. Find `closest('[data-media-id]')` and extract the mediaId
  5. Call `onDragSelect(mediaId)` if found

#### 3. `GalleryCard.tsx` (wiring)

- Pass `isDragSelectingRef` and `onDragSelect` from the selection hook to `VirtualGalleryGrid`

### Edge Cases

- **Fast swipes**: touchmove events may skip cells; acceptable behavior (user can tap to add missed ones)
- **Gap areas**: `elementFromPoint` returns grid container (no `data-media-id`), no cell selected — correct
- **Multi-finger**: existing guard (`event.touches.length > 1`) ignores additional fingers
- **Selection mode already active**: if user long-presses while already in selection mode, existing behavior replaces selection with new single item, then drag extends — works correctly

### Files Modified

| File | Change |
|------|--------|
| `src/hooks/useGallerySelection.ts` | Add isDragSelectingRef, handleDragSelect, modify touch handlers |
| `src/components/VirtualGalleryGrid.tsx` | Add native touchmove listener, new props |
| `src/components/GalleryCard.tsx` | Wire new props |
