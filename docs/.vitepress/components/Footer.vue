<script setup>
import { withBase } from "vitepress";

const COLUMNS = [
  {
    title: "Guide",
    links: [
      { text: "Why Smuggle?", to: "/guide/why" },
      { text: "Getting Started", to: "/guide/getting-started" },
    ],
  },
  {
    title: "Reference",
    links: [
      { text: "Commands", to: "/reference/commands" },
      { text: "Flags", to: "/reference/flags" },
      { text: "llms.txt", to: "/llms.txt", external: true },
    ],
  },
  {
    title: "Project",
    links: [
      { text: "GitHub", href: "https://github.com/logaretm/smuggle", external: true },
      { text: "Issues", href: "https://github.com/logaretm/smuggle/issues", external: true },
      { text: "Releases", href: "https://github.com/logaretm/smuggle/releases", external: true },
    ],
  },
];

const year = new Date().getFullYear();
</script>

<template>
  <footer class="sm-footer">
    <div class="sm-footer-bg" aria-hidden="true">
      <svg viewBox="0 0 600 200" preserveAspectRatio="none">
        <defs>
          <linearGradient id="ftr-glow" x1="50%" y1="0%" x2="50%" y2="100%">
            <stop offset="0%" stop-color="#d2691e" stop-opacity="0.18" />
            <stop offset="100%" stop-color="#d2691e" stop-opacity="0" />
          </linearGradient>
        </defs>
        <ellipse cx="300" cy="0" rx="500" ry="120" fill="url(#ftr-glow)" />
      </svg>
    </div>

    <div class="sm-footer-inner">
      <div class="sm-footer-grid">
        <div class="sm-footer-brand">
          <div class="sm-footer-mark">
            <svg viewBox="0 0 128 128" aria-hidden="true">
              <defs>
                <linearGradient id="ftr-front" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0" stop-color="#d2691e" />
                  <stop offset="1" stop-color="#a0360b" />
                </linearGradient>
                <linearGradient id="ftr-top" x1="0" y1="1" x2="0" y2="0">
                  <stop offset="0" stop-color="#cd853f" />
                  <stop offset="1" stop-color="#e8a070" />
                </linearGradient>
                <linearGradient id="ftr-right" x1="0" y1="0" x2="1" y2="0">
                  <stop offset="0" stop-color="#8b2f0a" />
                  <stop offset="1" stop-color="#6b2407" />
                </linearGradient>
                <linearGradient id="ftr-tape" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0" stop-color="#a0360b" />
                  <stop offset="1" stop-color="#7a2807" />
                </linearGradient>
              </defs>
              <ellipse cx="68" cy="118" rx="48" ry="5" fill="#000" opacity="0.22" />
              <path d="M96 48 L96 112 L112 96 L112 32 Z" fill="url(#ftr-right)" stroke="#4a1505" stroke-width="2" stroke-linejoin="round" />
              <path d="M24 48 L40 32 L112 32 L96 48 Z" fill="url(#ftr-top)" stroke="#4a1505" stroke-width="2" stroke-linejoin="round" />
              <path d="M52 48 L68 32 L84 32 L68 48 Z" fill="#b7410e" stroke="#4a1505" stroke-width="1.2" stroke-linejoin="round" opacity="0.85" />
              <path d="M24 48 L96 48 L96 112 L24 112 Z" fill="url(#ftr-front)" stroke="#4a1505" stroke-width="2" stroke-linejoin="round" />
              <path d="M52 48 L68 48 L68 112 L52 112 Z" fill="url(#ftr-tape)" stroke="#4a1505" stroke-width="1.2" stroke-linejoin="round" opacity="0.85" />
              <g stroke="#fff5e6" stroke-width="5.5" stroke-linecap="round" stroke-linejoin="round" fill="none">
                <path d="M50 64 Q42 64 42 71 L42 77 Q42 80 36 80 Q42 80 42 83 L42 89 Q42 96 50 96" />
                <path d="M70 64 Q78 64 78 71 L78 77 Q78 80 84 80 Q78 80 78 83 L78 89 Q78 96 70 96" />
              </g>
            </svg>
          </div>
          <div class="sm-footer-brand-text">
            <div class="sm-footer-brand-name">smuggle</div>
            <div class="sm-footer-brand-tag">Local npm packages.</div>
          </div>
        </div>

        <div
          v-for="col in COLUMNS"
          :key="col.title"
          class="sm-footer-col"
        >
          <div class="sm-footer-col-title">{{ col.title }}</div>
          <ul>
            <li v-for="link in col.links" :key="link.text">
              <a
                v-if="link.href"
                :href="link.href"
                :rel="link.external ? 'noopener' : null"
                :target="link.external ? '_blank' : null"
              >
                {{ link.text }}
                <svg v-if="link.external" class="sm-ext" viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M6 3 H3 V13 H13 V10 M9 3 H13 V7 M7.5 8.5 L13 3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
              </a>
              <a
                v-else
                :href="withBase(link.to)"
                :rel="link.external ? 'noopener' : null"
                :target="link.external ? '_blank' : null"
              >
                {{ link.text }}
                <svg v-if="link.external" class="sm-ext" viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M6 3 H3 V13 H13 V10 M9 3 H13 V7 M7.5 8.5 L13 3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
              </a>
            </li>
          </ul>
        </div>
      </div>

      <div class="sm-footer-strip">
        <div class="sm-footer-meta">
          <span>© {{ year }} smuggle</span>
          <span class="sm-dot" aria-hidden="true">•</span>
          <a href="https://github.com/logaretm/smuggle/blob/main/LICENSE" rel="noopener" target="_blank">MIT License</a>
        </div>
        <div class="sm-footer-tag">
          <span class="sm-cmd">$</span>
          <span class="sm-cmd-text">crafted with rust &amp; care</span>
          <span class="sm-cursor" />
        </div>
      </div>
    </div>
  </footer>
