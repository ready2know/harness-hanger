<script setup lang="ts">
import {
  BotIcon,
  FolderIcon,
  GlobeIcon,
  LayoutDashboardIcon,
  PuzzleIcon,
  SettingsIcon,
  SparklesIcon,
  StoreIcon,
} from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed, onMounted } from 'vue'

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
import { projectLabels } from '@/lib/format'
import { useProjectsStore } from '@/stores/projects'

const dashboardItems: NavItem[] = [{ to: '/', title: 'Dashboard', icon: LayoutDashboardIcon }]
const catalogItems: NavItem[] = [
  { to: '/marketplace', title: 'Marketplace', icon: StoreIcon },
  { to: '/skills', title: 'Skills', icon: SparklesIcon },
  { to: '/agents', title: 'Agents', icon: BotIcon },
  { to: '/plugins', title: 'Plugins', icon: PuzzleIcon },
]
const footerItems: NavItem[] = [{ to: '/settings', title: 'Settings', icon: SettingsIcon }]

const projectsStore = useProjectsStore()
const { projects } = storeToRefs(projectsStore)
onMounted(projectsStore.load)

const scopeItems = computed<NavItem[]>(() => {
  const labels = projectLabels(projects.value ?? [])
  return [
    { to: '/global', title: 'Global', icon: GlobeIcon },
    ...(projects.value ?? []).map((project) => ({
      to: `/projects/${project.id}`,
      title: labels.get(project.id) ?? project.name,
      icon: FolderIcon,
    })),
  ]
})
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
        <AppSidebarNav :items="dashboardItems" />
      </SidebarGroup>
      <SidebarSeparator />
      <SidebarGroup>
        <AppSidebarNav :items="catalogItems" />
      </SidebarGroup>
      <SidebarSeparator />
      <SidebarGroup aria-label="Global and projects">
        <AppSidebarNav :items="scopeItems" />
      </SidebarGroup>
    </SidebarContent>

    <SidebarSeparator />

    <SidebarFooter>
      <AppSidebarNav :items="footerItems" />
    </SidebarFooter>
  </Sidebar>
</template>
