<template>
  <div class="w-full h-full rounded-box bg-base-200 flex flex-col overflow-hidden">
    <div class="shrink-0 px-3 pt-3 pb-2 border-b border-base-content/10 bg-base-100/45">
      <div class="flex items-center gap-2">
        <div class="min-w-0 flex-1">
          <div class="text-[11px] font-bold uppercase tracking-[0.22em] text-base-content/40">
            Blurry Photos
          </div>
          <div class="mt-1 text-sm font-semibold text-base-content/80 truncate">
            Blur detection for the current view
          </div>
        </div>
        <TButton
          :icon="IconRefresh"
          tooltip="Rescan for blur"
          :buttonSize="'small'"
          :disabled="isScanning"
          @click.stop="triggerScan(true)"
        />
        <TButton
          :icon="IconClose"
          :tooltip="$t('msgbox.close')"
          :buttonSize="'small'"
          @click.stop="$emit('close')"
        />
      </div>

      <div class="mt-3 grid grid-cols-2 gap-2">
        <div class="rounded-box bg-base-200/80 border border-base-content/8 px-2 py-2">
          <div class="text-[10px] uppercase tracking-wide text-base-content/40">Blurry photos</div>
          <div class="text-sm font-bold text-base-content/80">{{ items.length.toLocaleString() }}</div>
        </div>
        <div class="rounded-box bg-base-200/80 border border-base-content/8 px-2 py-2">
          <div class="text-[10px] uppercase tracking-wide text-base-content/40">Selected</div>
          <div class="text-sm font-bold text-base-content/80">{{ selectedIds.size.toLocaleString() }}</div>
        </div>
      </div>
    </div>

    <div v-if="isScanning" class="flex-1 flex items-center justify-center p-5">
      <div class="text-center text-base-content/45 space-y-3">
        <span class="loading loading-spinner text-primary w-8 h-8"></span>
        <div class="text-xs font-semibold">
          Scanning for blur… {{ scanStatus.current }}/{{ scanStatus.total }}
        </div>
      </div>
    </div>

    <div v-else class="flex-1 min-h-0 flex flex-col">
      <div class="shrink-0 p-2 space-y-2 border-b border-base-content/10">
        <div class="rounded-box border border-base-content/8 bg-base-100/45 p-3 space-y-2">
          <div class="flex items-center gap-1.5">
            <button class="btn btn-xs btn-ghost" :disabled="items.length === 0" @click="selectAll">
              <component :is="isAllSelected ? IconCheckNone : IconCheckAll" class="w-3.5 h-3.5" />
              {{ isAllSelected ? 'Clear' : 'Select all' }}
            </button>
            <button
              class="btn btn-xs btn-error btn-outline ml-auto"
              :disabled="selectedIds.size === 0 || isFixing || isDeleting"
              @click="askDeleteSelected"
            >
              <span v-if="isDeleting" class="loading loading-spinner loading-xs"></span>
              <IconTrash v-else class="w-3.5 h-3.5" />
              {{ $t('msgbox.blurry.trash') }} {{ selectedIds.size > 0 ? selectedIds.size : '' }}
            </button>
            <button
              class="btn btn-xs btn-primary"
              :disabled="selectedIds.size === 0 || isFixing"
              @click="fixSelected"
            >
              <span v-if="isFixing" class="loading loading-spinner loading-xs"></span>
              <IconSparkles v-else class="w-3.5 h-3.5" />
              Fix {{ selectedIds.size > 0 ? selectedIds.size : '' }}
            </button>
          </div>

          <div v-if="isFixing" class="text-[11px] text-base-content/50">
            Fixing {{ fixStatus.current + 1 }}/{{ fixStatus.total }}: {{ fixStatus.currentFile }}
          </div>
          <div v-if="fixError" class="text-[11px] text-error leading-relaxed">
            {{ fixError }}
          </div>
          <div v-if="deleteError" class="text-[11px] text-error leading-relaxed">
            {{ deleteError }}
          </div>
        </div>
      </div>

      <div v-if="items.length === 0" class="flex-1 flex items-center justify-center p-5">
        <div class="text-center text-base-content/45 space-y-2 max-w-[280px]">
          <IconFocus class="w-9 h-9 mx-auto text-base-content/25" />
          <div class="text-sm font-bold">No blurry photos found</div>
          <div class="text-xs leading-relaxed">
            Run a scan to check the current view for blurry photos (Laplacian variance below threshold).
          </div>
        </div>
      </div>

      <div v-else class="flex-1 min-h-0 overflow-y-auto divide-y divide-base-content/8">
        <button
          v-for="(item, index) in items"
          :key="item.file_id"
          class="w-full p-2.5 text-left transition-colors hover:bg-base-200/70"
          @click="handleFileClick(item.file_id)"
          @dblclick="openPreview(index)"
        >
          <div class="flex items-center gap-2">
            <label class="shrink-0 flex items-center" @click.stop>
              <input
                type="checkbox"
                class="checkbox checkbox-xs checkbox-primary"
                :checked="selectedIds.has(item.file_id)"
                @change="toggleSelected(item.file_id)"
              />
            </label>

            <div class="w-12 h-12 rounded-box overflow-hidden border border-base-content/10 bg-base-content/5 shrink-0">
              <img v-if="item.file?.thumbnail" :src="item.file.thumbnail" class="w-full h-full object-cover" />
              <div v-else class="w-full h-full skeleton"></div>
            </div>

            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-1.5 min-w-0">
                <span class="text-xs font-bold text-base-content/75 truncate">{{ item.file?.name }}</span>
                <span v-if="item.fixed_path" class="badge badge-xs badge-success">Fixed</span>
              </div>
              <div class="mt-0.5 text-[11px] text-base-content/50">
                Blur score: {{ formatScore(item.blur_score) }}
              </div>
            </div>
            <span
              role="button"
              class="btn btn-xs btn-ghost btn-square shrink-0"
              :title="$t('msgbox.blurry.preview')"
              @click.stop="openPreview(index)"
            >
              <IconZoomActual class="w-3.5 h-3.5" />
            </span>
          </div>
        </button>
      </div>
    </div>

    <Teleport to="body">
    <!-- Large preview: blur is only visible at 100%, so offer a 1:1 view -->
    <div
      v-if="previewItem"
      class="fixed inset-0 z-[60] bg-black/80 flex flex-col"
      @click.self="closePreview"
    >
      <div class="shrink-0 flex items-center gap-2 px-3 py-2 bg-base-100/95 border-b border-base-content/10">
        <div class="min-w-0 flex-1">
          <div class="text-sm font-bold truncate">{{ previewItem.file?.name }}</div>
          <div class="text-[11px] text-base-content/55">
            Blur score {{ formatScore(previewItem.blur_score) }} · {{ previewIndex + 1 }} / {{ items.length }}
          </div>
        </div>
        <button class="btn btn-xs btn-ghost" :disabled="previewIndex <= 0" @click="stepPreview(-1)">
          <IconLeft class="w-3.5 h-3.5" /> {{ $t('msgbox.blurry.prev') }}
        </button>
        <button class="btn btn-xs btn-ghost" :disabled="previewIndex >= items.length - 1" @click="stepPreview(1)">
          {{ $t('msgbox.blurry.next') }} <IconRight class="w-3.5 h-3.5" />
        </button>
        <button class="btn btn-xs btn-ghost" @click="previewActualSize = !previewActualSize">
          <component :is="previewActualSize ? IconZoomFit : IconZoomActual" class="w-3.5 h-3.5" />
          {{ previewActualSize ? $t('msgbox.blurry.fit') : $t('msgbox.blurry.actual_size') }}
        </button>
        <button class="btn btn-xs btn-ghost" @click="keepPreview" :title="$t('msgbox.blurry.keep_tip')">
          {{ $t('msgbox.blurry.keep') }}
        </button>
        <button class="btn btn-xs btn-error" :disabled="isDeleting" @click="trashPreview">
          <IconTrash class="w-3.5 h-3.5" /> {{ $t('msgbox.blurry.trash') }}
        </button>
        <TButton :icon="IconClose" :tooltip="$t('msgbox.blurry.close_tip')" :buttonSize="'small'" @click.stop="closePreview" />
      </div>
      <div
        ref="previewScroller"
        class="flex-1 min-h-0 overflow-auto flex"
        :class="previewActualSize ? '' : 'items-center justify-center'"
        @click.self="closePreview"
      >
        <img
          :key="previewItem.file_id"
          :src="previewSrc"
          :class="previewActualSize ? 'max-w-none m-auto' : 'max-w-full max-h-full object-contain'"
          draggable="false"
          @load="onPreviewLoaded"
        />
      </div>
      <div class="shrink-0 px-3 py-1 text-[11px] text-center text-white/60 bg-black/40">
        {{ $t('msgbox.blurry.shortcuts') }}
      </div>
    </div>

    <!-- Trash confirmation (trash only; never permanent) -->
    <div v-if="confirmIds.length > 0" class="modal modal-open z-[70]">
      <div class="modal-box max-w-sm">
        <h3 class="font-bold text-base">{{ $t('msgbox.blurry.confirm_title') }}</h3>
        <p class="py-3 text-sm text-base-content/70">
          {{ confirmIds.length === 1 ? $t('msgbox.blurry.confirm_one') : $t('msgbox.blurry.confirm_many', { count: confirmIds.length.toLocaleString() }) }}
        </p>
        <div class="modal-action">
          <button class="btn btn-sm" @click="confirmIds = []">Cancel</button>
          <button class="btn btn-sm btn-error" @click="confirmTrash">{{ $t('msgbox.blurry.confirm_ok') }}</button>
        </div>
      </div>
      <div class="modal-backdrop bg-black/40" @click="confirmIds = []"></div>
    </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
