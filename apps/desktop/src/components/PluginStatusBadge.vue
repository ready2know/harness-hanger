<script setup lang="ts">
import { Badge } from '@/components/ui/badge'
import type { PluginStatus } from '@/lib/api'
import { statusLabels } from '@/lib/format'

defineProps<{ status: PluginStatus }>()

const styles: Record<PluginStatus, { variant: 'secondary' | 'outline'; class?: string }> = {
  enabled: {
    variant: 'secondary',
    class: 'bg-emerald-500/15 text-emerald-700 dark:bg-emerald-500/20 dark:text-emerald-400',
  },
  disabled: { variant: 'secondary' },
  notInstalled: { variant: 'outline', class: 'text-muted-foreground' },
}

const explanations: Record<PluginStatus, string> = {
  enabled: 'Claude Code loads it for your user, in every project.',
  disabled: 'Installed for your user, but turned off in Claude Code.',
  notInstalled: 'Not installed for your user. It can still be installed in a project.',
}
</script>

<template>
  <Badge
    :variant="styles[status].variant"
    :class="['shrink-0', styles[status].class]"
    :title="explanations[status]"
  >
    {{ statusLabels[status] }}
  </Badge>
</template>
