<template>
  <div class="w-full h-full rounded-box bg-base-200 flex flex-col overflow-hidden">
    <div class="shrink-0 px-3 pt-3 pb-2 border-b border-base-content/10 bg-base-100/45">
      <div class="flex items-center gap-2">
        <div class="min-w-0 flex-1">
          <div class="text-[11px] font-bold uppercase tracking-[0.22em] text-base-content/40">
            Duplicate Review
          </div>
          <div class="mt-1 text-sm font-semibold text-base-content/80 truncate">
            Exact hash matches from LAP albums
          </div>
        </div>
        <TButton
          :icon="IconRefresh"
          :tooltip="$t('toolbar.tooltip.refresh')"
          :buttonSize="'small'"
          :disabled="isDedupLoading"
          @click.stop="triggerBackendDedup(true)"
        />
        <TButton
          :icon="IconClose"
          :tooltip="$t('msgbox.close')"
          :buttonSize="'small'"
          @click.stop="$emit('close')"
        />
      </div>

      <div class="mt-3 grid grid-cols-3 gap-2">
        <div class="rounded-box bg-base-200/80 border border-base-content/8 px-2 py-2">
          <div class="text-[10px] uppercase tracking-wide text-base-content/40">Groups</div>
          <div class="text-sm font-bold text-base-content/80">{{ totalGroupCount.toLocaleString() }}</div>
        </div>
        <div class="rounded-box bg-base-200/80 border border-base-content/8 px-2 py-2">
          <div class="text-[10px] uppercase tracking-wide text-base-content/40">Duplicates</div>
          <div class="text-sm font-bold text-base-content/80">{{ totalDuplicateFileCount.toLocaleString() }}</div>
        </div>
        <div class="rounded-box bg-base-200/80 border border-base-content/8 px-2 py-2">
          <div class="text-[10px] uppercase tracking-wide text-base-content/40">Reclaim</div>
          <div class="text-sm font-bold text-base-content/80 truncate">{{ formatFileSize(totalReclaimableBytes) }}</div>
        </div>
      </div>
    </div>

    <div v-if="isDedupLoading" class="flex-1 flex items-center justify-center p-5">
      <div class="text-center text-base-content/45 space-y-3">
        <span class="loading loading-spinner text-primary w-8 h-8"></span>
        <div class="text-xs font-semibold">{{ $t('info_panel.dedup.scanning') }}</div>
      </div>
    </div>

    <div v-else class="flex-1 min-h-0 flex flex-col">
      <div class="shrink-0 p-2 space-y-2 border-b border-base-content/10">
        <div class="rounded-box border border-base-content/8 bg-base-100/45 p-3 space-y-3">
          <div class="flex items-start gap-2">
            <div class="min-w-0 flex-1">
              <div class="text-xs font-bold uppercase tracking-wide text-base-content/55">Match Criteria</div>
              <div class="mt-1 flex flex-wrap gap-1.5">
                <span class="badge badge-sm badge-primary">Exact hash</span>
                <span class="badge badge-sm badge-outline">Same file size</span>
                <span class="badge badge-sm badge-outline">Backend verified</span>
              </div>
            </div>
          </div>

          <div>
            <div class="mb-1.5 text-xs font-bold uppercase tracking-wide text-base-content/55">Media Type</div>
            <div class="grid grid-cols-4 gap-1.5">
              <button
                v-for="option in mediaTypeOptions"
                :key="option.value"
                class="btn btn-xs"
                :class="mediaTypeFilter === option.value ? 'btn-primary' : 'btn-ghost'"
                @click="mediaTypeFilter = option.value"
              >
                {{ option.label }}
              </button>
            </div>
          </div>

          <div>
            <div class="mb-1.5 text-xs font-bold uppercase tracking-wide text-base-content/55">Delete Duplicates From Album</div>
            <div class="flex items-center gap-1.5">
              <select v-model="deleteAlbumId" class="select select-xs select-bordered min-w-0 flex-1">
                <option value="">Choose album...</option>
                <option
                  v-for="album in albumOptions"
                  :key="album.id"
                  :value="String(album.id)"
                >
                  {{ album.name }} ({{ album.duplicateCount }})
                </option>
              </select>
              <button
                class="btn btn-xs btn-primary shrink-0"
                :disabled="!deleteAlbumId || filteredDuplicateGroups.length === 0 || isPreparingAlbumSelection"
                @click="selectDeleteAlbumDuplicates"
              >
                <span v-if="isPreparingAlbumSelection" class="loading loading-spinner loading-xs"></span>
                <IconCheckAll v-else class="w-3.5 h-3.5" />
                Select
              </button>
            </div>
            <div class="mt-1.5 text-[11px] leading-relaxed text-base-content/45">
              Files outside the chosen album are protected. If needed, the keep marker is moved to a protected album before selection.
            </div>
          </div>

          <div class="flex items-center gap-2 pt-1">
            <button class="btn btn-xs btn-ghost" :disabled="selectedDeleteCount === 0" @click="clearSelection">
              <IconCheckNone class="w-3.5 h-3.5" />
              Clear
            </button>
            <button
              class="btn btn-xs ml-auto"
              :class="selectedDeleteCount === 0 ? 'btn-ghost text-base-content/35' : 'btn-error'"
              :disabled="selectedDeleteCount === 0"
              @click="trashSelectedDuplicates"
            >
              <IconTrash class="w-3.5 h-3.5" />
              {{ isMac ? $t('menu.file.move_to_trash') : $t('menu.file.delete') }}
            </button>
          </div>

          <div v-if="selectedDeleteCount > 0" class="text-xs font-medium text-error">
            {{ selectedDeleteCount.toLocaleString() }} selected, {{ formatFileSize(selectedDeleteBytes) }}
          </div>
        </div>
      </div>

      <div v-if="filteredDuplicateGroups.length === 0" class="flex-1 flex items-center justify-center p-5">
        <div class="text-center text-base-content/45 space-y-2 max-w-[280px]">
          <IconSimilar class="w-9 h-9 mx-auto text-base-content/25" />
          <div class="text-sm font-bold">No matching duplicate groups</div>
          <div class="text-xs leading-relaxed">
            Change the media type or refresh the exact-hash scan for the current LAP view.
          </div>
        </div>
      </div>

      <div v-else class="flex-1 min-h-0 grid grid-rows-[minmax(140px,35%)_1fr] gap-2 p-2">
        <div class="min-h-0 overflow-y-auto space-y-1.5 pr-1">
          <button
            v-for="(group, idx) in filteredDuplicateGroups"
            :key="group.id"
            class="w-full rounded-box border p-2.5 text-left transition-colors"
            :class="selectedGroupId === group.id
              ? 'border-primary/60 bg-primary/8'
              : 'border-base-content/8 bg-base-100/35 hover:bg-base-100/60 hover:border-base-content/20'"
            @click="selectedGroupId = group.id"
          >
            <div class="flex items-center gap-2">
              <div class="w-11 h-11 rounded-box overflow-hidden border border-base-content/10 bg-base-content/5 shrink-0">
                <img v-if="group.keepItem?.file?.thumbnail" :src="group.keepItem.file.thumbnail" class="w-full h-full object-cover" />
                <div v-else class="w-full h-full skeleton"></div>
              </div>
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class="text-xs font-bold text-base-content/75">{{ $t('info_panel.dedup.group_label', { index: idx + 1 }) }}</span>
                  <span class="badge badge-xs badge-outline">{{ getGroupMediaLabel(group) }}</span>
                </div>
                <div class="mt-1 text-[11px] text-base-content/45 truncate">
                  {{ getGroupAlbumSummary(group) }}
                </div>
              </div>
              <div class="text-right shrink-0">
                <div class="text-xs font-semibold text-base-content/70">{{ formatFileSize(group.reclaimableBytes) }}</div>
                <div class="text-[11px] text-base-content/45">{{ group.file_count }} {{ $t('info_panel.dedup.items') }}</div>
              </div>
            </div>
          </button>
        </div>

        <div v-if="activeGroup" class="min-h-0 rounded-box border border-base-content/8 bg-base-100/35 flex flex-col overflow-hidden">
          <div class="shrink-0 px-3 py-2 border-b border-base-content/10 flex items-center gap-2">
            <div class="min-w-0 flex-1">
              <div class="text-xs font-bold text-base-content/75 truncate">
                {{ $t('info_panel.dedup.group_label', { index: activeGroupIndex + 1 }) }}
              </div>
              <div class="text-[11px] text-base-content/45 truncate">
                {{ activeGroup.file_count }} exact files, {{ formatFileSize(activeGroup.reclaimableBytes) }} reclaimable
              </div>
            </div>
            <button class="btn btn-xs btn-ghost" @click="emit('compare-group', String(activeGroup.id), activeGroup.keepItem?.file_id || 0)">
              <IconSplitOn class="w-3.5 h-3.5" />
            </button>
            <button class="btn btn-xs btn-ghost" :disabled="!deleteAlbumId" @click="selectGroupDuplicates(activeGroup.id)">
              <component :is="isAllGroupDuplicatesSelected(activeGroup.id) ? IconCheckNone : IconCheckAll" class="w-3.5 h-3.5" />
            </button>
          </div>

          <div class="flex-1 min-h-0 overflow-y-auto divide-y divide-base-content/8">
            <button
              v-for="item in activeGroup.items"
              :key="item.file_id"
              class="w-full p-2.5 text-left transition-colors"
              :class="[
                item.is_keep === 1 ? 'bg-primary/5' : 'hover:bg-base-200/70',
                isDupSelected(activeGroup.id, item.file_id) ? 'bg-error/7 hover:bg-error/10' : ''
              ]"
              @click="handleFileClick(item.file_id)"
              @dblclick="emit('preview-file', item.file_id)"
            >
              <div class="flex items-center gap-2">
                <label class="shrink-0 flex items-center" @click.stop>
                  <input
                    v-if="canDeleteItem(item)"
                    type="checkbox"
                    class="checkbox checkbox-xs checkbox-primary"
                    :checked="isDupSelected(activeGroup.id, item.file_id)"
                    @change="toggleDupSelected(activeGroup.id, item.file_id)"
                  />
                  <IconLock v-else class="w-4 h-4 text-primary" />
                </label>

                <div class="w-12 h-12 rounded-box overflow-hidden border border-base-content/10 bg-base-content/5 shrink-0">
                  <img v-if="item.file?.thumbnail" :src="item.file.thumbnail" class="w-full h-full object-cover" />
                  <div v-else class="w-full h-full skeleton"></div>
                </div>

                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-1.5 min-w-0">
                    <span class="text-xs font-bold text-base-content/75 truncate">{{ item.file?.name }}</span>
                    <span v-if="item.is_keep === 1" class="badge badge-xs badge-primary">{{ $t('info_panel.dedup.keep_label') }}</span>
                    <span v-else-if="isProtectedItem(item)" class="badge badge-xs badge-outline">Protected</span>
                  </div>
                  <div class="mt-0.5 text-[11px] text-base-content/50 truncate" :title="formatDedupFolderPath(item.file)">
                    {{ formatDedupFolderPath(item.file) }}
                  </div>
                  <div class="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[11px] text-base-content/40">
                    <span>{{ formatFileSize(Number(item.file?.size || 0)) }}</span>
                    <span>{{ getFileMediaLabel(item.file) }}</span>
                    <span v-if="item.file?.width && item.file?.height">{{ item.file.width }} x {{ item.file.height }}</span>
                    <span v-if="item.file?.created_at">{{ formatTimestamp(item.file.created_at, $t('format.date_time')) }}</span>
                  </div>
                </div>

                <button
                  v-if="item.is_keep !== 1 && !isProtectedItem(item)"
                  class="btn btn-xs btn-ghost shrink-0"
                  @click.stop="setKeep(activeGroup.id, item.file_id)"
                >
                  {{ $t('info_panel.dedup.set_keep') }}
                </button>
              </div>
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted } from 'vue';
import { formatFileSize, getFolderName, getFolderPath, formatFolderBreadcrumb, getThumbnailDataUrl, isMac, formatTimestamp } from '@/common/utils';
import TButton from '@/components/TButton.vue';
import { IconCheckAll, IconCheckNone, IconClose, IconLock, IconSimilar, IconSplitOn, IconTrash, IconRefresh } from '@/common/icons';
import { dedupStartScan, dedupGetScanStatus, dedupGetOverview, listenDedupScanProgress, dedupListGroups, dedupSetKeep, getAlbum, getFileThumb } from '@/common/api';
import { config } from '@/common/config';

