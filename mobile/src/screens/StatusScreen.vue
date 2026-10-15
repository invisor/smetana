<script setup>
import Frame from './Frame.vue'
import EmptyState from '@src/components/core/EmptyState.vue'

defineProps({ loading: { type: Boolean, default: false } })
</script>

<template>
  <Frame>
    <EmptyState
      icon="cloud"
      :title="loading ? 'Starting' : 'Connecting to your workstation'"
      description="This takes a few seconds."
    />
  </Frame>
</template>
