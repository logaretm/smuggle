<script setup>
const FEATURES = [
  {
    title: "No symlinks",
    detail: "Real files in node_modules, just like a normal install.",
    accent: "rust",
  },
  {
    title: "No lockfile churn",
    detail: "pnpm-lock.yaml, package-lock.json, yarn.lock — untouched.",
    accent: "bronze",
  },
  {
    title: "Automatic cleanup",
    detail: "Originals are restored on exit. Even on Ctrl-C.",
    accent: "rust",
  },
  {
    title: "Instant feedback",
    detail: "File watcher re-packs and re-swaps on every change.",
    accent: "bronze",
  },
];
</script>

<template>
  <div class="hero-features">
    <div
      v-for="(f, i) in FEATURES"
      :key="f.title"
      :class="['hf-card', `hf-${f.accent}`]"
      :style="{ '--i': i }"
    >
      <div class="hf-icon">
        <!-- No symlinks: broken chain -->
        <svg v-if="i === 0" viewBox="0 0 40 40" aria-hidden="true">
          <defs>
            <linearGradient :id="`hf-grad-${i}`" x1="0" y1="0" x2="1" y2="1">
              <stop offset="0" stop-color="#d2691e" />
              <stop offset="1" stop-color="#b7410e" />
            </linearGradient>
          </defs>
          <g fill="none" :stroke="`url(#hf-grad-${i})`" stroke-width="2.4" stroke-linecap="round">
            <path d="M14 14 a6 6 0 0 0 -6 6 v2 a6 6 0 0 0 4 5.6" />
            <path d="M26 26 a6 6 0 0 0 6 -6 v-2 a6 6 0 0 0 -4 -5.6" />
            <path d="M16 16 l8 8" stroke-dasharray="2 3" />
          </g>
          <path d="M9 31 L31 9" stroke="#b7410e" stroke-width="2.5" stroke-linecap="round" opacity="0.85" />
        </svg>

        <!-- No lockfile changes: lock with check -->
        <svg v-else-if="i === 1" viewBox="0 0 40 40" aria-hidden="true">
          <defs>
            <linearGradient :id="`hf-grad-${i}`" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" stop-color="#cd853f" />
              <stop offset="1" stop-color="#8b4513" />
            </linearGradient>
          </defs>
          <path d="M14 18 v-3 a6 6 0 0 1 12 0 v3" fill="none" :stroke="`url(#hf-grad-${i})`" stroke-width="2.4" stroke-linecap="round" />
          <rect x="10" y="18" width="20" height="14" rx="3" :fill="`url(#hf-grad-${i})`" stroke="#6b3410" stroke-width="0.8" />
          <path d="M16 25 L19 28 L25 22" fill="none" stroke="#fff5e6" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" />
        </svg>

        <!-- Automatic cleanup: circular arrow -->
        <svg v-else-if="i === 2" viewBox="0 0 40 40" aria-hidden="true">
          <defs>
            <linearGradient :id="`hf-grad-${i}`" x1="0" y1="0" x2="1" y2="1">
              <stop offset="0" stop-color="#d2691e" />
              <stop offset="1" stop-color="#b7410e" />
            </linearGradient>
          </defs>
          <path d="M30 14 a12 12 0 1 0 3 8" fill="none" :stroke="`url(#hf-grad-${i})`" stroke-width="2.6" stroke-linecap="round" />
          <path d="M30 8 L30 15 L23 15" fill="none" :stroke="`url(#hf-grad-${i})`" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" />
          <circle cx="20" cy="20" r="3.5" fill="#f5deb3" stroke="#8b4513" stroke-width="1" />
        </svg>

        <!-- Instant feedback: lightning -->
        <svg v-else viewBox="0 0 40 40" aria-hidden="true">
          <defs>
            <linearGradient :id="`hf-grad-${i}`" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" stop-color="#d2691e" />
              <stop offset="1" stop-color="#b7410e" />
            </linearGradient>
          </defs>
          <path d="M22 6 L10 22 L18 22 L16 34 L30 16 L22 16 Z" :fill="`url(#hf-grad-${i})`" stroke="#6b3410" stroke-width="0.8" stroke-linejoin="round" />
        </svg>
      </div>
      <div class="hf-text">
        <div class="hf-title">{{ f.title }}</div>
        <div class="hf-detail">{{ f.detail }}</div>
      </div>
      <div class="hf-shine" />
    </div>
  </div>
</template>

<style scoped>
.hero-features {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 0.85rem;
  max-width: 1152px;
  margin: 1rem auto 2rem;
  padding: 0 24px;
}

@media (max-width: 960px) {
  .hero-features { grid-template-columns: repeat(2, 1fr); }
}

@media (max-width: 540px) {
  .hero-features { grid-template-columns: 1fr; padding: 0 16px; }
}

.hf-card {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: 0.85rem;
  padding: 1.1rem 1rem;
  border-radius: 14px;
  background: linear-gradient(180deg, var(--vp-c-bg) 0%, var(--vp-c-bg-soft) 100%);
  border: 1px solid var(--vp-c-divider);
  overflow: hidden;
  transition:
    transform 0.4s cubic-bezier(0.4, 0, 0.2, 1),
    border-color 0.3s ease,
    box-shadow 0.3s ease;
  animation: hf-in 0.6s cubic-bezier(0.4, 0, 0.2, 1) backwards;
  animation-delay: calc(var(--i) * 70ms);
}

.dark .hf-card {
  background: linear-gradient(180deg, rgba(36, 26, 22, 0.6) 0%, rgba(26, 19, 16, 0.6) 100%);
  border-color: rgba(210, 105, 30, 0.14);
}

.hf-card:hover {
  transform: translateY(-3px);
  border-color: rgba(210, 105, 30, 0.45);
  box-shadow:
    0 0 0 3px rgba(210, 105, 30, 0.06),
    0 18px 40px -16px rgba(183, 65, 14, 0.4);
}

.hf-icon {
  flex-shrink: 0;
  width: 40px;
  height: 40px;
  padding: 6px;
  border-radius: 10px;
  background: linear-gradient(135deg, rgba(210, 105, 30, 0.12), rgba(183, 65, 14, 0.06));
  border: 1px solid rgba(210, 105, 30, 0.2);
  display: grid;
  place-items: center;
  filter: drop-shadow(0 2px 6px rgba(139, 69, 19, 0.15));
  transition: transform 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

.hf-card:hover .hf-icon { transform: scale(1.08) rotate(-3deg); }

.hf-icon svg { width: 100%; height: 100%; }

.hf-text { flex: 1; min-width: 0; }

.hf-title {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--vp-c-text-1);
  margin-bottom: 0.2rem;
  letter-spacing: -0.005em;
}

.hf-detail {
  font-size: 0.82rem;
  color: var(--vp-c-text-2);
  line-height: 1.5;
}

.hf-shine {
  position: absolute;
  inset: 0;
  background: linear-gradient(
    105deg,
    transparent 30%,
    rgba(210, 105, 30, 0.18) 50%,
    transparent 70%
  );
  transform: translateX(-110%);
  transition: transform 0.8s cubic-bezier(0.4, 0, 0.2, 1);
  pointer-events: none;
  mix-blend-mode: overlay;
}

.hf-card:hover .hf-shine { transform: translateX(110%); }

@keyframes hf-in {
  from {
    opacity: 0;
    transform: translateY(12px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .hf-card { animation: none; }
  .hf-card:hover { transform: none; }
  .hf-card:hover .hf-icon { transform: none; }
  .hf-shine { display: none; }
}
</style>
