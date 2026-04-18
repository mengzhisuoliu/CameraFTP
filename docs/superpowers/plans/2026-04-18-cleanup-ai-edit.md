# Cleanup Plan: ai-edit Branch Code Quality

> Worktree: `.worktrees/cleanup-review` (cleanup branch, fast-forwarded to main)
> Date: 2026-04-18
> Total issues: 28 (12 dead code, 8 low-value tests, 8 simplifiable + 1 proguard)

---

## Priority Ranking (All Issues)

| Rank | ID | Category | Severity | File | Issue |
|------|----|----------|----------|------|-------|
| 1 | S16 | Crash risk | **Critical** | `proguard-rules.pro` | Missing `ImageViewerBridge$Companion` keep rule — R8 strips Companion in release |
| 2 | T1 | Broken test | **High** | `useAiEditProgress.test.tsx:162-196` | 2 tests assert old `notifyNativeDone` signature — `(true, null)` vs actual `(true, "修图完成...")` |
| 3 | T1b | Broken test | **High** | `useAiEditProgress.test.tsx:275-295` | Test expects `isDone='no'` immediately after `doneEvent()`, but source sets `isDone: true` first |
| 4 | T5 | Broken test | **High** | `AiEditProgressBar.test.tsx:132-147` | Test asserts `bottom-20` class, but code uses `style={{ bottom: '5rem' }}` |
| 5 | D4 | Dead code | Medium | `service.rs:174,310` | `WorkerState.batch_total` write-only field |
| 6 | D2 | Dead code | Medium | `service.rs:26` | `_DEFAULT_EDIT_PROMPT` constant + test |
| 7 | D3 | Dead code | Medium | `service.rs:139` | `queue_len()` method never called |
| 8 | D1 | Dead code | Low | `config.rs:66` | `SEEDREAM_MODELS` constant unused in Rust |
| 9 | D5 | Dead code | Medium | `seedream-models.ts:22-24` | `getSeedreamModelLabel()` never imported |
| 10 | D6 | Dead code | Medium | `useAiEditProgress.ts:37-38` | `_listenerCleanup` never invoked |
| 11 | D10 | Dead code | Medium | `ImageViewerBridge.kt:131-137` | `emitGalleryItemsAddedForUri()` dead @JavascriptInterface |
| 12 | D9 | Dead code | Low | `ImageViewerActivity.kt:130-133` | `scanNewFile()` companion never called |
| 13 | D12 | Dead code | Low | `App.tsx:34` | Redundant `useAiEditProgressListener()` |
| 14 | D7 | Dead code | Low | `ui/index.ts:16` | `SelectOption` re-export unused |
| 15 | D11 | Dead code | Low | `types/index.ts:27-29` | Unused type re-exports |
| 16 | T2 | Low-value | Medium | `useAiEditProgress.test.tsx:341-355` | Exact duplicate of L129-143 |
| 17 | T3 | Low-value | Low | `useAiEditProgress.test.tsx:336-339` | Trivial `cancelAiEdit` wrapper test |
| 18 | T4 | Low-value | Low | `useAiEditProgress.test.tsx:447-510` | FTP scenario re-verifies individual tests |
| 19 | T6 | Low-value | Medium | `AiEditProgressBar.test.tsx` | Over-mocked + brittle CSS assertions |
| 20 | T7 | Low-value | Low | `error.test.ts` | Trivial tests for JS runtime behavior |
| 21 | T8 | Low-value | Low | `useLatestPhoto.test.tsx:180-199` | Brittle event subscription spy |
| 22 | S1 | Simplifiable | Medium | `service.rs:426-431` | Double error wrapping (AppError wrapping AppError) |
| 23 | S2 | Simplifiable | Medium | `service.rs:420-449` | 4× single-arm `match ProviderConfig::SeedEdit` |
| 24 | S5 | Simplifiable | Medium | `service.rs:600-641` | Test duplicates `WorkerState` struct |
| 25 | S12 | Simplifiable | Medium | `ImageViewerActivity.kt` + `ImageViewerBridge.kt` | Duplicate URI→file path resolution |
| 26 | S14 | Simplifiable | Medium | `ImageViewerActivity.kt:466-497` | Double JSON decode hack |
| 27 | S6 | Simplifiable | Low | `AiEditProgressBar.tsx:164-185` | Duplicate keyframes |
| 28 | S8 | Simplifiable | Medium | `AiEditConfigPanel.tsx:54-59` | Migration logic in UI component |
| -- | S3,S4,S7,S9,S10,S11,S13,S15 | Simplifiable | Low | Various | Minor cleanups (see details below) |

---

## Phase 1: Fix Broken Tests + Crash Risk

**Goal**: Eliminate actively misleading tests and prevent release-build crash.

### 1.1 Add missing proguard rule
**File**: `src-tauri/gen/android/app/proguard-rules.pro`
**Change**: Add `-keep class com.gjk.cameraftpcompanion.bridges.ImageViewerBridge$Companion { *; }`

### 1.2 Fix `notifyNativeDone` tests
**File**: `src/hooks/__tests__/useAiEditProgress.test.tsx`
**Fix test at L162-175** "notifies native layer on success":
```typescript
// Before: expect(onAiEditComplete).toHaveBeenCalledWith(true, null);
// After:
expect(onAiEditComplete).toHaveBeenCalledWith(true, '修图完成，共3张');
```

**Fix test at L177-196** "notifies native layer with failure message":
```typescript
// Before: expect(onAiEditComplete).toHaveBeenCalledWith(false, '修图完成，2张失败：fail1.jpg、fail2.jpg');
// After:
expect(onAiEditComplete).toHaveBeenCalledWith(false, '修图完成，成功1张，失败2张');
```

