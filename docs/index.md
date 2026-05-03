---
layout: home
hero:
  name: Smuggle
  text: Local npm packages
  tagline: No symlinks, no lockfile pollution, no registry overrides. Just real files in node_modules.
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: Why Smuggle?
      link: /guide/why
---

<HeroFeatures />

<div class="section-head">
  <span class="section-tag">live</span>
  <h2 class="section-title">See it run</h2>
  <p class="section-sub">A typical session — publish, install, watch.</p>
</div>

<TerminalDemo />

<div class="section-head">
  <span class="section-tag">flow</span>
  <h2 class="section-title">How it works</h2>
  <p class="section-sub">Seven steps from your source folder into <code>node_modules</code>, and back out cleanly.</p>
</div>

<ProcessDiagram />

<style scoped>
.section-head {
  text-align: center;
  max-width: 720px;
  margin: 4rem auto 0.5rem;
  padding: 0 1rem;
}

.section-tag {
  display: inline-block;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.7rem;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  font-weight: 600;
  color: #d2691e;
  padding: 0.2rem 0.65rem;
  border-radius: 999px;
  background: rgba(210, 105, 30, 0.1);
  border: 1px solid rgba(210, 105, 30, 0.28);
  margin-bottom: 0.85rem;
}

.section-title {
  font-size: 2rem;
  font-weight: 700;
  letter-spacing: -0.02em;
  margin: 0 0 0.5rem;
  border-top: 0;
  padding-top: 0;
  background: linear-gradient(135deg, #b7410e, #d2691e, #8b4513);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}

.section-sub {
  color: var(--vp-c-text-2);
  font-size: 1rem;
  line-height: 1.6;
  margin: 0;
}

.section-sub code {
  font-size: 0.9em;
  padding: 0.1rem 0.4rem;
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-divider);
  border-radius: 4px;
}
</style>