// RP-CUSTOM: Blurry-photo finder pane; entire file is fork-only.
import { computed, ref, watch, nextTick, onMounted, onUnmounted } from 'vue';
import { getThumbnailDataUrl, getPreviewUrl } from '@/common/utils';
import TButton from '@/components/TButton.vue';
import { IconCheckAll, IconCheckNone, IconClose, IconFocus, IconRefresh, IconSparkles, IconTrash, IconLeft, IconRight, IconZoomActual, IconZoomFit } from '@/common/icons';
import {
  blurStartScan,
  blurGetScanStatus,
  blurListResults,
  listenBlurScanProgress,
  blurStartFixBatch,
  blurGetFixStatus,
  listenBlurFixProgress,
  getFileThumb,
  batchDeleteFiles,
} from '@/common/api';
import { config } from '@/common/config';
import { useToast } from '@/common/toast';
import { i18n } from '@/common/i18n';
import { emit as tauriEmit } from '@tauri-apps/api/event';

const toast = useToast();

// Scores are computed at <=1024px with a light pre-blur (v2); results saved by the older
// full-resolution method live under a different key and are never reused.
const SCAN_KEY_VERSION = 'blur-v2:';
const scoped = (key: string) => (key ? SCAN_KEY_VERSION + key : '');

const BLUR_RESULT_LIMIT = 500;
const thumbnailPlaceholder = new URL('@/assets/images/image-file.png', import.meta.url).href;

