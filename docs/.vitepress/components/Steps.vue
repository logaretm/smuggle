<script setup>
defineProps({});
</script>

<template>
  <ol class="steps">
    <slot />
  </ol>
</template>

<style scoped>
.steps {
  list-style: none;
  counter-reset: step;
  margin: 1.5rem 0;
  padding: 0;
  position: relative;
}

.steps :deep(> li) {
  counter-increment: step;
  position: relative;
  padding-left: 3.25rem;
  padding-bottom: 1.75rem;
  margin: 0;
}

.steps :deep(> li::before) {
  content: counter(step);
  position: absolute;
  left: 0;
  top: 0;
  width: 2.25rem;
  height: 2.25rem;
  display: grid;
  place-items: center;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-weight: 700;
  font-size: 0.95rem;
  color: #fff5e6;
  background: linear-gradient(135deg, #b7410e, #d2691e);
  border-radius: 999px;
  box-shadow: 0 0 0 4px var(--vp-c-bg), 0 4px 12px -4px rgba(183, 65, 14, 0.5);
  z-index: 1;
}

.steps :deep(> li::after) {
  content: "";
  position: absolute;
  left: 1.125rem;
  top: 2.5rem;
  bottom: 0;
  width: 2px;
  background: linear-gradient(180deg, rgba(210, 105, 30, 0.4), rgba(210, 105, 30, 0));
}

.steps :deep(> li:last-child) { padding-bottom: 0; }
.steps :deep(> li:last-child::after) { display: none; }

.steps :deep(> li > h3) {
  margin: 0.25rem 0 0.5rem;
  padding: 0;
  font-size: 1.15rem;
  font-weight: 600;
  letter-spacing: -0.01em;
  border: 0;
}

.steps :deep(> li > h3:first-child) { margin-top: 0.25rem; }

.steps :deep(> li > p) {
  margin: 0.5rem 0;
}

.steps :deep(> li > div[class*="language-"]) {
  margin: 0.6rem 0;
}

@media (max-width: 540px) {
  .steps :deep(> li) { padding-left: 2.5rem; }
  .steps :deep(> li::before) {
    width: 1.85rem;
    height: 1.85rem;
    font-size: 0.85rem;
  }
  .steps :deep(> li::after) {
    left: 0.925rem;
    top: 2.05rem;
  }
}
</style>
