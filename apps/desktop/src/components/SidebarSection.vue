<script setup lang="ts">
import { ChevronRightIcon } from '@lucide/vue'
import { useLocalStorage } from '@vueuse/core'
import { computed } from 'vue'

import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { SidebarGroup, SidebarGroupLabel, useSidebar } from '@/components/ui/sidebar'

const props = defineProps<{
  title: string
  /** The `localStorage` key that remembers whether the section is open. */
  storageKey: string
}>()

const open = useLocalStorage(props.storageKey, true)
const { state } = useSidebar()
// Collapsed to icons, the sidebar hides section titles, so the items stay reachable.
const shown = computed(() => open.value || state.value === 'collapsed')
</script>

<template>
  <Collapsible :open="shown" class="group/section" @update:open="open = $event">
    <SidebarGroup>
      <SidebarGroupLabel as-child>
        <CollapsibleTrigger
          class="hover:bg-sidebar-accent hover:text-sidebar-accent-foreground w-full cursor-pointer"
        >
          {{ title }}
          <ChevronRightIcon
            class="ml-auto transition-transform group-data-[state=open]/section:rotate-90"
          />
        </CollapsibleTrigger>
      </SidebarGroupLabel>
      <CollapsibleContent>
        <slot />
      </CollapsibleContent>
    </SidebarGroup>
  </Collapsible>
</template>