const props = defineProps({
  scanKey: {
    type: String,
    default: '',
  },
  queryParams: {
    type: Object as () => Record<string, any> | null,
    default: null,
  },
});

const emit = defineEmits<{
  close: [];
  'select-file': [fileId: number];
  'preview-file': [fileId: number];
}>();

const isScanning = ref(false);
const isFixing = ref(false);
const fixError = ref('');
const items = ref<any[]>([]);
const selectedIds = ref<Set<number>>(new Set());
const scanStatus = ref<any>({ current: 0, total: 0 });
const fixStatus = ref<any>({ current: 0, total: 0, currentFile: '' });
const unlistenScanProgress = ref<null | (() => void)>(null);
const unlistenFixProgress = ref<null | (() => void)>(null);

const isDeleting = ref(false);
const deleteError = ref('');
const confirmIds = ref<number[]>([]);
const previewIndex = ref(-1);
const previewActualSize = ref(false);
const previewScroller = ref<HTMLElement | null>(null);

const previewItem = computed(() => (previewIndex.value >= 0 ? items.value[previewIndex.value] ?? null : null));
const previewSrc = computed(() => {
  const f = previewItem.value?.file;
  return f ? getPreviewUrl(f.id, f.file_path) : '';
});

function formatScore(score: number) {
  return score >= 100 ? String(Math.round(score)) : score.toFixed(1);
}

