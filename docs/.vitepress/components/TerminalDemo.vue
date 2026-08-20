<script setup>
import { ref, onMounted, onUnmounted } from "vue";

const SCRIPT = [
  { kind: "cmd", text: "smuggle setup", typing: true, delay: 95, pause: 250 },
  { kind: "ok", text: "\u2713 trusted local CA, installed daemon", delay: 400 },
  { kind: "blank", text: "", delay: 320 },
  { kind: "cmd", text: "cd ~/projects/my-package", typing: true, delay: 80 },
  { kind: "cmd", text: "smuggle publish", typing: true, delay: 95, pause: 220 },
  { kind: "info", text: "\u2192 npm pack ./my-package", delay: 360 },
  { kind: "ok", text: "\u2713 stored my-package@1.0.0 in ~/.smuggle", delay: 380 },
  { kind: "blank", text: "", delay: 380 },
  { kind: "cmd", text: "cd ~/projects/my-app", typing: true, delay: 80 },
  { kind: "cmd", text: "smuggle", typing: true, delay: 95, pause: 250 },
  { kind: "muted", text: "intercepting registry.npmjs.org", delay: 340 },
  { kind: "muted", text: "pinned 1 lockfile entry", delay: 320 },
  { kind: "info", text: "\u2192 npm install", delay: 380 },
  { kind: "ok", text: "\u2713 served my-package from the store", delay: 420 },
];

const lines = ref([]);
const typed = ref("");
const isTyping = ref(false);
const showCursor = ref(true);
const visible = ref(false);
const root = ref(null);

let aborted = false;

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function play() {
  while (!aborted) {
    lines.value = [];
    for (const step of SCRIPT) {
      if (aborted) return;
      if (step.kind === "cmd" && step.typing) {
        isTyping.value = true;
        typed.value = "";
        for (const ch of step.text) {
          if (aborted) return;
          typed.value += ch;
          await sleep(20 + Math.random() * 40);
        }
        if (step.pause) await sleep(step.pause);
        lines.value.push({ kind: "cmd", text: step.text });
        typed.value = "";
        isTyping.value = false;
      } else {
        lines.value.push({ kind: step.kind, text: step.text });
      }
      await sleep(step.delay || 200);
    }
    await sleep(2200);
  }
}

let observer = null;

onMounted(() => {
  observer = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (e.isIntersecting && !visible.value) {
          visible.value = true;
          play();
        }
      }
    },
    { threshold: 0.2 },
  );
  if (root.value) observer.observe(root.value);
});

onUnmounted(() => {
  aborted = true;
  if (observer) observer.disconnect();
});
</script>

<template>
  <div ref="root" class="td-wrap">
    <div class="td">
      <div class="td-bar">
        <span class="td-dot td-r" />
        <span class="td-dot td-y" />
        <span class="td-dot td-g" />
        <span class="td-title">~ · smuggle</span>
        <span class="td-tag">live demo</span>
      </div>
      <div class="td-body">
        <div
          v-for="(line, idx) in lines"
          :key="idx"
          :class="['td-line', `td-${line.kind}`]"
        >
          <span v-if="line.kind === 'cmd'" class="td-prompt">$</span>
          <span v-else-if="line.kind !== 'blank'" class="td-prompt-spacer" />
          <span class="td-text">{{ line.text }}</span>
        </div>
        <div v-if="isTyping" class="td-line td-cmd">
          <span class="td-prompt">$</span>
          <span class="td-text">{{ typed }}</span>
          <span class="td-cursor" :class="{ blink: !typed }" />
        </div>
        <div v-else class="td-line td-cmd td-idle">
          <span class="td-prompt">$</span>
          <span class="td-cursor blink" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.td-wrap {
  max-width: 880px;
  margin: 1.5rem auto 0;
  padding: 0 1rem;
}

.td {
  position: relative;
  border-radius: 14px;
  overflow: hidden;
  background: #1a120e;
  border: 1px solid rgba(210, 105, 30, 0.18);
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.04) inset,
    0 24px 60px -24px rgba(139, 69, 19, 0.4),
    0 8px 20px -8px rgba(0, 0, 0, 0.3);
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
}

.td::before {
  content: "";
  position: absolute;
  inset: -1px;
  border-radius: 14px;
  padding: 1px;
  background: linear-gradient(
    135deg,
    rgba(210, 105, 30, 0.5),
    rgba(210, 105, 30, 0.05) 30%,
    rgba(139, 69, 19, 0.05) 60%,
    rgba(210, 105, 30, 0.4)
  );
  -webkit-mask:
    linear-gradient(#000 0 0) content-box,
    linear-gradient(#000 0 0);
  -webkit-mask-composite: xor;
  mask-composite: exclude;
  pointer-events: none;
  opacity: 0.6;
}

.td-bar {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.55rem 0.85rem;
  background: linear-gradient(180deg, #2a1d16, #1f1611);
  border-bottom: 1px solid rgba(210, 105, 30, 0.14);
}

.td-dot {
  width: 11px;
  height: 11px;
  border-radius: 999px;
}
.td-r { background: #ff5f56; }
.td-y { background: #ffbd2e; }
.td-g { background: #27c93f; }

.td-title {
  margin-left: 0.6rem;
  font-size: 0.78rem;
  color: #b8a89a;
  letter-spacing: 0.02em;
}

.td-tag {
  margin-left: auto;
  font-size: 0.65rem;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: #d2691e;
  padding: 0.15rem 0.5rem;
  border-radius: 4px;
  background: rgba(210, 105, 30, 0.12);
  border: 1px solid rgba(210, 105, 30, 0.3);
}

.td-body {
  padding: 1rem 1.1rem 1.1rem;
  min-height: 240px;
  font-size: 0.85rem;
  line-height: 1.7;
  color: #e8d9c8;
}

.td-line {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  white-space: pre;
  animation: td-line-in 0.25s ease;
}

.td-prompt {
  color: #d2691e;
  font-weight: 700;
  flex-shrink: 0;
}

.td-prompt-spacer {
  display: inline-block;
  width: 0.55rem;
  flex-shrink: 0;
}

.td-cmd .td-text { color: #fff5e6; }
.td-info .td-text { color: #d6c1a8; }
.td-muted .td-text { color: #8b7864; }
.td-ok .td-text { color: #7fdc7e; }
.td-warn .td-text { color: #ffbd2e; }
.td-blank { height: 0.4rem; }

.td-cursor {
  display: inline-block;
  width: 8px;
  height: 1.05em;
  margin-left: 2px;
  background: #d2691e;
  vertical-align: text-bottom;
}

.td-cursor.blink { animation: td-blink 1s steps(1) infinite; }

.td-idle { opacity: 0.7; }

@keyframes td-line-in {
  from {
    opacity: 0;
    transform: translateY(4px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

@keyframes td-blink {
  50% { opacity: 0; }
}

@media (max-width: 540px) {
  .td-body { font-size: 0.78rem; padding: 0.85rem; }
  .td-title { display: none; }
}
</style>
