export const pageSizes = ['5', '10', '25', '50', '100', 'all'] as const

export type PageSize = (typeof pageSizes)[number]

/** Where lists keep the page size that the person picked, and the size they start with. */
export interface PageSizeSetting {
  key: string
  initial: PageSize
}

/** The lists of the catalog: plugins, skills, agents, and commands. */
export const catalogPageSize: PageSizeSetting = { key: 'list-page-size', initial: '10' }

/** The lists of the Global and project pages, which hold several lists each. */
export const scopePageSize: PageSizeSetting = { key: 'scope-list-page-size', initial: '5' }