function openPreview(index: number) {
  if (index < 0 || index >= items.value.length) return;
  previewIndex.value = index;
  handleFileClick(items.value[index].file_id);
}

function closePreview() {
  previewIndex.value = -1;
}

function stepPreview(delta: number) {
  const next = previewIndex.value + delta;
  if (next >= 0 && next < items.value.length) openPreview(next);
}

// Centre the 1:1 view so the middle of the frame (usually the subject) is visible.
async function onPreviewLoaded() {
  if (!previewActualSize.value) return;
  await nextTick();
  const el = previewScroller.value;
  if (!el) return;
  el.scrollLeft = (el.scrollWidth - el.clientWidth) / 2;
  el.scrollTop = (el.scrollHeight - el.clientHeight) / 2;
}
watch(previewActualSize, () => void onPreviewLoaded());

/** Remove entries from the list without touching the files (also used after trashing). */
function dropItems(ids: Set<number>) {
  if (ids.size === 0) return;
  const current = previewItem.value?.file_id;
  const remaining = items.value.filter((i: any) => !ids.has(i.file_id));
  let nextIndex = -1;
  if (current !== undefined && remaining.length > 0) {
    const stay = remaining.findIndex((i: any) => i.file_id === current);
    nextIndex = stay >= 0 ? stay : Math.min(previewIndex.value, remaining.length - 1);
  }
  items.value = remaining;
  for (const id of ids) selectedIds.value.delete(id);
  previewIndex.value = nextIndex;
}

function keepPreview() {
  const item = previewItem.value;
  if (item) dropItems(new Set([item.file_id]));
}

function trashPreview() {
  const item = previewItem.value;
  if (item) confirmIds.value = [item.file_id];
}

function askDeleteSelected() {
  if (selectedIds.value.size > 0) confirmIds.value = Array.from(selectedIds.value);
}

async function confirmTrash() {
  const ids = new Set(confirmIds.value);
  confirmIds.value = [];
  const files = items.value
    .filter((i: any) => ids.has(i.file_id) && i.file?.file_path)
    .map((i: any) => ({ fileId: i.file_id, filePath: i.file.file_path }));
  if (files.length === 0) return;
  deleteError.value = '';
  isDeleting.value = true;
  try {
    // Always trash (permanently = false); the pane never offers permanent delete.
    const result = await batchDeleteFiles(files, false);
    const deleted = new Set<number>((result?.deletedFileIds ?? []).map((id: any) => Number(id)));
    const failed = files.filter((f) => !deleted.has(f.fileId)).length;
    dropItems(deleted);
    if (deleted.size > 0) {
      // Let the main grid / image viewer drop the files too.
      void tauriEmit('files-deleted', { source: 'blurry', fileIds: [...deleted] });
      toast.success(i18n.global.t('msgbox.blurry.moved', { count: deleted.size }) as string);
    }
    if (failed > 0) {
      deleteError.value = i18n.global.t('msgbox.blurry.failed', { count: failed }) as string;
    }
  } catch (error: any) {
    deleteError.value = String(error?.message || error);
  } finally {
    isDeleting.value = false;
  }
}

function onKeydown(e: KeyboardEvent) {
  if (!previewItem.value || confirmIds.value.length > 0) return;
  const key = e.key;
  if (key === 'Escape') closePreview();
  else if (key === 'ArrowLeft') stepPreview(-1);
  else if (key === 'ArrowRight') stepPreview(1);
  else if (key === 'Delete' || key === 'Backspace') trashPreview();
  else if (key === 'k' || key === 'K') keepPreview();
  else if (key === 'z' || key === 'Z') previewActualSize.value = !previewActualSize.value;
  else return;
  e.preventDefault();
  e.stopPropagation();
}

