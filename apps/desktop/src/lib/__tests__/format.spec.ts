import { describe, expect, it } from 'vitest'

import {
  count,
  formatSource,
  frontmatterFields,
  frontmatterTags,
  projectLabels,
  relativePath,
} from '../format'

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

  it('tells projects with the same name apart by their parent folder', () => {
    const project = (id: number, path: string) => ({
      id,
      path,
      name: path.split('/').pop()!,
      addedAt: '',
      error: null,
    })

    const labels = projectLabels([
      project(1, '/dev/hanger/master'),
      project(2, '/dev/zen/master'),
      project(3, '/dev/app'),
    ])

    expect([...labels.values()]).toEqual(['hanger/master', 'zen/master', 'app'])
  })

  it('reads tags from a list or from text separated by commas', () => {
    expect(frontmatterTags({ tags: ['review', ' ci ', 3, ''], keywords: 'git, github' })).toEqual([
      'review',
      'ci',
      'git',
      'github',
    ])
    expect(frontmatterTags({ name: 'x' })).toEqual([])
  })

  it('lists the frontmatter fields that a page does not show elsewhere', () => {
    expect(
      frontmatterFields({
        name: 'review',
        description: 'Reviews code.',
        tags: ['a'],
        model: 'opus',
        'user-invocable': false,
        'allowed-tools': ['Read', 'Grep'],
        metadata: { team: 'core' },
      }),
    ).toEqual([
      { key: 'model', kind: 'text', text: 'opus' },
      { key: 'user-invocable', kind: 'text', text: 'false' },
      { key: 'allowed-tools', kind: 'list', items: ['Read', 'Grep'] },
      { key: 'metadata', kind: 'json', json: '{\n  "team": "core"\n}' },
    ])
  })
})
