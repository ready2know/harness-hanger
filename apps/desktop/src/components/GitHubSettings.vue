<script setup lang="ts">
import {
  CheckIcon,
  CopyIcon,
  KeyRoundIcon,
  LoaderCircleIcon,
  LogInIcon,
  LogOutIcon,
  ShieldCheckIcon,
  TerminalIcon,
} from '@lucide/vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import { onBeforeUnmount, onMounted, ref } from 'vue'

import ExternalLink from '@/components/ExternalLink.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import {
  cancelGhSignIn,
  checkGitHubToken,
  getGitHubStatus,
  pollBrowserSignIn,
  pollGhSignIn,
  saveGitHubToken,
  signOutOfGitHub,
  startBrowserSignIn,
  startGhSignIn,
  type AccountStatus,
  type SignInPrompt,
} from '@/lib/api'

const status = ref<AccountStatus>()
const loadError = ref<string>()
/** What the last action did, such as a check. */
const notice = ref<string>()
const actionError = ref<string>()
const busy = ref(false)

const tokenOpen = ref(false)
const token = ref('')
const tokenError = ref<string>()
const saving = ref(false)

interface SignIn {
  via: 'browser' | 'gh'
  /** Set once GitHub, or `gh`, gave a code. */
  prompt?: SignInPrompt
  error?: string
}
const signIn = ref<SignIn>()
let timer: ReturnType<typeof setTimeout> | undefined
// Each sign-in is numbered, so that a late answer for one that was closed is dropped.
let attempt = 0

onMounted(load)
onBeforeUnmount(stopSignIn)

async function load() {
  try {
    status.value = await getGitHubStatus()
    loadError.value = undefined
  } catch (e) {
    loadError.value = String(e)
  }
}

function clearMessages() {
  notice.value = undefined
  actionError.value = undefined
}

// The badge says so from now on, so there is no notice.
function signedIn(next: AccountStatus) {
  status.value = next
}

async function saveToken() {
  const value = token.value.trim()
  if (!value || saving.value) return
  clearMessages()
  saving.value = true
  tokenError.value = undefined
  try {
    signedIn(await saveGitHubToken(value))
    token.value = ''
    tokenOpen.value = false
  } catch (e) {
    tokenError.value = String(e)
  } finally {
    saving.value = false
  }
}

async function check() {
  clearMessages()
  busy.value = true
  try {
    const login = await checkGitHubToken()
    notice.value = `GitHub accepts the token in use. It belongs to ${login}.`
  } catch (e) {
    actionError.value = String(e)
  } finally {
    busy.value = false
  }
}

async function signOut() {
  clearMessages()
  busy.value = true
  try {
    status.value = await signOutOfGitHub()
    notice.value = 'The saved token is forgotten.'
  } catch (e) {
    actionError.value = String(e)
  } finally {
    busy.value = false
  }
}

async function startSignIn(via: SignIn['via']) {
  stopSignIn()
  clearMessages()
  const current = ++attempt
  signIn.value = { via }
  try {
    if (via === 'browser') {
      const code = await startBrowserSignIn()
      if (current !== attempt) return
      signIn.value = { via, prompt: code }
      later(() => pollBrowser(current, code.deviceCode, code.interval), code.interval)
    } else {
      const prompt = await startGhSignIn()
      if (current !== attempt) {
        // Closed while `gh` was starting.
        await cancelGhSignIn()
        return
      }
      signIn.value = { via, prompt }
      later(() => pollGh(current), 1)
    }
  } catch (e) {
    if (current === attempt) signIn.value = { via, error: String(e) }
  }
}

function later(poll: () => void, seconds: number) {
  timer = setTimeout(poll, seconds * 1000)
}

async function pollBrowser(current: number, deviceCode: string, interval: number) {
  try {
    const answer = await pollBrowserSignIn(deviceCode, interval)
    if (current !== attempt) return
    if (answer.state === 'done') finish(answer.status)
    else later(() => pollBrowser(current, deviceCode, answer.interval), answer.interval)
  } catch (e) {
    if (current === attempt) fail(String(e))
  }
}

async function pollGh(current: number) {
  try {
    const done = await pollGhSignIn()
    if (current !== attempt) return
    if (done) finish(done)
    else later(() => pollGh(current), 1)
  } catch (e) {
    if (current === attempt) fail(String(e))
  }
}

function finish(next: AccountStatus) {
  attempt++
  signIn.value = undefined
  signedIn(next)
}

function fail(error: string) {
  if (signIn.value) signIn.value = { ...signIn.value, error }
}

/** Stops waiting for a sign-in, and stops `gh` if it runs one. */
function stopSignIn() {
  clearTimeout(timer)
  attempt++
  const running = signIn.value
  signIn.value = undefined
  if (running?.via === 'gh' && !running.error) cancelGhSignIn().catch(() => {})
}

function setSignInOpen(open: boolean) {
  if (!open) stopSignIn()
}

async function copyAndOpen(prompt: SignInPrompt) {
  try {
    await navigator.clipboard.writeText(prompt.userCode)
  } catch {
    // The code is on screen to type in.
  }
  try {
    await openUrl(prompt.verificationUri)
  } catch (e) {
    fail(String(e))
  }
}
</script>

