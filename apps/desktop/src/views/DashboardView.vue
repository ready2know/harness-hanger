<script setup lang="ts">
import { onMounted, ref } from 'vue'

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { getAppInfo, type AppInfo } from '@/lib/api'

const info = ref<AppInfo>()
const error = ref<string>()

onMounted(async () => {
  try {
    info.value = await getAppInfo()
  } catch (e) {
    error.value = String(e)
  }
})
</script>

<template>
  <div class="mx-auto flex w-full max-w-3xl flex-col gap-6 p-8">
    <h1 class="text-2xl font-semibold">Dashboard</h1>
    <Card>
      <CardHeader>
        <CardTitle>Installation</CardTitle>
        <CardDescription>Where Claude Code configuration is read from.</CardDescription>
      </CardHeader>
      <CardContent>
        <p v-if="error" class="text-destructive">{{ error }}</p>
        <dl v-else-if="info" class="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-2 text-sm">
          <dt class="text-muted-foreground">Version</dt>
          <dd>{{ info.version }}</dd>
          <dt class="text-muted-foreground">Claude Code config</dt>
          <dd class="font-mono break-all">{{ info.claudeConfigDir ?? 'not found' }}</dd>
        </dl>
        <p v-else class="text-muted-foreground">Loading…</p>
      </CardContent>
    </Card>
  </div>
</template>