const dedupPane2GlobalState = ((globalThis as any).__lapDedupPane2State ||= {
  lastScanKey: '',
});
const DEDUP_GROUP_LIMIT = 200;
const thumbnailPlaceholder = new URL('@/assets/images/image-file.png', import.meta.url).href;

const props = defineProps({
  selectedFileId: {
    type: Number,
    default: -1,
  },
  dedupScanKey: {
    type: String,
    default: '',
  },
  dedupQueryParams: {
    type: Object as () => Record<string, any> | null,
    default: null,
  },
});

const emit = defineEmits<{
  close: [];
  'select-file': [fileId: number];
  'preview-file': [fileId: number];
  'compare-group': [groupId: string, keepFileId: number];
  'trash-selected-duplicates': [groupId: string, fileIds: number[], reclaimableBytes: number];
}>();

type MediaTypeFilter = 'all' | 'image' | 'raw' | 'video';

const mediaTypeOptions: Array<{ value: MediaTypeFilter; label: string }> = [
  { value: 'all', label: 'All' },
  { value: 'image', label: 'Image' },
  { value: 'raw', label: 'RAW' },
  { value: 'video', label: 'Video' },
];

const selectedDupIdsByGroup = ref<Map<number, Set<number>>>(new Map());
const mediaTypeFilter = ref<MediaTypeFilter>('all');
const deleteAlbumId = ref('');
const isPreparingAlbumSelection = ref(false);
const isDedupLoading = ref(false);
const unlistenDedupProgress = ref<null | (() => void)>(null);
const queuedScanKey = ref('');
const rawGroups = ref<any[]>([]);
const selectedGroupId = ref<number | null>(null);
const totalGroupCount = ref(0);
const totalDuplicateFileCount = ref(0);
const totalReclaimableBytes = ref(0);
const albumRootPaths = ref<Map<number, string>>(new Map());
const dedupStatusPollTimer = ref<ReturnType<typeof setInterval> | null>(null);
const isPollingDedupStatus = ref(false);

