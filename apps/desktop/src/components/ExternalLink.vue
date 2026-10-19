<script setup lang="ts">
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed } from 'vue'

/**
 * A link from a manifest. Only a web address opens, in the browser; anything else is text. The
 * default slot replaces the address as the link's text.
 */
const props = defineProps<{ href: string }>()

const web = computed(() => /^https?:\/\//i.test(props.href))
</script>

<template>
  <a
    v-if="web"
    :href="href"
    class="wrap-anywhere underline-offset-4 hover:underline"
    @click.prevent="openUrl(href)"
  >
    <slot>{{ href }}</slot>
  </a>
  <span v-else class="wrap-anywhere"
    ><slot>{{ href }}</slot></span
  >
</template>
