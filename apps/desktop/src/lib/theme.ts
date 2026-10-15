/** Mirrors the OS light/dark preference onto the `.dark` class used by the theme tokens. */
export function followSystemTheme(): void {
  const query = window.matchMedia('(prefers-color-scheme: dark)')
  const apply = () => document.documentElement.classList.toggle('dark', query.matches)
  apply()
  query.addEventListener('change', apply)
}