### 1.3 Fix `isDone` timing test
**File**: `src/hooks/__tests__/useAiEditProgress.test.tsx`
**Fix test at L275-295** "resets state after timeout":
```typescript
// The test asserts isDone='no' immediately after doneEvent(), but source sets isDone=true.
// Fix: assert isDone='yes' first, then advance past 3000ms to verify reset.
eventHandler!(doneEvent());
await act(async () => { await flush(); });
expect(getText('is-done')).toBe('yes');  // Changed: was 'no'

await act(async () => {
  vi.advanceTimersByTime(3000);  // Changed: was 500
  await flush();
});
expect(getText('is-done')).toBe('no');  // After timeout, reset
expect(getText('is-editing')).toBe('no');
```

### 1.4 Fix `bottom-20` CSS test
**File**: `src/components/__tests__/AiEditProgressBar.test.tsx`
**Fix test at L132-147**: Update to match actual implementation using inline style:
```typescript
// Before: expect(bar.className).toContain('bottom-20');
// After: verify inline style or adjust to match actual rendering
```

**Verification**: Run `./build.sh windows android` + frontend tests

---

## Phase 2: Remove Dead Code

**Goal**: Remove all unused functions, constants, types, and methods.

### 2.1 Rust dead code
- **`config.rs:66-70`**: Remove `SEEDREAM_MODELS` constant
- **`service.rs:26`**: Remove `_DEFAULT_EDIT_PROMPT` constant
- **`service.rs:139`**: Remove `queue_len()` method
- **`service.rs:174,187,196,310`**: Remove `batch_total` field from `WorkerState`
- **`service.rs:595`**: Remove `default_edit_prompt_is_chinese` test

### 2.2 TypeScript dead code
- **`seedream-models.ts:22-24`**: Remove `getSeedreamModelLabel()` function
- **`seedream-models.ts:7-12`**: Replace `SeedreamModel` interface with `SelectOption` import (also fixes S10)
- **`useAiEditProgress.ts:37-38`**: Remove `_listenerCleanup` variable and `void` suppression
- **`useAiEditProgress.ts:168`**: Remove `_listenerCleanup = unlisten;` assignment
- **`ui/index.ts:16`**: Remove `export type { SelectOption }` re-export
- **`App.tsx:34`**: Remove `useAiEditProgressListener()` call
- **`types/index.ts:27-29`**: Remove `AiEditConfig`, `ProviderConfig`, `SeedEditConfig` re-exports

### 2.3 Kotlin dead code
- **`ImageViewerActivity.kt:127-133`**: Remove `scanNewFile()` companion method
- **`ImageViewerBridge.kt:131-137`**: Remove `emitGalleryItemsAddedForUri()` method

**Verification**: Run `./build.sh windows android` + frontend tests

---

## Phase 3: Clean Up Low-Value Tests

**Goal**: Remove duplicate, trivial, and brittle test cases (~260 lines).

### 3.1 `useAiEditProgress.test.tsx`
- **Remove L341-355**: Duplicate "gallery refresh" test (identical to L129-143)
- **Remove L336-339**: Trivial `cancelAiEdit` wrapper test
- **Remove L447-510**: FTP scenario integration test (re-verifies individual tests)

### 3.2 `AiEditProgressBar.test.tsx`
- **Remove L132-163**: Broken + brittle CSS class positioning tests
- Keep: null-render test, cancel/dismiss button tests, basic rendering tests

### 3.3 `error.test.ts`
- **Remove L19-21**: Redundant `null` test (covered by `undefined`)
- **Remove L50-52**: `handles numbers` trivial test
- **Remove L56-59**: `silent: returns result on success` trivial test

### 3.4 `useLatestPhoto.test.tsx`
- **Remove L180-199**: Brittle event subscription spy test

**Verification**: Run `./build.sh windows android` + frontend tests

---

## Phase 4: Simplify Code

**Goal**: Reduce complexity, deduplicate, improve maintainability.

### 4.1 Rust simplifications
- **`service.rs:426-431`** (S1): Fix double error wrapping — use `??` or `?` on inner Result
- **`service.rs:420-449`** (S2): Destructure `ProviderConfig::SeedEdit` once instead of 4 matches
- **`config.rs:35`** (S3): Use `ProviderConfig::default()` in `AiEditConfig::default()`
- **`image_processor/mod.rs:8-11`** (S4): Remove unnecessary `#[allow(dead_code)]`
- **`service.rs:600-641`** (S5): Extract `WorkerState` to module level so test uses real type

### 4.2 TypeScript simplifications
- **`AiEditProgressBar.tsx:164-185`** (S6): Merge duplicate keyframes into one
- **`AiEditConfigPanel.tsx:54-59`** (S8): Move migration to `configStore` load path
- **`PromptDialog.tsx:10-11`** (S9): Use barrel imports from `./ui`
- **`AiEditConfigCard.tsx:18-23`** (S11): Extract shared `createConfigUpdater` or pass `updateDraft` directly

### 4.3 Kotlin simplifications
- **`ImageViewerActivity.kt:843,846`** (S13): Remove duplicate `stopHighlightSweepAnimation()` call
- **`ImageViewerActivity.kt:466-497`** (S14): Parse JSON once instead of double decode
- **`ImageViewerActivity.kt:807-826`** (S12): Extract shared URI resolution (or delegate to bridge)
- **`ImageViewerActivity.kt:1207-1240`** (S15): Extract `bindViews()` from `onCreate`/`onConfigurationChanged`

**Verification**: Run `./build.sh windows android` + frontend tests

---

## Execution Protocol

After each phase:
1. Commit with message: `refactor: [phase description]`
2. Dispatch **@oracle** subagent for code review of the phase's changes
3. Run `./build.sh windows android` to verify compilation
4. Run frontend tests to verify no regressions
