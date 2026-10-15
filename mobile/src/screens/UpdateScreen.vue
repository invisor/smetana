<script setup>
import Frame from './Frame.vue'
import EmptyState from '@src/components/core/EmptyState.vue'
import Button from '@src/components/core/Button.vue'
import { openStore } from '../open.js'

defineProps({ storeUrl: { type: String, default: '' } })
</script>

<template>
  <Frame>
    <EmptyState
      icon="triangle-alert"
      title="Update the app"
      description="Your workstation needs a newer version of this app than the one installed."
    />
    <Button variant="primary" size="lg" full-width @click="openStore(storeUrl)">Open the store</Button>
  </Frame>
</template>