const isAllSelected = computed(() => items.value.length > 0 && selectedIds.value.size === items.value.length);

function toggleSelected(fileId: number) {
  if (selectedIds.value.has(fileId)) selectedIds.value.delete(fileId);
  else selectedIds.value.add(fileId);
}

function selectAll() {
  if (isAllSelected.value) {
    selectedIds.value = new Set();
  } else {
    selectedIds.value = new Set(items.value.map((item: any) => item.file_id));
  }
}

function handleFileClick(fileId: number) {
  emit('select-file', fileId);
}

async function hydrateThumbnails(list: any[]) {
  const tasks = list.map(async (item: any) => {
    const file = item?.file;
    if (!file || file.thumbnail) return;
    if (!file.file_path) {
      file.thumbnail = thumbnailPlaceholder;
      return;
    }
    const thumb = await getFileThumb(
      file.id,
      file.file_path,
      file.file_type || 1,
      file.e_orientation || 0,
      config.settings.thumbnailSize,
      false
    );
    file.thumbnail = getThumbnailDataUrl(thumb, thumbnailPlaceholder, false, config.settings.thumbnailSize, file.file_path);
  });
  await Promise.all(tasks);
}

async function fetchResults() {
  try {
    const result = await blurListResults(scoped(props.scanKey), BLUR_RESULT_LIMIT, 0);
    const list = Array.isArray(result?.items) ? result.items : [];
    await hydrateThumbnails(list);
    items.value = list;
    const available = new Set(list.map((item: any) => item.file_id));
    for (const id of Array.from(selectedIds.value)) {
      if (!available.has(id)) selectedIds.value.delete(id);
    }
  } catch (error) {
    console.error('BlurryPane fetchResults error:', error);
    items.value = [];
  }
}

async function triggerScan(force = false) {
  if (!props.scanKey) return;
  if (!force) {
    const hasScan = items.value.length > 0;
    if (hasScan) return;
  }

  isScanning.value = true;
  try {
    await blurStartScan(scoped(props.scanKey), Date.now(), props.queryParams || null);
    await pollScanUntilDone();
  } catch (error) {
    console.error('BlurryPane triggerScan error:', error);
    isScanning.value = false;
  }
}

async function pollScanUntilDone() {
  while (true) {
    const status = await blurGetScanStatus();
    scanStatus.value = status;
    if (!status?.isScanning) break;
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  isScanning.value = false;
  await fetchResults();
}

async function fixSelected() {
  if (selectedIds.value.size === 0) return;
  fixError.value = '';
  isFixing.value = true;
  try {
    await blurStartFixBatch(Array.from(selectedIds.value), config.settings.blurFix.apiBaseUrl, 'deblur');
    await pollFixUntilDone();
  } catch (error: any) {
    fixError.value = String(error?.message || error);
    isFixing.value = false;
  }
}

async function pollFixUntilDone() {
  while (true) {
    const status = await blurGetFixStatus();
    fixStatus.value = status;
    if (!status?.isFixing) {
      if (status?.error) fixError.value = status.error;
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  isFixing.value = false;
  selectedIds.value = new Set();
  await fetchResults();
}

watch(
  () => props.scanKey,
  async (newKey) => {
    if (!newKey) {
      items.value = [];
      selectedIds.value = new Set();
      return;
    }
    await triggerScan(false);
    await fetchResults();
  },
  { immediate: true }
);

onMounted(async () => {
  window.addEventListener('keydown', onKeydown, true);
  unlistenScanProgress.value = await listenBlurScanProgress((event: any) => {
    scanStatus.value = event?.payload || scanStatus.value;
  });
  unlistenFixProgress.value = await listenBlurFixProgress((event: any) => {
    fixStatus.value = event?.payload || fixStatus.value;
  });
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown, true);
  if (unlistenScanProgress.value) unlistenScanProgress.value();
  if (unlistenFixProgress.value) unlistenFixProgress.value();
});
</script>
