import { describe, expect, it } from 'vitest'

import { count, formatSource, relativePath } from '../format'

describe('format', () => {
  it('counts with a singular or plural noun', () => {
    expect(count(1, 'skill')).toBe('1 skill')
    expect(count(0, 'skill')).toBe('0 skills')
    expect(count(3, 'agent')).toBe('3 agents')
  })

  it('formats marketplace sources', () => {
    expect(formatSource({ kind: 'github', owner: 'acme', repo: 'tools' })).toBe(
      'github.com/acme/tools',
    )
    expect(formatSource({ kind: 'local', path: '/src/market' })).toBe('/src/market')
  })

  it('shortens paths inside the root', () => {
    expect(relativePath('/m/plugins/a/SKILL.md', '/m')).toBe('plugins/a/SKILL.md')
    expect(relativePath('/m/plugins/a', '/m/')).toBe('plugins/a')
    expect(relativePath('C:\\m\\plugins\\a', 'C:\\m')).toBe('plugins\\a')
    expect(relativePath('/market/x', '/m')).toBe('/market/x')
    expect(relativePath('/other/x', '/m')).toBe('/other/x')
  })
})
