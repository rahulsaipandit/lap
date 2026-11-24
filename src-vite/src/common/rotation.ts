// RP-CUSTOM: Rotate-left / save-rotation-to-file helper; entire file is fork-only.
import { ask, message } from '@tauri-apps/plugin-dialog';
import { i18n } from '@/common/i18n';
import { applyFileRotation, canApplyFileRotation } from '@/common/api';
import { useToast } from '@/common/toast';

const toast = useToast();

/**
 * Ask the user whether to save a file's viewing rotation into the file (EXIF
 * Orientation), and do so if they agree. Warns and does nothing when the
 * format can't store it. Returns the refreshed file record on success,
 * otherwise null.
 */
export async function confirmAndSaveRotation(fileId: number): Promise<any | null> {
  const t = (key: string) => i18n.global.t(key) as string;

  if (!(await canApplyFileRotation(fileId))) {
    await message(t('msgbox.save_rotation.unsupported_message'), {
      title: t('msgbox.save_rotation.unsupported_title'),
      kind: 'warning',
    });
    return null;
  }

  const confirmed = await ask(t('msgbox.save_rotation.message'), {
    title: t('msgbox.save_rotation.title'),
    kind: 'info',
    okLabel: t('msgbox.save_rotation.ok'),
    cancelLabel: t('msgbox.cancel'),
  });
  if (!confirmed) return null;

  try {
    const file = await applyFileRotation(fileId);
    toast.success(t('msgbox.save_rotation.success'));
    return file ?? null;
  } catch (error) {
    console.error('Failed to save rotation:', error);
    toast.error(`${t('msgbox.save_rotation.failed')}: ${error}`);
    return null;
  }
}
