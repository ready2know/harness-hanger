<script setup lang="ts">
import type { Component } from 'vue'
import { RouterLink, useRoute } from 'vue-router'

import { SidebarMenu, SidebarMenuButton, SidebarMenuItem } from '@/components/ui/sidebar'

export interface NavItem {
  /** The section's path. Pages below it, such as `/marketplace/3`, keep the item active. */
  to: string
  title: string
  icon: Component
}

defineProps<{ items: NavItem[] }>()

const route = useRoute()

function isActive(item: NavItem): boolean {
  if (item.to === '/') return route.path === '/'
  return route.path === item.to || route.path.startsWith(`${item.to}/`)
}
</script>

<template>
  <SidebarMenu>
    <SidebarMenuItem v-for="item in items" :key="item.to">
      <SidebarMenuButton as-child :is-active="isActive(item)" :tooltip="item.title">
        <RouterLink :to="item.to" :aria-current="isActive(item) ? 'page' : undefined">
          <component :is="item.icon" />
          <span>{{ item.title }}</span>
        </RouterLink>
      </SidebarMenuButton>
    </SidebarMenuItem>
  </SidebarMenu>
</template>