</template>

<style scoped>
.sm-footer {
  position: relative;
  margin-top: 4rem;
  padding: 0;
  border-top: 1px solid var(--vp-c-divider);
  background: linear-gradient(180deg, var(--vp-c-bg) 0%, var(--vp-c-bg-soft) 100%);
  overflow: hidden;
}

.dark .sm-footer {
  background: linear-gradient(180deg, #110e0c 0%, #1a1410 100%);
  border-top-color: rgba(210, 105, 30, 0.18);
}

.sm-footer-bg {
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 0;
}

.sm-footer-bg svg { width: 100%; height: 100%; display: block; }

.sm-footer-inner {
  position: relative;
  z-index: 1;
  max-width: 1152px;
  margin: 0 auto;
  padding: 3rem 1.5rem 1.5rem;
}

.sm-footer-grid {
  display: grid;
  grid-template-columns: 1.6fr repeat(3, 1fr);
  gap: 2.5rem;
  margin-bottom: 2.5rem;
}

@media (max-width: 760px) {
  .sm-footer-grid {
    grid-template-columns: 1fr 1fr;
    gap: 1.75rem;
  }
}

@media (max-width: 480px) {
  .sm-footer-grid { grid-template-columns: 1fr; }
}

.sm-footer-brand {
  display: flex;
  align-items: flex-start;
  gap: 1rem;
}

@media (max-width: 760px) {
  .sm-footer-brand { grid-column: 1 / -1; }
}

.sm-footer-mark {
  flex-shrink: 0;
  width: 64px;
  height: 64px;
  filter: drop-shadow(0 8px 20px rgba(210, 105, 30, 0.35));
  transition: transform 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

.sm-footer-mark svg { width: 100%; height: 100%; }

.sm-footer-mark:hover { transform: rotate(-3deg) scale(1.04); }

.sm-footer-brand-name {
  font-size: 1.6rem;
  font-weight: 800;
  letter-spacing: -0.02em;
  background: linear-gradient(135deg, #b7410e, #d2691e, #8b4513);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
  margin-bottom: 0.25rem;
}

.sm-footer-brand-tag {
  font-size: 0.85rem;
  color: var(--vp-c-text-2);
  line-height: 1.5;
  max-width: 280px;
}

.sm-footer-col-title {
  font-family: var(--vp-font-family-mono);
  font-size: 0.7rem;
  font-weight: 700;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: var(--sm-rust-2);
  margin-bottom: 0.85rem;
}

.sm-footer-col ul {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
}

.sm-footer-col a {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.88rem;
  color: var(--vp-c-text-2);
  text-decoration: none !important;
  transition: color 0.2s ease, transform 0.2s ease;
}

.sm-footer-col a:hover {
  color: var(--sm-rust-2);
  transform: translateX(2px);
}

.sm-ext {
  width: 10px;
  height: 10px;
  opacity: 0.6;
}

.sm-footer-strip {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding-top: 1.25rem;
  border-top: 1px solid var(--vp-c-divider);
}

.sm-footer-meta {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.78rem;
  color: var(--vp-c-text-3);
  font-family: var(--vp-font-family-mono);
}

.sm-footer-meta a {
  color: var(--vp-c-text-2);
  text-decoration: none !important;
  transition: color 0.2s ease;
}

.sm-footer-meta a:hover { color: var(--sm-rust-2); }

.sm-dot { color: var(--vp-c-text-3); }

.sm-footer-tag {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-family: var(--vp-font-family-mono);
  font-size: 0.78rem;
  color: var(--vp-c-text-3);
  padding: 0.35rem 0.7rem;
  border-radius: 6px;
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-divider);
}

.sm-cmd {
  color: var(--sm-rust-2);
  font-weight: 700;
}

.sm-cmd-text { color: var(--vp-c-text-2); }

.sm-cursor {
  display: inline-block;
  width: 6px;
  height: 0.95em;
  background: var(--sm-rust-2);
  margin-left: 2px;
  animation: sm-blink 1s steps(1) infinite;
  vertical-align: text-bottom;
}

@keyframes sm-blink {
  50% { opacity: 0; }
}

@media (prefers-reduced-motion: reduce) {
  .sm-cursor { animation: none; }
  .sm-footer-mark:hover { transform: none; }
}
</style>
