<script setup>
/* The page every shell screen sits in: the safe-area insets, the surface, one
   centred column. Kept here so the five screens do not each restate it. */
import { computed } from 'vue'

const style = computed(() => ({
  minHeight: '100dvh',
  boxSizing: 'border-box',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'stretch',
  justifyContent: 'center',
  gap: 'var(--space-5)',
  padding: 'max(var(--space-6), env(safe-area-inset-top)) max(var(--space-6), env(safe-area-inset-right)) max(var(--space-6), env(safe-area-inset-bottom)) max(var(--space-6), env(safe-area-inset-left))',
  background: 'var(--surface)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)'
}))
</script>

<template>
  <main :style="style"><slot /></main>
</template>
