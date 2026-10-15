<script setup lang="ts">
import {
  BotIcon,
  LayoutDashboardIcon,
  PuzzleIcon,
  SettingsIcon,
  SparklesIcon,
  StoreIcon,
} from '@lucide/vue'

import AppSidebarNav, { type NavItem } from '@/components/AppSidebarNav.vue'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarSeparator,
  SidebarTrigger,
} from '@/components/ui/sidebar'

const mainItems: NavItem[] = [
  { to: '/', title: 'Dashboard', icon: LayoutDashboardIcon },
  { to: '/marketplace', title: 'Marketplace', icon: StoreIcon },
  { to: '/skills', title: 'Skills', icon: SparklesIcon },
  { to: '/agents', title: 'Agents', icon: BotIcon },
  { to: '/plugins', title: 'Plugins', icon: PuzzleIcon },
]
const footerItems: NavItem[] = [{ to: '/settings', title: 'Settings', icon: SettingsIcon }]
</script>

<template>
  <Sidebar collapsible="icon">
    <SidebarHeader>
      <div
        class="flex h-8 items-center justify-between group-data-[collapsible=icon]:justify-center"
      >
        <span class="truncate px-2 text-sm font-semibold group-data-[collapsible=icon]:hidden">
          harness-hanger
        </span>
        <SidebarTrigger />
      </div>
    </SidebarHeader>

    <SidebarContent>
      <SidebarGroup>
        <AppSidebarNav :items="mainItems" />
      </SidebarGroup>
    </SidebarContent>

    <SidebarSeparator />

    <SidebarFooter>
      <AppSidebarNav :items="footerItems" />
    </SidebarFooter>
  </Sidebar>
</template>
