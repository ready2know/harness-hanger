<script setup lang="ts">
import { openUrl } from '@tauri-apps/plugin-opener'
import MarkdownIt from 'markdown-it'
import { computed } from 'vue'

const props = defineProps<{ source: string }>()

// Raw HTML is shown as text: the Markdown comes from marketplaces, which anyone can publish.
const markdown = new MarkdownIt({ html: false, linkify: true })

const html = computed(() => markdown.render(props.source))

/**
 * Opens web links in the browser. Other links, such as to a file next to the page, go nowhere:
 * the window only shows the app.
 */
function follow(event: MouseEvent) {
  const link = event.target instanceof Element ? event.target.closest('a') : null
  if (!link) return
  event.preventDefault()
  const href = link.getAttribute('href') ?? ''
  if (/^https?:\/\//i.test(href)) void openUrl(href)
}
</script>

<template>
  <!-- eslint-disable-next-line vue/no-v-html -- markdown-it escapes raw HTML -->
  <div
    class="prose prose-sm prose-neutral dark:prose-invert prose-code:before:content-none prose-code:after:content-none prose-blockquote:font-normal prose-blockquote:not-italic max-w-none wrap-anywhere [&_blockquote_p]:before:content-none [&_blockquote_p]:after:content-none"
    @click="follow"
    v-html="html"
  />
</template>
