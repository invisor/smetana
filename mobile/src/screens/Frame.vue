<script setup>
/* The page every shell screen sits in: the safe-area insets, the surface, one
   centred column. Kept here so the five screens do not each restate it. */
import { computed } from 'vue'
import { around } from './safeArea.js'

const style = computed(() => ({
  minHeight: '100dvh',
  boxSizing: 'border-box',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'stretch',
  justifyContent: 'center',
  gap: 'var(--space-5)',
  padding: around('var(--space-6)'),
  background: 'var(--surface)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)'
}))
</script>

<template>
  <main :style="style"><slot /></main>
</template>
