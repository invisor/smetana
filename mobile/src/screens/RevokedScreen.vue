<script setup>
import Frame from './Frame.vue'
import EmptyState from '@src/components/core/EmptyState.vue'
import Button from '@src/components/core/Button.vue'
import { unpair } from '../bridge/index.js'
</script>

<template>
  <Frame>
    <EmptyState
      icon="triangle-alert"
      title="This phone was removed"
      description="The workstation no longer lists this phone. Pair it again from the desktop's Devices tab."
    />
    <Button variant="primary" full-width @click="unpair">Pair again</Button>
  </Frame>
</template>
