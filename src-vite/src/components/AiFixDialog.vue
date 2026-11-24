<template>
  <ModalDialog title="Fix with AI" :width="440" @cancel="clickCancel">
    <section class="space-y-3">
      <div class="text-xs text-base-content/50 leading-relaxed">
        Sends {{ fileIds.length }} photo{{ fileIds.length === 1 ? '' : 's' }} to the PromptFix
        service running at <span class="font-mono">{{ apiBaseUrl }}</span>. Pick a quick task, or
        type your own instruction below.
      </div>

      <div class="grid grid-cols-2 gap-1.5">
        <button
          v-for="taskOption in taskOptions"
          :key="taskOption.value"
          class="btn btn-sm btn-outline justify-start"
          :disabled="isApplying"
          @click="applyTask(taskOption.value)"
        >
          {{ taskOption.label }}
        </button>
      </div>

      <div class="space-y-1.5">
        <div class="text-xs font-bold uppercase tracking-wide text-base-content/55">
          Custom instruction (also use for object removal)
        </div>
        <textarea
          v-model="customInstruction"
          class="textarea textarea-sm w-full bg-base-100"
          rows="2"
          placeholder="e.g. Exclude the individual on the left, Scrub logo off image, Optimize the visual quality..."
          :disabled="isApplying"
        ></textarea>
        <button
          class="btn btn-sm btn-primary w-full"
          :disabled="isApplying || !customInstruction.trim()"
          @click="applyCustom"
        >
          <span v-if="isApplying" class="loading loading-spinner loading-xs"></span>
          Apply
        </button>
      </div>

      <div v-if="isApplying" class="text-[11px] text-base-content/50">
        Fixing {{ fixStatus.current + 1 }}/{{ fixStatus.total }}: {{ fixStatus.currentFile }}
      </div>
      <div v-if="errorMessage" class="text-[11px] text-error leading-relaxed">
        {{ errorMessage }}
      </div>
    </section>

    <div class="mt-4 flex justify-end">
      <button class="t-button-default" :disabled="isApplying" @click="clickCancel">
        {{ isApplying ? $t('msgbox.close') : $t('msgbox.cancel') }}
      </button>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
  // RP-CUSTOM: PromptFix AI-fix dialog; entire file is fork-only.
import { ref, onMounted, onBeforeUnmount } from 'vue';
import ModalDialog from '@/components/ModalDialog.vue';
import { blurStartFixBatch, blurGetFixStatus, listenBlurFixProgress } from '@/common/api';
import { config } from '@/common/config';
import { useUIStore } from '@/stores/uiStore';

const props = defineProps({
  fileIds: {
    type: Array as () => number[],
    default: () => [],
  },
});

const emit = defineEmits(['applied', 'cancel']);
const uiStore = useUIStore();

const taskOptions = [
  { value: 'colorize', label: '🎨 Colorize' },
  { value: 'dehaze', label: '🌫️ Dehaze' },
  { value: 'deblur', label: '💨 Deblur' },
  { value: 'dewatermark', label: '🖼️ Remove Watermark' },
  { value: 'desnow', label: '❄️ Remove Snow' },
  { value: 'lowlight', label: '🌙 Enhance Lighting' },
];

const apiBaseUrl = config.settings.blurFix.apiBaseUrl;
const customInstruction = ref('');
const isApplying = ref(false);
const errorMessage = ref('');
const fixStatus = ref<any>({ current: 0, total: 0, currentFile: '' });
const unlistenFixProgress = ref<null | (() => void)>(null);

onMounted(async () => {
  uiStore.pushInputHandler('AiFixDialog');
  unlistenFixProgress.value = await listenBlurFixProgress((event: any) => {
    fixStatus.value = event?.payload || fixStatus.value;
  });
});

onBeforeUnmount(() => {
  uiStore.removeInputHandler('AiFixDialog');
  if (unlistenFixProgress.value) unlistenFixProgress.value();
});

async function runFix(task: string | null, instruction: string | null) {
  if (props.fileIds.length === 0 || isApplying.value) return;
  errorMessage.value = '';
  isApplying.value = true;
  try {
    await blurStartFixBatch(props.fileIds, apiBaseUrl, task, instruction);
    await pollUntilDone();
  } catch (error: any) {
    errorMessage.value = String(error?.message || error);
    isApplying.value = false;
  }
}

async function pollUntilDone() {
  while (true) {
    const status = await blurGetFixStatus();
    fixStatus.value = status;
    if (!status?.isFixing) {
      if (status?.error) errorMessage.value = status.error;
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  isApplying.value = false;
  if (!errorMessage.value) {
    emit('applied');
  }
}

function applyTask(task: string) {
  runFix(task, null);
}

function applyCustom() {
  const instruction = customInstruction.value.trim();
  if (!instruction) return;
  runFix(null, instruction);
}

function clickCancel() {
  emit('cancel');
}
</script>
