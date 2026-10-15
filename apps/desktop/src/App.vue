<script setup lang="ts">
import { useLocalStorage } from '@vueuse/core'
import { RouterView } from 'vue-router'

import AppSidebar from '@/components/AppSidebar.vue'
import { SidebarInset, SidebarProvider } from '@/components/ui/sidebar'

// Remembers the collapsed state across launches. The cookie that the generated provider writes
// does not reliably persist on the custom protocol the app is served from.
const sidebarOpen = useLocalStorage('sidebar-open', true)
</script>

<template>
  <SidebarProvider v-model:open="sidebarOpen">
    <AppSidebar />
    <SidebarInset>
      <RouterView />
    </SidebarInset>
  </SidebarProvider>
</template>