const duplicateGroups = computed(() =>
  rawGroups.value.map((group: any) => {
    const keepItem = (group.items || []).find((i: any) => i.is_keep === 1) || null;
    const duplicateItems = (group.items || []).filter((i: any) => i.is_keep === 0);
    return {
      ...group,
      keepItem,
      duplicateItems,
      reclaimableBytes: Math.max(0, Number(group.total_size || 0) - Number(group.file_size || 0)),
    };
  })
);

const filteredDuplicateGroups = computed(() =>
  duplicateGroups.value.filter(group => groupMatchesMediaFilter(group))
);

const activeGroup = computed(() => {
  if (selectedGroupId.value === null) return null;
  return filteredDuplicateGroups.value.find(group => group.id === selectedGroupId.value) || null;
});

const activeGroupIndex = computed(() => {
  if (!activeGroup.value) return -1;
  return filteredDuplicateGroups.value.findIndex(group => group.id === activeGroup.value.id);
});

const albumOptions = computed(() => {
  const albums = new Map<number, { id: number; name: string; duplicateCount: number }>();
  for (const group of filteredDuplicateGroups.value) {
    for (const item of group.duplicateItems || []) {
      const file = item?.file;
      const albumId = Number(file?.album_id || 0);
      if (albumId <= 0) continue;
      const name = file?.album_name || albumRootPaths.value.get(albumId) || `Album ${albumId}`;
      const existing = albums.get(albumId) || { id: albumId, name, duplicateCount: 0 };
      existing.duplicateCount += 1;
      albums.set(albumId, existing);
    }
  }
  return Array.from(albums.values()).sort((a, b) => a.name.localeCompare(b.name));
});

