<script setup>
defineProps({
  kind: { type: String, default: "tip" }, // tip | warn | info | danger
  title: { type: String, default: "" },
});
</script>

<template>
  <div :class="['callout', `callout-${kind}`]">
    <div class="callout-glyph" aria-hidden="true">
      <svg v-if="kind === 'tip'" viewBox="0 0 20 20"><path d="M10 2 a6 6 0 0 1 6 6 c0 2.5 -1.5 4 -2.5 5 v2 h-7 v-2 c-1 -1 -2.5 -2.5 -2.5 -5 a6 6 0 0 1 6 -6 z M7 17 h6" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
      <svg v-else-if="kind === 'warn'" viewBox="0 0 20 20"><path d="M10 2 L18 17 H2 Z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" /><path d="M10 8 V12 M10 14.5 V14.6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg>
      <svg v-else-if="kind === 'danger'" viewBox="0 0 20 20"><circle cx="10" cy="10" r="7.5" fill="none" stroke="currentColor" stroke-width="1.6" /><path d="M10 6 V11 M10 13.5 V13.6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg>
      <svg v-else viewBox="0 0 20 20"><circle cx="10" cy="10" r="7.5" fill="none" stroke="currentColor" stroke-width="1.6" /><path d="M10 9 V14 M10 6.5 V6.6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg>
    </div>
    <div class="callout-body">
      <div v-if="title" class="callout-title">{{ title }}</div>
      <div class="callout-text"><slot /></div>
    </div>
  </div>
</template>

<style scoped>
.callout {
  display: flex;
  gap: 0.85rem;
  padding: 0.95rem 1.1rem;
  border-radius: 10px;
  margin: 1.25rem 0;
  border: 1px solid var(--vp-c-divider);
  background: var(--vp-c-bg-soft);
  position: relative;
}

.callout::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  border-radius: 10px 0 0 10px;
}

.callout-tip { border-color: rgba(210, 105, 30, 0.35); background: linear-gradient(180deg, rgba(210, 105, 30, 0.06), rgba(210, 105, 30, 0.02)); }
.callout-tip::before { background: linear-gradient(180deg, #d2691e, #b7410e); }
.callout-tip .callout-glyph { color: #d2691e; }

.callout-warn { border-color: rgba(255, 189, 46, 0.35); background: linear-gradient(180deg, rgba(255, 189, 46, 0.05), rgba(255, 189, 46, 0.02)); }
.callout-warn::before { background: linear-gradient(180deg, #ffbd2e, #d18a00); }
.callout-warn .callout-glyph { color: #d18a00; }
.dark .callout-warn .callout-glyph { color: #ffbd2e; }

.callout-info { border-color: rgba(100, 160, 220, 0.35); background: linear-gradient(180deg, rgba(100, 160, 220, 0.05), rgba(100, 160, 220, 0.02)); }
.callout-info::before { background: linear-gradient(180deg, #6aa6dc, #3d7bb0); }
.callout-info .callout-glyph { color: #3d7bb0; }
.dark .callout-info .callout-glyph { color: #88bbe6; }

.callout-danger { border-color: rgba(220, 80, 80, 0.4); background: linear-gradient(180deg, rgba(220, 80, 80, 0.05), rgba(220, 80, 80, 0.02)); }
.callout-danger::before { background: linear-gradient(180deg, #ff5f56, #c44); }
.callout-danger .callout-glyph { color: #c44; }
.dark .callout-danger .callout-glyph { color: #ff8888; }

.callout-glyph {
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  margin-top: 1px;
}

.callout-glyph svg { width: 100%; height: 100%; }

.callout-body { flex: 1; min-width: 0; }

.callout-title {
  font-weight: 600;
  font-size: 0.92rem;
  letter-spacing: -0.005em;
  margin-bottom: 0.2rem;
  color: var(--vp-c-text-1);
}

.callout-text :deep(p) { margin: 0.4rem 0; line-height: 1.6; font-size: 0.9rem; color: var(--vp-c-text-2); }
.callout-text :deep(p:first-child) { margin-top: 0; }
.callout-text :deep(p:last-child) { margin-bottom: 0; }
.callout-text :deep(code) { font-size: 0.85em; }
</style>
