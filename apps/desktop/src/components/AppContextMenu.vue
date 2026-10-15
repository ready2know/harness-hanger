<script setup lang="ts">
import { InfoIcon } from '@lucide/vue'
import { useEventListener } from '@vueuse/core'
import { ref } from 'vue'

import AboutDialog from '@/components/AboutDialog.vue'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'

const aboutOpen = ref(false)

// A context-menu trigger cancels the system menu itself, and only if nothing else has cancelled
// the event first, so leave events inside triggers alone. Elsewhere, such as in portalled dialogs,
// there is no menu of ours to show, so only cancel the system one.
useEventListener(document, 'contextmenu', (event) => {
  const target = event.target
  if (!(target instanceof Element && target.closest('[data-slot="context-menu-trigger"]'))) {
    event.preventDefault()
  }
})
</script>

<template>
  <ContextMenu>
    <!-- The generated trigger is select-none; keep the app's text selectable. -->
    <ContextMenuTrigger as="div" class="select-auto">
      <slot />
    </ContextMenuTrigger>
    <ContextMenuContent>
      <ContextMenuItem @select="aboutOpen = true">
        <InfoIcon />
        About
      </ContextMenuItem>
    </ContextMenuContent>
  </ContextMenu>

  <AboutDialog v-model:open="aboutOpen" />
</template>