const selectedDeleteCount = computed(() => {
  let count = 0;
  for (const group of duplicateGroups.value) {
    count += group.duplicateItems.filter((item: any) => isDupSelected(group.id, item.file_id)).length;
  }
  return count;
});

const selectedDeleteBytes = computed(() => {
  let total = 0;
  for (const group of duplicateGroups.value) {
    total += group.duplicateItems.reduce((sum: number, item: any) => {
      return isDupSelected(group.id, item.file_id) ? sum + Number(item.file?.size || 0) : sum;
    }, 0);
  }
  return total;
});

function getFileType(file: any): number {
  return Number(file?.file_type || 0);
}

function fileMatchesMediaFilter(file: any) {
  const fileType = getFileType(file);
  if (mediaTypeFilter.value === 'all') return true;
  if (mediaTypeFilter.value === 'image') return fileType === 1;
  if (mediaTypeFilter.value === 'video') return fileType === 2;
  if (mediaTypeFilter.value === 'raw') return fileType === 3 || fileType === 4;
  return true;
}

function groupMatchesMediaFilter(group: any) {
  return (group.items || []).some((item: any) => fileMatchesMediaFilter(item?.file));
}

function getFileMediaLabel(file: any) {
  const fileType = getFileType(file);
  if (fileType === 1) return 'Image';
  if (fileType === 2) return 'Video';
  if (fileType === 3 || fileType === 4) return 'RAW';
  return 'File';
}

