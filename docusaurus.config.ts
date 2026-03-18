import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'Draton Docs',
  tagline: 'Authoritative documentation for the Draton language, toolchain, and self-host mirror.',
  favicon: 'data:image/svg+xml,<svg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 100 100%22><rect width=%22100%22 height=%22100%22 rx=%2218%22 fill=%22%23172b3a%22/><path d=%22M28 20h18c18 0 30 12 30 30S64 80 46 80H28V20zm14 14v32h4c10 0 16-6 16-16s-6-16-16-16h-4z%22 fill=%22%23f3b33e%22/></svg>',
  url: 'https://docs.draton.lhqm.io.vn',
  baseUrl: '/',
  organizationName: 'draton-lang',
  projectName: 'draton',
  trailingSlash: false,
  onBrokenLinks: 'throw',
  onDuplicateRoutes: 'throw',
  markdown: {
    mermaid: true,
    hooks: {
      onBrokenMarkdownLinks: 'throw'
    }
  },
  themes: ['@docusaurus/theme-mermaid'],
  presets: [
    [
      'classic',
      {
        docs: {
          routeBasePath: 'docs',
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/draton-lang/draton/tree/main/',
          showLastUpdateAuthor: false,
          showLastUpdateTime: true
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css'
        }
      } satisfies Preset.Options
    ]
  ],
  themeConfig: {
    image: 'data:image/svg+xml,<svg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 1200 630%22><rect width=%221200%22 height=%22630%22 fill=%22%23111827%22/><rect x=%2270%22 y=%2270%22 width=%221060%22 height=%22490%22 rx=%2234%22 fill=%22%23172b3a%22/><text x=%22120%22 y=%22240%22 fill=%22%23f3b33e%22 font-family=%22Arial, sans-serif%22 font-size=%2298%22 font-weight=%22700%22>Draton</text><text x=%22120%22 y=%22340%22 fill=%22%23f8fafc%22 font-family=%22Arial, sans-serif%22 font-size=%2248%22>Language and toolchain documentation</text><text x=%22120%22 y=%22430%22 fill=%22%23cbd5e1%22 font-family=%22Arial, sans-serif%22 font-size=%2234%22>Canonical syntax, architecture, tooling, runtime, and contributor rules</text></svg>',
    navbar: {
      title: 'Draton Docs',
      items: [
        {type: 'docSidebar', sidebarId: 'mainSidebar', position: 'left', label: 'Docs'},
        {to: '/docs/language/syntax-overview', label: 'Language', position: 'left'},
        {to: '/docs/tooling/cli-overview', label: 'Tooling', position: 'left'},
        {to: '/docs/compiler-architecture', label: 'Architecture', position: 'left'},
        {href: 'https://github.com/draton-lang/draton/releases', label: 'Releases', position: 'right'},
        {href: 'https://github.com/draton-lang/draton', label: 'GitHub', position: 'right'}
      ]
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Start Here',
          items: [
            {label: 'Overview', to: '/docs'},
            {label: 'Install', to: '/docs/install'},
            {label: 'Quickstart', to: '/docs/quickstart'}
          ]
        },
        {
          title: 'Language',
          items: [
            {label: 'Syntax Overview', to: '/docs/language/syntax-overview'},
            {label: 'Contracts and Types', to: '/docs/language/contracts-and-types'},
            {label: 'Modules, Classes, and Layers', to: '/docs/language/modules-and-structure'}
          ]
        },
        {
          title: 'Project',
          items: [
            {label: 'Compiler Architecture', to: '/docs/compiler-architecture'},
            {label: 'GC Scorecard', to: '/docs/gc-scorecard'},
            {label: 'GitHub Repository', href: 'https://github.com/draton-lang/draton'}
          ]
        }
      ],
      copyright: `Copyright ${new Date().getFullYear()} Draton contributors.`
    },
    prism: {
      additionalLanguages: ['bash', 'rust', 'toml', 'json', 'yaml']
    },
    colorMode: {
      defaultMode: 'light',
      disableSwitch: false,
      respectPrefersColorScheme: true
    }
  } satisfies Preset.ThemeConfig
};

export default config;
