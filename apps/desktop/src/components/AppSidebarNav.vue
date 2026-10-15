<script setup lang="ts">
import { computed, type Component } from 'vue'
import { RouterLink, useRoute } from 'vue-router'

import { SidebarMenu, SidebarMenuButton, SidebarMenuItem } from '@/components/ui/sidebar'

export interface NavItem {
  /** The section's path. Pages below it, such as `/marketplace/3`, keep the item active. */
  to: string
  title: string
  icon: Component
}

const props = defineProps<{ items: NavItem[] }>()

const route = useRoute()

function covers(to: string): boolean {
  if (to === '/') return route.path === '/'
  return route.path === to || route.path.startsWith(`${to}/`)
}

/** The item with the longest path that covers the page, so that only Add collection is active on its page, not Collections too. */
const activeTo = computed(
  () =>
    props.items
      .map((item) => item.to)
      .filter(covers)
      .sort((a, b) => b.length - a.length)[0],
)

function isActive(item: NavItem): boolean {
  return item.to === activeTo.value
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