function getGroupMediaLabel(group: any) {
  return getFileMediaLabel(group.keepItem?.file || group.items?.[0]?.file);
}

function getSelectedDeleteAlbumId() {
  const albumId = Number(deleteAlbumId.value || 0);
  return Number.isFinite(albumId) && albumId > 0 ? albumId : null;
}

function isProtectedItem(item: any) {
  const selectedAlbumId = getSelectedDeleteAlbumId();
  if (!selectedAlbumId) return item?.is_keep === 1;
  return Number(item?.file?.album_id || 0) !== selectedAlbumId;
}

function canDeleteItem(item: any) {
  const selectedAlbumId = getSelectedDeleteAlbumId();
  if (!selectedAlbumId) return false;
  return item?.is_keep !== 1 && Number(item?.file?.album_id || 0) === selectedAlbumId;
}

function getDupSelectedSet(groupId: number): Set<number> {
  const existing = selectedDupIdsByGroup.value.get(groupId);
  if (existing) return existing;
  const set = new Set<number>();
  selectedDupIdsByGroup.value.set(groupId, set);
  return set;
}

function isDupSelected(groupId: number, fileId: number) {
  return getDupSelectedSet(groupId).has(fileId);
}

function toggleDupSelected(groupId: number, fileId: number) {
  const group = filteredDuplicateGroups.value.find((item: any) => item.id === groupId);
  const item = group?.duplicateItems?.find((dup: any) => dup.file_id === fileId);
  if (!item || !canDeleteItem(item)) return;

  const set = getDupSelectedSet(groupId);
  if (set.has(fileId)) set.delete(fileId);
  else set.add(fileId);
}

function handleFileClick(fileId: number) {
  emit('select-file', fileId);
}

async function setKeep(groupId: number, fileId: number) {
  await dedupSetKeep(groupId, fileId);
  getDupSelectedSet(groupId).delete(fileId);
  await fetchGroups(groupId);
}

