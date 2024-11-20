<template>
  <SystemMonitor />
</template>

<script setup lang="ts">
import SystemMonitor from './components/SystemMonitor.vue'
</script>

<style>
html,
body {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background-color: #1e1e1e;
}

#app {
  width: 100%;
  height: 100%;
}
</style>