<template>
  <section class="flex flex-col gap-3" aria-labelledby="github-heading">
    <div class="flex flex-col gap-1">
      <h2 id="github-heading" class="text-lg font-semibold">GitHub</h2>
      <p class="text-muted-foreground text-sm">
        Signed in, Discover searches every marketplace on GitHub. Without a sign-in, it finds only
        repositories with the <code>claude-code-marketplace</code> topic. The token goes to GitHub
        only.
      </p>
    </div>

    <p v-if="loadError" role="alert" class="text-destructive wrap-anywhere">{{ loadError }}</p>
    <p v-else-if="!status" role="status" class="text-muted-foreground">Loading…</p>
    <div v-else class="flex flex-col gap-3 rounded-xl border p-4">
      <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
        <Badge
          v-if="status.source"
          class="bg-emerald-600 text-white dark:bg-emerald-600"
          data-testid="github-badge"
        >
          <CheckIcon />
          Signed in
        </Badge>
        <Badge v-else variant="outline" data-testid="github-badge">Not signed in</Badge>
        <p v-if="status.source" class="text-sm" data-testid="github-status">
          <template v-if="status.source.kind === 'env'">
            With the token in <code>{{ status.source.variable }}</code> from the environment.
            <template v-if="status.saved">It overrides the token saved here.</template>
          </template>
          <template v-else-if="status.source.kind === 'saved'">
            <template v-if="status.login"
              >As <strong>{{ status.login }}</strong
              >. </template
            >The token is saved in the system keychain.
          </template>
          <template v-else>
            Through the <code>gh</code> command<template v-if="status.login">
              as <strong>{{ status.login }}</strong></template
            >.
          </template>
        </p>
      </div>
      <p
        v-if="status.keychainError"
        role="alert"
        class="text-sm text-amber-700 wrap-anywhere dark:text-amber-400"
      >
        Cannot use the system keychain: {{ status.keychainError }}
      </p>

      <div class="flex flex-wrap gap-2">
        <Button v-if="status.browserSignIn" size="sm" @click="startSignIn('browser')">
          <LogInIcon />
          Sign in with GitHub
        </Button>
        <Button
          v-if="status.gh && !status.gh.signedIn"
          size="sm"
          :variant="status.browserSignIn ? 'outline' : 'default'"
          @click="startSignIn('gh')"
        >
          <TerminalIcon />
          Sign in with gh
        </Button>
        <Button
          size="sm"
          variant="outline"
          :aria-expanded="tokenOpen"
          @click="tokenOpen = !tokenOpen"
        >
          <KeyRoundIcon />
          Paste a token
        </Button>
        <Button v-if="status.source" size="sm" variant="outline" :disabled="busy" @click="check">
          <ShieldCheckIcon />
          Check
        </Button>
        <Button v-if="status.saved" size="sm" variant="ghost" :disabled="busy" @click="signOut">
          <LogOutIcon />
          Sign out
        </Button>
      </div>

      <form v-if="tokenOpen" class="flex flex-col gap-2" @submit.prevent="saveToken">
        <div class="flex gap-2">
          <Input
            v-model="token"
            type="password"
            autocomplete="off"
            aria-label="GitHub token"
            placeholder="ghp_… or github_pat_…"
            :disabled="saving"
          />
          <Button type="submit" :disabled="saving || !token.trim()">
            <LoaderCircleIcon v-if="saving" class="animate-spin" />
            {{ saving ? 'Checking…' : 'Save' }}
          </Button>
        </div>
        <p class="text-muted-foreground text-xs">
          A personal access token needs no scopes, because Discover reads only public repositories.
          harness-hanger checks it with GitHub, then saves it in the system keychain.
          <ExternalLink
            href="https://github.com/settings/tokens/new?description=harness-hanger"
            class="text-foreground underline"
          >
            Create one on GitHub
          </ExternalLink>
        </p>
        <p v-if="tokenError" role="alert" class="text-destructive text-sm wrap-anywhere">
          {{ tokenError }}
        </p>
      </form>

      <p
        v-if="notice"
        role="status"
        class="text-sm text-emerald-700 wrap-anywhere dark:text-emerald-400"
      >
        {{ notice }}
      </p>
      <p v-if="actionError" role="alert" class="text-destructive text-sm wrap-anywhere">
        {{ actionError }}
      </p>
    </div>

    <Dialog :open="signIn !== undefined" @update:open="setSignInOpen">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Sign in to GitHub</DialogTitle>
          <DialogDescription v-if="signIn?.via === 'gh'">
            The <code>gh</code> command signs in, and harness-hanger uses its login.
          </DialogDescription>
          <DialogDescription v-else>
            harness-hanger gets a token that reads public repositories, and saves it in the system
            keychain.
          </DialogDescription>
        </DialogHeader>

        <p v-if="signIn?.error" role="alert" class="text-destructive text-sm wrap-anywhere">
          {{ signIn.error }}
        </p>
        <p v-else-if="!signIn?.prompt" role="status" class="text-muted-foreground text-sm">
          Asking GitHub for a code…
        </p>
        <div v-else class="flex flex-col items-center gap-3 py-2">
          <p class="text-sm">Enter this code on GitHub:</p>
          <p
            class="font-mono text-3xl font-semibold tracking-widest select-all"
            data-testid="user-code"
          >
            {{ signIn.prompt.userCode }}
          </p>
          <Button @click="copyAndOpen(signIn.prompt)">
            <CopyIcon />
            Copy code and open GitHub
          </Button>
          <p role="status" class="text-muted-foreground flex items-center gap-2 text-sm">
            <LoaderCircleIcon class="size-4 animate-spin" />
            Waiting for you to finish on GitHub…
          </p>
        </div>

        <DialogFooter>
          <Button variant="ghost" @click="stopSignIn">
            {{ signIn?.error ? 'Close' : 'Cancel' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </section>
</template>