function selectGroupDuplicates(groupId: number) {
  const group = filteredDuplicateGroups.value.find(g => g.id === groupId);
  if (!group) return;

  const set = getDupSelectedSet(groupId);
  const duplicateIds = group.duplicateItems
    .filter((item: any) => canDeleteItem(item))
    .map((item: any) => item.file_id);
  const allSelected = duplicateIds.length > 0 && duplicateIds.every((id: number) => set.has(id));

  set.clear();
  if (!allSelected) {
    duplicateIds.forEach((id: number) => set.add(id));
  }
}

function isAllGroupDuplicatesSelected(groupId: number) {
  const group = filteredDuplicateGroups.value.find(g => g.id === groupId);
  if (!group || group.duplicateItems.length === 0) return false;
  const set = getDupSelectedSet(groupId);
  return group.duplicateItems.every((item: any) => set.has(item.file_id));
}

async function selectDeleteAlbumDuplicates() {
  const targetAlbumId = getSelectedDeleteAlbumId();
  if (!targetAlbumId) return;

  isPreparingAlbumSelection.value = true;
  clearSelection();

  try {
    const groupsToRefresh = new Set<number>();
    for (const group of filteredDuplicateGroups.value) {
      const targetItems = (group.items || []).filter((item: any) => Number(item?.file?.album_id || 0) === targetAlbumId);
      const protectedItems = (group.items || []).filter((item: any) => Number(item?.file?.album_id || 0) !== targetAlbumId);

      if (targetItems.length === 0 || protectedItems.length === 0) continue;

      const keepItem = group.keepItem;
      if (keepItem && Number(keepItem?.file?.album_id || 0) === targetAlbumId) {
        const protectedKeep = protectedItems[0];
        await dedupSetKeep(group.id, protectedKeep.file_id);
        groupsToRefresh.add(group.id);
      }
    }

    if (groupsToRefresh.size > 0) {
      await fetchGroups(selectedGroupId.value);
    }

    for (const group of filteredDuplicateGroups.value) {
      const protectedItems = (group.items || []).filter((item: any) => Number(item?.file?.album_id || 0) !== targetAlbumId);
      if (protectedItems.length === 0) continue;

      const set = getDupSelectedSet(group.id);
      for (const item of group.duplicateItems || []) {
        if (Number(item?.file?.album_id || 0) === targetAlbumId && canDeleteItem(item)) {
          set.add(item.file_id);
        }
      }
    }
  } finally {
    isPreparingAlbumSelection.value = false;
  }
}

function clearSelection() {
  selectedDupIdsByGroup.value.clear();
}

function trashSelectedDuplicates() {
  const ids: number[] = [];
  for (const group of duplicateGroups.value) {
    ids.push(...Array.from(getDupSelectedSet(group.id).values()));
  }
  if (ids.length === 0) return;
  emit('trash-selected-duplicates', 'dedup2-visible', Array.from(new Set(ids)), selectedDeleteBytes.value);
}

function formatDedupFolderPath(file: any): string {
  const folderPath = getFolderPath(file?.file_path);
  if (!folderPath) return '';

  const albumId = Number(file?.album_id || 0);
  const albumRoot = albumId ? albumRootPaths.value.get(albumId) || '' : '';
  const albumLabel = file?.album_name || (albumRoot ? getFolderName(albumRoot) : '');
  return formatFolderBreadcrumb(folderPath, albumRoot, albumLabel);
}

function getGroupAlbumSummary(group: any): string {
  const names = new Set<string>();
  for (const item of group.items || []) {
    const file = item?.file;
    if (file?.album_name) names.add(file.album_name);
  }
  if (names.size === 0) return 'Current view';
  return Array.from(names).slice(0, 3).join(', ') + (names.size > 3 ? ` +${names.size - 3}` : '');
}

async function hydrateAlbumRootPaths(groups: any[]) {
  const albumIds = new Set<number>();
  for (const group of groups || []) {
    for (const item of Array.isArray(group?.items) ? group.items : []) {
      const albumId = Number(item?.file?.album_id || 0);
      if (albumId > 0 && !albumRootPaths.value.has(albumId)) {
        albumIds.add(albumId);
      }
    }
  }

  if (albumIds.size === 0) return;

  const results = await Promise.all(
    Array.from(albumIds).map(async (albumId) => ({
      albumId,
      album: await getAlbum(albumId),
    }))
  );

  for (const { albumId, album } of results) {
    if (album?.path) {
      albumRootPaths.value.set(albumId, album.path);
    }
  }
}

