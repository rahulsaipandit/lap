// RP-CUSTOM: Send-to-Smart-Photos frontend; entire file is fork-only.
import { ask, message } from '@tauri-apps/plugin-dialog';
import { i18n } from '@/common/i18n';
import { config } from '@/common/config';
import { ref, watch, onMounted, onBeforeUnmount, type Ref } from 'vue';
import { getSmartPhotosRootStatus, smartPhotosPreflight, sendToSmartPhotos, checkInSmartPhotos } from '@/common/api';
import { useToast } from '@/common/toast';

const toast = useToast();

/**
 * Copy (never move) the given files and/or folders into the Smart Photos folder
 * set in Settings. The originals are left untouched. Only JPG/JPEG/PNG are sent.
 * Returns true if a send ran.
 */
export async function sendPathsToSmartPhotos(paths: string[]): Promise<boolean> {
  const t = (key: string, params?: Record<string, unknown>) => i18n.global.t(key, params ?? {}) as string;
  const root = String(config.settings.smartPhotosRoot || '').trim();
  paths = paths.filter(Boolean);
  if (!paths.length) return false;

  const status = await getSmartPhotosRootStatus(root);
  if (status !== 'ok') {
    const key = status === 'unset' ? 'no_folder' : status === 'readonly' ? 'readonly' : 'unavailable';
    await message(t(`msgbox.smart_photos.${key}`), { title: t('menu.file.send_to_smart_photos'), kind: 'warning' });
    return false;
  }

  try {
    const pre = await smartPhotosPreflight(paths);
    if (!pre.total) {
      toast.info(t('msgbox.smart_photos.nothing'));
      return false;
    }

    // Photos outside a <year>\<event folder> layout need the user's OK to use the photo's own date.
    let allowDateFallback = false;
    if (pre.needs_fallback > 0) {
      allowDateFallback = await ask(t('msgbox.smart_photos.layout_message', { count: pre.needs_fallback }), {
        title: t('msgbox.smart_photos.layout_title'),
        kind: 'warning',
        okLabel: t('msgbox.smart_photos.layout_ok'),
        cancelLabel: t('msgbox.cancel'),
      });
    }

    const res = await sendToSmartPhotos(paths, root, allowDateFallback);
    const parts = (['copied', 'skipped_exists', 'skipped_layout', 'failed'] as const)
      .filter(k => res[k] > 0)
      .map(k => t(`msgbox.smart_photos.${k}`, { count: res[k] }));
    const text = parts.join(', ') || t('msgbox.smart_photos.nothing');
    if (res.failed > 0) {
      toast.error(`${text}${res.errors?.[0] ? ` — ${res.errors[0]}` : ''}`);
    } else {
      toast.success(text);
    }
    if (res.copied > 0) window.dispatchEvent(new CustomEvent('smart-photos-changed'));
    return true;
  } catch (error) {
    toast.error(`${t('msgbox.smart_photos.failed_prefix')}: ${error}`);
    return false;
  }
}

// ---- "Already in Smart Photos" badge -------------------------------------------------
// Looked up lazily: only thumbnails that are rendered ask, requests are batched into one
// IPC call, and results are cached. The cache is dropped after a send or when the
// Smart Photos folder changes. An offline drive yields no badges (never an error).
const badgeCache = new Map<string, boolean>();
const pending = new Map<string, Array<(v: boolean) => void>>();
let flushTimer: ReturnType<typeof setTimeout> | null = null;
let generation = 0;

function flush() {
  flushTimer = null;
  const root = String(config.settings.smartPhotosRoot || '').trim();
  const batch = new Map(pending);
  pending.clear();
  const paths = [...batch.keys()];
  if (!paths.length) return;
  const gen = generation;
  const resolveAll = (results: boolean[]) => {
    paths.forEach((p, i) => {
      const value = Boolean(results[i]);
      if (gen === generation) badgeCache.set(p, value);
      batch.get(p)?.forEach(cb => cb(value));
    });
  };
  if (!root) return resolveAll([]);
  void checkInSmartPhotos(paths, root).then(resolveAll);
}

function lookup(path: string): Promise<boolean> {
  if (badgeCache.has(path)) return Promise.resolve(badgeCache.get(path)!);
  return new Promise(resolve => {
    const waiters = pending.get(path) ?? [];
    waiters.push(resolve);
    pending.set(path, waiters);
    if (!flushTimer) flushTimer = setTimeout(flush, 80);
  });
}

/** Reactive flag: is an identical copy of this photo already in Smart Photos? */
export function useInSmartPhotos(getPath: () => string | undefined): Ref<boolean> {
  const inSmartPhotos = ref(false);
  let alive = true;
  const refresh = async () => {
    const path = getPath();
    const root = String(config.settings.smartPhotosRoot || '').trim();
    if (!path || !root || !/\.(jpe?g|png)$/i.test(path)) {
      inSmartPhotos.value = false;
      return;
    }
    const value = await lookup(path);
    if (alive && getPath() === path) inSmartPhotos.value = value;
  };
  const invalidate = () => {
    badgeCache.clear();
    generation++;
    void refresh();
  };
  onMounted(() => {
    void refresh();
    window.addEventListener('smart-photos-changed', invalidate);
  });
  onBeforeUnmount(() => {
    alive = false;
    window.removeEventListener('smart-photos-changed', invalidate);
  });
  watch(getPath, () => void refresh());
  watch(() => config.settings.smartPhotosRoot, invalidate);
  return inSmartPhotos;
}