async function hydrateGroupThumbnails(groups: any[]) {
  const tasks: Promise<void>[] = [];
  for (const group of groups || []) {
    const items = Array.isArray(group?.items) ? group.items : [];
    for (const item of items) {
      const file = item?.file;
      if (!file) continue;
      if (file.thumbnail) continue;
      if (!file.file_path) {
        file.thumbnail = thumbnailPlaceholder;
        continue;
      }
      tasks.push((async () => {
        const thumb = await getFileThumb(
          file.id,
          file.file_path,
          file.file_type || 1,
          file.e_orientation || 0,
          config.settings.thumbnailSize,
          false
        );
        file.thumbnail = getThumbnailDataUrl(thumb, thumbnailPlaceholder, false, config.settings.thumbnailSize, file.file_path);
      })());
    }
  }
  await Promise.all(tasks);
}

async function refreshOverview() {
  try {
    const overview = await dedupGetOverview();
    if (!overview) return;
    totalGroupCount.value = Number(overview.total_groups || 0);
    totalDuplicateFileCount.value = Number(overview.total_files || 0);
    totalReclaimableBytes.value = Number(overview.total_reclaimable_bytes || 0);
  } catch (error) {
    console.error('refreshOverview error:', error);
  }
}

async function fetchGroups(preferredGroupId: number | null = null) {
  try {
    const groups = await dedupListGroups(1, DEDUP_GROUP_LIMIT, 'size_desc', 'all');
    const normalized = Array.isArray(groups) ? groups : [];
    await hydrateAlbumRootPaths(normalized);
    await hydrateGroupThumbnails(normalized);
    rawGroups.value = normalized;
    await refreshOverview();

    const available = new Set(rawGroups.value.map((group: any) => Number(group.id)));
    for (const key of Array.from(selectedDupIdsByGroup.value.keys())) {
      if (!available.has(key)) {
        selectedDupIdsByGroup.value.delete(key);
      }
    }

    if (preferredGroupId && rawGroups.value.some((group: any) => group.id === preferredGroupId)) {
      selectedGroupId.value = preferredGroupId;
      return;
    }

    if (selectedGroupId.value && rawGroups.value.some((group: any) => group.id === selectedGroupId.value)) {
      return;
    }

    selectedGroupId.value = rawGroups.value.length > 0 ? Number(rawGroups.value[0].id) : null;
  } catch (error) {
    console.error('fetchGroups error:', error);
    rawGroups.value = [];
    selectedGroupId.value = null;
  }
}

function stopDedupStatusPolling() {
  if (dedupStatusPollTimer.value) {
    clearInterval(dedupStatusPollTimer.value);
    dedupStatusPollTimer.value = null;
  }
}

async function handleDedupScanSettled() {
  stopDedupStatusPolling();
  isDedupLoading.value = false;
  await fetchGroups();
  if (queuedScanKey.value && queuedScanKey.value !== dedupPane2GlobalState.lastScanKey) {
    queuedScanKey.value = '';
    await triggerBackendDedup(true);
  }
}

function ensureDedupStatusPolling() {
  if (dedupStatusPollTimer.value) return;

  dedupStatusPollTimer.value = setInterval(async () => {
    if (isPollingDedupStatus.value) return;

    isPollingDedupStatus.value = true;
    try {
      const status = await dedupGetScanStatus();
      totalGroupCount.value = Math.max(Number(status?.groups || 0), rawGroups.value.length);
      if (status?.state && status.state !== 'running') {
        await handleDedupScanSettled();
      }
    } catch (error) {
      console.error('ensureDedupStatusPolling error:', error);
    } finally {
      isPollingDedupStatus.value = false;
    }
  }, 1000);
}

async function triggerBackendDedup(force = false) {
  if (!props.dedupScanKey) {
    stopDedupStatusPolling();
    isDedupLoading.value = false;
    return;
  }

  if (!force && dedupPane2GlobalState.lastScanKey === props.dedupScanKey) {
    isDedupLoading.value = true;
    const status = await dedupGetScanStatus();
    totalGroupCount.value = Math.max(Number(status?.groups || 0), rawGroups.value.length);
    if (status?.state === 'running') {
      queuedScanKey.value = props.dedupScanKey;
      ensureDedupStatusPolling();
      return;
    }
    await fetchGroups();
    isDedupLoading.value = false;
    return;
  }

  isDedupLoading.value = true;

  try {
    const status = await dedupGetScanStatus();
    totalGroupCount.value = Math.max(Number(status?.groups || 0), rawGroups.value.length);
    if (status?.state === 'running') {
      queuedScanKey.value = props.dedupScanKey;
      ensureDedupStatusPolling();
      return;
    }

    await dedupStartScan(props.dedupQueryParams || null);
    dedupPane2GlobalState.lastScanKey = props.dedupScanKey;

    const latest = await dedupGetScanStatus();
    totalGroupCount.value = Math.max(Number(latest?.groups || 0), rawGroups.value.length);
    if (latest?.state === 'running') {
      ensureDedupStatusPolling();
    } else {
      await handleDedupScanSettled();
    }
  } catch (error) {
    console.error('triggerBackendDedup error:', error);
    stopDedupStatusPolling();
    isDedupLoading.value = false;
  }
}

watch(
  () => props.dedupScanKey,
  async (newKey, oldKey) => {
    if (!newKey) {
      stopDedupStatusPolling();
      isDedupLoading.value = false;
      rawGroups.value = [];
      selectedGroupId.value = null;
      totalGroupCount.value = 0;
      totalDuplicateFileCount.value = 0;
      totalReclaimableBytes.value = 0;
      return;
    }
    isDedupLoading.value = true;
    if (newKey !== oldKey) {
      await triggerBackendDedup();
      return;
    }
    await fetchGroups();
    isDedupLoading.value = false;
  },
  { immediate: true }
);

watch(filteredDuplicateGroups, (groups) => {
  if (groups.length === 0) {
    selectedGroupId.value = null;
    return;
  }
  if (!selectedGroupId.value || !groups.some((group: any) => group.id === selectedGroupId.value)) {
    selectedGroupId.value = Number(groups[0].id);
  }
});

watch(albumOptions, (albums) => {
  if (deleteAlbumId.value && !albums.some(album => String(album.id) === deleteAlbumId.value)) {
    deleteAlbumId.value = '';
  }
});

watch(deleteAlbumId, () => {
  clearSelection();
});

watch(selectedGroupId, (groupId, prevGroupId) => {
  if (!groupId || groupId === prevGroupId) return;
  const group = filteredDuplicateGroups.value.find((item: any) => item.id === groupId);
  const keepId = group?.keepItem?.file_id;
  if (keepId) {
    emit('select-file', keepId);
  }
});

onMounted(async () => {
  if (props.dedupScanKey) {
    isDedupLoading.value = true;
  }
  const status = await dedupGetScanStatus();
  await refreshOverview();
  if (status?.state === 'running') {
    isDedupLoading.value = true;
    ensureDedupStatusPolling();
  } else if (props.dedupScanKey) {
    await fetchGroups();
  }

  unlistenDedupProgress.value = await listenDedupScanProgress(async (event: any) => {
    const state = event?.payload?.state;
    totalGroupCount.value = Math.max(Number(event?.payload?.groups || 0), totalGroupCount.value);
    if (state === 'running') {
      isDedupLoading.value = true;
      ensureDedupStatusPolling();
      return;
    }

    if (state === 'finished' || state === 'idle' || state === 'error') {
      await handleDedupScanSettled();
    }
  });
});

onUnmounted(() => {
  stopDedupStatusPolling();
  if (unlistenDedupProgress.value) {
    unlistenDedupProgress.value();
    unlistenDedupProgress.value = null;
  }
});
</script>
